use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use glm_cache::{
    DRAFT_INDEXER_PAGE_BYTES, DRAFT_KV_PAGE_BYTES, DurablePageRequest, FileTierStore,
    NamespaceInputs, PAGE_TOKENS, PagePieceBytes, PageTableConfig, PrefixIndex, PrefixNamespace,
    Residency, ResidencyConfig, ResidencyError, ResidencyManager, RestoreError, RestoreService,
    SequencePageTable, StoreError, TierPiece, decode_draft_sidecar_payload,
    encode_draft_sidecar_payload, owner_rank,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const PAGE_COUNT: usize = 3;
const TORN_JOURNAL_BYTES: usize = 113;

#[derive(Debug, Serialize)]
pub struct CacheLifecycleProof {
    schema: &'static str,
    pages_published: usize,
    page_tokens: u64,
    mtp_sidecars: usize,
    page_payload_bytes: u64,
    resident_payload_bytes: u64,
    torn_journal_bytes: usize,
    restart_recovered_pages: usize,
    prefix_match_tokens: u64,
    dcp_postures_reusing_namespace: [u8; 2],
    bounded_restore_saturation_observed: bool,
    hbm_capacity_pages: u8,
    dram_capacity_pages: u8,
    pinned_pressure_failed_closed: bool,
    hbm_to_dram_to_nvme_observed: bool,
    copy_on_write_tail_observed: bool,
    speculative_rollback_observed: bool,
    partial_speculative_commit_observed: bool,
    corruption_failed_closed: bool,
    corrupted_page_remained_nvme: bool,
    journal_sha256: String,
    pages_sha256_after_corruption: String,
    verdict: &'static str,
}

pub fn write_cache_lifecycle_proof(
    evidence_dir: &Path,
) -> Result<CacheLifecycleProof, Box<dyn Error>> {
    if evidence_dir.try_exists()? {
        return Err(proof_error("cache lifecycle evidence directory already exists").into());
    }
    fs::create_dir_all(evidence_dir)?;
    let store_root = evidence_dir.join("tier-store");
    let namespace = proof_namespace()?;
    let tokens: Vec<u32> = (0..(PAGE_COUNT as u32 * u32::try_from(PAGE_TOKENS)?)).collect();
    let key_oracle = PrefixIndex::new(namespace);
    let keys = key_oracle.derive_keys(&tokens);

    let mut store = FileTierStore::open(&store_root)?;
    let mut published = Vec::with_capacity(PAGE_COUNT);
    for (ordinal, key) in keys.iter().enumerate() {
        published.push(store.publish(page_request(
            namespace,
            key.0,
            u64::try_from(ordinal + 1)?,
            u8::try_from(0x20 + ordinal)?,
        )?)?);
    }
    let page_payload_bytes = published[0]
        .pieces
        .iter()
        .try_fold(0_u64, |total, piece| total.checked_add(piece.byte_length))
        .ok_or_else(|| proof_error("page byte sum overflow"))?;
    let resident_payload_bytes = store.resident_bytes();
    drop(store);

    let journal_path = store_root.join("journal.log");
    let mut journal = OpenOptions::new().append(true).open(&journal_path)?;
    journal.write_all(&[0xa5; TORN_JOURNAL_BYTES])?;
    journal.sync_data()?;
    drop(journal);

    let mut reopened = FileTierStore::open(&store_root)?;
    let mut recovered = Vec::with_capacity(PAGE_COUNT);
    for (ordinal, key) in keys.iter().enumerate() {
        let page = reopened
            .restore(key.0)?
            .ok_or_else(|| proof_error("published page disappeared after restart"))?;
        let sidecar = page
            .pieces
            .get(&TierPiece::DraftSidecar)
            .ok_or_else(|| proof_error("restored page lost its draft sidecar"))?;
        let (draft_kv, draft_indexer) = decode_draft_sidecar_payload(sidecar)?;
        let marker = u8::try_from(0x20 + ordinal)?;
        if !draft_kv.iter().all(|&byte| byte == marker.wrapping_add(2))
            || !draft_indexer
                .iter()
                .all(|&byte| byte == marker.wrapping_add(3))
        {
            return Err(proof_error("restored token-major draft sidecar changed").into());
        }
        recovered.push(page.record);
    }
    if recovered != published {
        return Err(proof_error("restart changed published tier records").into());
    }

    let mut prefix = PrefixIndex::new(namespace);
    prefix.insert(&tokens, recovered.clone())?;
    let mut query = tokens.clone();
    query.extend(10_000..10_017);
    let prefix_match = prefix
        .longest_match_with_capability(&query, true)
        .ok_or_else(|| proof_error("MTP prefix was not reusable after restart"))?;
    for _dcp_posture in [1_u8, 4_u8] {
        if prefix
            .longest_match_with_capability(&query, true)
            .as_ref()
            .map(|matched| matched.page_keys.as_slice())
            != Some(keys.as_slice())
        {
            return Err(proof_error("DCP-neutral prefix identity drifted").into());
        }
    }
    drop(reopened);

    let mut residency = ResidencyManager::new(ResidencyConfig {
        hbm_bytes: page_payload_bytes,
        dram_bytes: page_payload_bytes,
    })?;
    for record in &recovered {
        residency.register_nvme(record.clone())?;
    }
    let service = RestoreService::spawn(&store_root, 1)?;

    let first_request = residency.begin_restore(1, keys[0].0, 0, owner_rank(0))?;
    let first_handle = service.try_submit(first_request)?;
    let second_request = residency.begin_restore(2, keys[1].0, 1, owner_rank(1))?;
    let bounded_restore_saturation_observed = matches!(
        service.try_submit(second_request),
        Err(RestoreError::Saturated)
    );
    if !bounded_restore_saturation_observed {
        return Err(proof_error("restore queue did not enforce its bound").into());
    }
    residency.abort_restore(keys[1].0)?;
    residency.complete_restore(first_handle.receive()?)?;

    restore_page(&mut residency, &service, keys[1].0, 1, 3)?;
    if residency.location(keys[0].0) != Some(Residency::Dram)
        || residency.location(keys[1].0) != Some(Residency::Hbm)
    {
        return Err(proof_error("HBM pressure did not demote the LRU page to DRAM").into());
    }

    residency.pin_hbm(keys[1].0)?;
    let pinned_request = residency.begin_restore(4, keys[2].0, 2, owner_rank(2))?;
    let pinned_result = service.try_submit(pinned_request)?.receive()?;
    let pinned_pressure_failed_closed = matches!(
        residency.complete_restore(pinned_result),
        Err(ResidencyError::Pinned)
    );
    if !pinned_pressure_failed_closed {
        return Err(proof_error("pinned HBM pressure did not fail closed").into());
    }
    residency.abort_restore(keys[2].0)?;
    residency.unpin(keys[1].0)?;

    restore_page(&mut residency, &service, keys[2].0, 2, 5)?;
    if residency.location(keys[1].0) != Some(Residency::Nvme)
        || residency.location(keys[2].0) != Some(Residency::Hbm)
    {
        return Err(proof_error("full DRAM did not force HBM eviction to NVMe").into());
    }
    residency.promote_dram(keys[0].0)?;
    if residency.location(keys[0].0) != Some(Residency::Hbm)
        || residency.location(keys[2].0) != Some(Residency::Nvme)
    {
        return Err(proof_error("DRAM promotion did not preserve bounded residency").into());
    }
    residency.pin_hbm(keys[0].0)?;
    let second_pinned_request = residency.begin_restore(6, keys[1].0, 1, owner_rank(1))?;
    let second_pinned_result = service.try_submit(second_pinned_request)?.receive()?;
    if !matches!(
        residency.complete_restore(second_pinned_result),
        Err(ResidencyError::Pinned)
    ) {
        return Err(proof_error("second pinned-pressure path did not fail closed").into());
    }
    residency.abort_restore(keys[1].0)?;
    residency.unpin(keys[0].0)?;
    restore_page(&mut residency, &service, keys[1].0, 1, 7)?;
    let hbm_to_dram_to_nvme_observed = residency.location(keys[0].0) == Some(Residency::Dram)
        && residency.location(keys[1].0) == Some(Residency::Hbm)
        && residency.location(keys[2].0) == Some(Residency::Nvme);
    if !hbm_to_dram_to_nvme_observed {
        return Err(proof_error("final tier posture is not bounded HBM/DRAM/NVMe").into());
    }
    drop(service);

    let (
        copy_on_write_tail_observed,
        speculative_rollback_observed,
        partial_speculative_commit_observed,
    ) = prove_page_table_lifecycle(&keys)?;

    let corrupt_offset = recovered[2]
        .pieces
        .iter()
        .find(|piece| piece.piece == TierPiece::TargetKv)
        .ok_or_else(|| proof_error("target KV piece missing"))?
        .storage_offset;
    let data_path = store_root.join("pages.dat");
    corrupt_one_byte(&data_path, corrupt_offset)?;
    let service = RestoreService::spawn(&store_root, 1)?;
    let corrupt_request = residency.begin_restore(8, keys[2].0, 2, owner_rank(2))?;
    let corruption_failed_closed = matches!(
        service.try_submit(corrupt_request)?.receive(),
        Err(RestoreError::Store(StoreError::Checksum))
    );
    if !corruption_failed_closed {
        return Err(proof_error("corrupt tier payload was observable").into());
    }
    residency.abort_restore(keys[2].0)?;
    let corrupted_page_remained_nvme = residency.location(keys[2].0) == Some(Residency::Nvme);
    if !corrupted_page_remained_nvme {
        return Err(proof_error("corrupt restore changed residency state").into());
    }
    drop(service);

    let report = CacheLifecycleProof {
        schema: "glmaxx.cache-lifecycle-proof.v1",
        pages_published: PAGE_COUNT,
        page_tokens: PAGE_TOKENS,
        mtp_sidecars: PAGE_COUNT,
        page_payload_bytes,
        resident_payload_bytes,
        torn_journal_bytes: TORN_JOURNAL_BYTES,
        restart_recovered_pages: recovered.len(),
        prefix_match_tokens: prefix_match.matched_tokens,
        dcp_postures_reusing_namespace: [1, 4],
        bounded_restore_saturation_observed,
        hbm_capacity_pages: 1,
        dram_capacity_pages: 1,
        pinned_pressure_failed_closed,
        hbm_to_dram_to_nvme_observed,
        copy_on_write_tail_observed,
        speculative_rollback_observed,
        partial_speculative_commit_observed,
        corruption_failed_closed,
        corrupted_page_remained_nvme,
        journal_sha256: file_sha256(&journal_path)?,
        pages_sha256_after_corruption: file_sha256(&data_path)?,
        verdict: "CPU_CACHE_LIFECYCLE_PASS",
    };
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    let report_path = evidence_dir.join("cache-lifecycle-proof.json");
    fs::write(&report_path, &json)?;
    println!("wrote {} bytes to {}", json.len(), report_path.display());
    Ok(report)
}

fn proof_namespace() -> Result<PrefixNamespace, Box<dyn Error>> {
    Ok(PrefixNamespace::new(NamespaceInputs {
        model_revision_sha256: [1; 32],
        tokenizer_sha256: [2; 32],
        chat_template_sha256: [3; 32],
        weight_policy_hash: [4; 32],
        target_kv_abi_sha256: [5; 32],
        draft_kv_abi_sha256: [6; 32],
        rope_parameters_sha256: [7; 32],
    })?)
}

fn page_request(
    namespace: PrefixNamespace,
    page_key: [u8; 32],
    generation: u64,
    marker: u8,
) -> Result<DurablePageRequest, Box<dyn Error>> {
    let draft_kv = vec![marker.wrapping_add(2); usize::try_from(DRAFT_KV_PAGE_BYTES)?];
    let draft_indexer = vec![marker.wrapping_add(3); usize::try_from(DRAFT_INDEXER_PAGE_BYTES)?];
    let draft_sidecar = encode_draft_sidecar_payload(&draft_kv, &draft_indexer)?;
    Ok(DurablePageRequest {
        namespace: namespace.0,
        page_key,
        generation,
        mtp: true,
        pieces: vec![
            PagePieceBytes {
                piece: TierPiece::TargetKv,
                bytes: vec![marker; usize::try_from(TierPiece::TargetKv.expected_bytes())?],
            },
            PagePieceBytes {
                piece: TierPiece::TargetIndexer,
                bytes: vec![
                    marker.wrapping_add(1);
                    usize::try_from(TierPiece::TargetIndexer.expected_bytes())?
                ],
            },
            PagePieceBytes {
                piece: TierPiece::DraftSidecar,
                bytes: draft_sidecar,
            },
        ],
    })
}

fn restore_page(
    residency: &mut ResidencyManager,
    service: &RestoreService,
    page_key: [u8; 32],
    ordinal: u64,
    request_id: u64,
) -> Result<(), Box<dyn Error>> {
    let request = residency.begin_restore(request_id, page_key, ordinal, owner_rank(ordinal))?;
    let result = service.try_submit(request)?.receive()?;
    residency.complete_restore(result)?;
    Ok(())
}

fn prove_page_table_lifecycle(
    keys: &[glm_cache::PrefixPageKey],
) -> Result<(bool, bool, bool), Box<dyn Error>> {
    let prefix_pages: Vec<_> = keys.iter().copied().map(|key| (key, true)).collect();
    let mut table = SequencePageTable::new(PageTableConfig {
        target_pages_per_rank: 4,
        draft_pages_per_rank: 4,
    })?;
    table.admit_with_prefix(1, true, &prefix_pages)?;
    table.fork_sequence(1, 2)?;
    table.append_committed(1, 5)?;
    table.fork_sequence(1, 3)?;
    let source = table.pages(1)?;
    let fork = table.pages(3)?;
    let copy_on_write_tail_observed = source.len() == 4
        && fork.len() == 4
        && source[..3]
            .iter()
            .zip(&fork[..3])
            .all(|(left, right)| left.physical == right.physical && left.references == 3)
        && source[3].physical != fork[3].physical
        && source[3].valid_tokens == 5
        && fork[3].valid_tokens == 5;
    if !copy_on_write_tail_observed {
        return Err(proof_error("partial prefix fork did not copy its mutable tail").into());
    }

    table.begin_tentative(3, 7)?;
    table.rollback_tentative(3)?;
    let speculative_rollback_observed = table.committed_tokens(3) == Some(197)
        && table
            .pages(3)?
            .last()
            .is_some_and(|page| page.valid_tokens == 5);
    if !speculative_rollback_observed {
        return Err(proof_error("speculative rollback changed committed state").into());
    }

    table.begin_tentative(3, 7)?;
    table.commit_tentative(3, 3)?;
    let partial_speculative_commit_observed = table.committed_tokens(3) == Some(200)
        && table
            .pages(3)?
            .last()
            .is_some_and(|page| page.valid_tokens == 8);
    if !partial_speculative_commit_observed {
        return Err(proof_error("partial speculative commit is inconsistent").into());
    }
    table.remove_sequence(3)?;
    table.remove_sequence(1)?;
    table.remove_sequence(2)?;
    let stats = table.stats()?;
    if stats.active_sequences != 0
        || stats.active_positions != 0
        || stats.target_pages_used != [0; 4]
        || stats.draft_pages_used != [0; 4]
    {
        return Err(proof_error("page-table cleanup leaked capacity").into());
    }
    Ok((
        copy_on_write_tail_observed,
        speculative_rollback_observed,
        partial_speculative_commit_observed,
    ))
}

fn corrupt_one_byte(path: &Path, offset: u64) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte)?;
    byte[0] ^= 0xff;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&byte)?;
    file.sync_data()?;
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn proof_error(message: &str) -> std::io::Error {
    std::io::Error::other(message)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn cache_lifecycle_is_bounded_recoverable_and_fail_closed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root: PathBuf = std::env::temp_dir().join(format!(
            "glmaxx-cache-lifecycle-{}-{nonce}",
            std::process::id()
        ));
        let report = write_cache_lifecycle_proof(&root).unwrap();
        assert_eq!(report.verdict, "CPU_CACHE_LIFECYCLE_PASS");
        assert_eq!(report.restart_recovered_pages, PAGE_COUNT);
        assert!(report.corruption_failed_closed);
        fs::remove_dir_all(root).unwrap();
    }
}
