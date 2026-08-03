# Fable review: production rank-manifest validation v2

Date: 2026-07-31
Reviewer: Fable (adversarial CPU implementation reviewer)
Handoff: `docs/fable-production-rank-manifest-validation-v2-handoff.md`

Note: the operator directed review artifacts into `docs/reviews/` rather than
the repository root named in the handoff.

Reviewed candidate commit (verified as detached worktree HEAD at start and
finish):

4bf7bb5e817e01cc299058b56a488b35011fd79d

Review venue: detached worktree at the pinned commit; the untracked handoff
copy in the worktree was verified byte-identical (`cmp`) to the main-repo copy
before use. `review-proof` on the handoff returned verdict `PASS` with all 11
expected/actual input digests equal. `cargo test --offline -p glm-format`
passed 63/63 (60 lib tests + 3 nvfp4 proof tests, 58.8 s), including
`pinned_capacity_inventory_source_bindings_match_validator`, which recomputes
all four pinned tensor-contract digests from
`PinnedRankPlan::manifest_tensors`.

## Provenance: input hashes verified at start AND finish

All 11 pinned inputs were hashed with `shasum -a 256` at review start and
again at review finish. Both sets match the handoff table exactly and are
identical to each other; the worktree HEAD was re-confirmed unchanged at
finish. Hashes were verified at start and verified again at finish; no input
is stale.

| Input | SHA-256 (identical at start and finish) |
|---|---|
| `crates/glm-format/src/rank_manifest.rs` | 3b94f2306e2c0ee82f342b66b945f48767441463929e12b575648f9ccda99d6b |
| `crates/glm-format/src/native_reader.rs` | 24eef8432a8dff2e830a8ec63e4e46bffcfafd94486e64fbb467945825ab0089 |
| `crates/glm-format/src/checkpoint.rs` | 08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c |
| `crates/glm-format/src/lib.rs` | 52aea8d21b3c3a504a46aa3c9517233fb8b438374ce5fb2e9ad07dc316ee7c0b |
| `crates/glm-cli/src/main.rs` | 3b76e43b81a1bf7a540565e4b8356999a1b2dcc9e5c8dd1036d4e3b17708128c |
| `spec/format-v0.md` | 619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a |
| `docs/checkpoint-ingest.md` | 186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d |
| `manifests/glm52-operation-v1.json` | 8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6 |
| `docs/fable-manifest-abi-v022-r2-handoff.md` | d13839b369b22b0614fea641836c755cb544411c8343ca2ab6f78cc0a603f0e0 |
| `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` | 24cd8f8502d6a6f2a34c0e28cb46083ef2585924e1fc60935dc0cae0f1c3118f |
| `scripts/local-checks.sh` | 378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b |

## Independent source-map derivation (external anchor)

Fetched only `MANIFEST.sha256` from the pinned Hugging Face revision
`9297b9f1d53af5c67cffa01e30cc071a1ff7144b` of
`brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw`. Observed: exactly 8,528 bytes, 92
newline-terminated records, final byte `0x0a`. Its SHA-256, matching both the
handoff and the engine constant `PINNED_SOURCE_MANIFEST_SHA256`
(`crates/glm-format/src/checkpoint.rs:24`):

bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63

Replacement value applied for `.gitattributes` (matches the engine's
`PINNED_GITATTRIBUTES_REVISION_SHA256`):

5bb36c320417db43af1dc6af8bd0fcc154bb7276eddaf96b12c395bdafed634d

Replacement value applied for `README.md` (matches the engine's
`PINNED_README_REVISION_SHA256`):

e60e023082ee175a11f51e79e8dd88f5e4ed9975fc904e64cdeabbbcf8abe225

An independent python3 script parsed the 92 `hash  filename` records into a
string map, applied the two replacements, and encoded with
`json.dumps(m, sort_keys=True, separators=(",",":"))` (8,805 bytes UTF-8).
SHA-256 of that encoding, matching the handoff and the engine constant
`PINNED_SOURCE_FILE_MAP_SHA256` (`checkpoint.rs:28`):

ad1e4fb286adbc261a2800ab17e4abde5bcd13efb22b150d65ec42b47e2af5fe

Additional external cross-checks from the fetched manifest: the record for
`model.safetensors.index.json` equals `PINNED_EXL3_INDEX_SHA256`
(`346227a4...64c2`), and the manifest-side hashes for `.gitattributes` and
`README.md` equal `PINNED_GITATTRIBUTES_MANIFEST_SHA256` (`34448b82...a930`)
and `PINNED_README_MANIFEST_SHA256` (`ed5aca8c...06e8`), so the two-entry
publisher exception in `is_pinned_publisher_manifest_exception` is anchored on
both sides by independently observed values. `LICENSE`,
`calibration_manifest.json`, `chat_template.jinja`, `config.json`,
`tokenizer.json`, `tokenizer_config.json`, and `generation_config.json` are
all present in the external manifest, so every name the writer requires via
`required_source_sha256` exists.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `source_verified_file_bytes` is only lower-bounded, never pinned.
   `rank_manifest.rs:162` accepts any value
   `>= PINNED_EXL3_PAYLOAD_BYTES` (316,304,795,648), and four-rank consensus
   (`native_reader.rs:866`) only requires the four claims to be equal. A
   forger can claim an arbitrarily inflated verified-byte figure in an
   otherwise-passing manifest. All 92 file identities are still pinned via the
   source-map digest, so this is cosmetic provenance, not an integrity hole.
2. Memory amplification when opening an untrusted rank file. The manifest
   control region may be up to `CONTROL_REGIONS_TOTAL_MAX_BYTES` (512 MiB)
   and is parsed into a full `serde_json::Value`
   (`rank_manifest.rs:76`) before any schema check; a crafted
   number-array manifest of that size expands to multi-gigabyte transient
   heap. This is bounded (caps, serde_json's 128-level recursion limit, and
   the attacker must supply the bytes on local disk), but it is avoidable
   pre-validation cost on the `NativeRankReader::open` path.
3. `manifest_consensus` (`native_reader.rs:855`) accepts `(None, None)`, so a
   four-rank set whose manifests all use a non-production schema (e.g. the
   `reader-test` fixtures) passes `validate_rank_set`. Production gating
   therefore depends on each caller also demanding `validated_manifest()`;
   `native_rank_proof` does (`glm-cli/src/main.rs:524-531`), but any future
   caller that treats `validate_rank_set` success alone as production
   validation would be wrong. Worth a doc comment or a stricter entry point.

### QUESTION

1. Payload-content residual (deliberate scope boundary — confirm ownership
   downstream). The engine-owned inventory pins tensor structure, byte
   counts, and codec-metadata digests, but not the converted payload content:
   per-tensor `payload_sha256`/`aux_sha256` live in the descriptor plane and
   `integrity.output_payload_sha256` sits outside the `tensors` array, so a
   forger who replaces weight values (same lengths) and re-signs descriptor
   hashes, header hashes, `output_payload_sha256`, header CRC, `file_uuid`,
   and `conversion_uuid` passes this validator and `native-rank-proof`
   (which reports, but cannot pin, the payload digests). No compile-time
   constant can exist before conversion has produced the deterministic
   payloads. The checkpoint-load-transaction r2 handoff assigns payload-hash
   binding and HBM-content proof to that gate; once conversion output exists,
   consider pinning the four `payload_sha256` values as engine-owned
   constants in a follow-up so content joins structure under the immutable
   identity.

## Answers to the 12 required adversarial questions

1. Contract-digest recomputation: YES, exact, and all four differ for the
   expected reasons. The passing test
   `pinned_capacity_inventory_source_bindings_match_validator` rebuilds each
   rank's `manifest_tensors()`, canonical-JSON encodes it, and its SHA-256
   equals the corresponding entry of `PINNED_RANK_TENSOR_CONTRACT_SHA256`
   (`rank_manifest.rs:24-29`); the four constants are pairwise distinct.
   Rank divergence comes from: `contiguous_tp_slice` start/end
   (= shard * rank), `explicit_rank_components` component names embedding
   `rank{N}` and start/end = rank..rank+1, and per-rank
   `codec_metadata_sha256` (Exl3Metadata embeds the rank). Replicated
   tensors are identical across ranks, as intended. On provenance: the
   constants are historically a fixed point of the engine's own emitter, but
   the same test independently re-validates every tensor through
   `validate_source_binding` and `validate_reconstruction` and re-derives the
   payload-byte total against `PINNED_RANK_SOURCE_PAYLOAD_BYTES`, and the
   underlying shape tables in `protected_tensor_contracts()` are separately
   cross-checked against the externally anchored checkpoint byte inventory
   (`validate_pinned_exl3_checkpoint` dtype-byte pins). This is a genuine
   engine-owned identity, not a bare emitter tautology.
2. YES. The production path is `NativeRankReader::open` →
   `validate_rank_manifest` (always `require_pinned_inventory = true`) →
   `validate_pinned_inventory` (`rank_manifest.rs:123-141`), which requires
   exact `tensor_count == PINNED_RANK_TENSOR_COUNT` (59,585), exact
   `tensor_source_payload_bytes == PINNED_RANK_SOURCE_PAYLOAD_BYTES`
   (81,590,319,104, also re-derived from per-tensor sums with checked
   arithmetic at `rank_manifest.rs:177-188`), the rank's own fixed contract
   digest, and the fixed full source-map identity
   (SHA-256 of the canonical `source_file_sha256` map ==
   `PINNED_SOURCE_FILE_MAP_SHA256`). A consistent rewrite of descriptors plus
   manifest still fails: the `tensors` array must hash to the compiled-in
   per-rank constant, so it must be byte-identical to the engine plan's
   canonical encoding, and `validate_tensors` forces the descriptors to match
   that array field-for-field. The residual (payload content, not structure)
   is QUESTION 1.
3. Mutate-and-resign trace (thought experiment over
   `validate_rank_manifest_inner`, plus the fixture tests that exercise the
   same classes): every mutation of name, role_id, layer/expert, rank/global/
   padded/source shape, source kind/axis/start/end/components, source_dtype,
   logical/stored dtype, codec_id, flags, primary/aux byte counts,
   quant_group_elements, codec_metadata_sha256, reconstruction, or
   collective_after changes the canonical `tensors` bytes and is rejected by
   the pinned contract digest (`Inventory`), and independently by the
   descriptor cross-check (`Tensor(i)`), the derivational slice checks
   (`validate_source_binding`), the codec binding
   (`validate_reconstruction`), or `expected_collective`. Descriptor-only
   mutations desynchronize from the manifest (`Tensor(i)`); manifest-only
   mutations break the contract digest; changing both consistently breaks the
   pinned constant. No listed class is uncaught. Payload content bytes (same
   lengths, full re-sign) are the one uncaught class at this layer — recorded
   as QUESTION 1 with downstream ownership, since no pre-conversion constant
   can exist for them.
4. YES. Independent derivation reproduced
   ad1e4fb286adbc261a2800ab17e4abde5bcd13efb22b150d65ec42b47e2af5fe exactly.
   The source verifier (`verify_pinned_source_files`,
   `checkpoint.rs:616-624`) builds `BTreeMap<name, lowercase-hex>` of the
   observed (revision-side) digests and hashes `serde_json::to_vec` of it;
   the manifest validator (`validate_pinned_inventory`,
   `rank_manifest.rs:133-136`) hashes `serde_json::to_vec` of the manifest's
   `source_file_sha256` `BTreeMap<String, String>`. Workspace `serde_json`
   1.0.149 without `preserve_order` gives sorted-key compact JSON — exactly
   the python `sort_keys=True, separators=(",",":")` representation; names
   are constrained ASCII (`parse_source_manifest` allows only alphanumeric
   and `._-`), so no escaping divergence, and manifest canonicality
   (byte-for-byte round-trip at `rank_manifest.rs:78-81`) pins the embedded
   map bytes to the same encoding.
5. YES. `validate_reconstruction` (`rank_manifest.rs:419-466`) binds codec →
   (reconstruction, source_dtype, source-kind class) exactly: BF16/F16/F32
   row-major must be `byte_exact_source_precision` with source dtype
   `BF16`/`F16`/`F32` and a protected kind; `CODEC_EXL3_SOURCE` must be
   `exl3_tr3_trellis_v0` with `EXL3_TR3_COMPONENTS` and
   `explicit_rank_components`; NVFP4 codecs are rejected outright both there
   and in `validate_top_level`. Descriptor-side, `validate_descriptors`
   (`native_reader.rs:744-823`) binds codec_id to exact logical/stored dtype
   IDs, geometry, and (for EXL3) metadata rank/layer/expert/shape/byte
   equalities; checkpoint-side, protected contracts carry exact
   `SafeDtype` per tensor and EXL3 component dtypes are pinned
   (mcg I32 scalar, suh/svh F16, trellis I16 with exact shape), backed by the
   pinned per-dtype byte inventory. Masquerading a protected tensor as EXL3
   or vice versa trips both the kind class and the pinned contract digest.
6. YES. For `replicated`: axis must be -1 and equal `tp_shard_axis`,
   start=end=0, single self component, and rank == global == source shape.
   For `contiguous_tp_slice`: shard = extent/4 with an explicit
   multiple-of-4 check, start = shard*rank, end = start+shard, rank shape
   re-derived from global shape, source shape == global shape. For
   `explicit_rank_components`: the four `rank{N}` component names are
   re-derived from the tensor name, start/end = rank..rank+1, global shape
   re-derived as rank shape with the shard axis multiplied by 4, source
   shape empty, source dtype pinned. All re-derivations are validator-side
   from rank number and global shape (not copied from the manifest), and the
   passing pinned-inventory test runs them for all four actual plans.
7. YES to exact correspondence; NO, an honest capacity-EXL3 manifest cannot
   fail from drift. Field-by-field comparison of the writer
   (`rank_conversion_manifest`, `glm-cli/src/main.rs:871-971`, and
   `PinnedRankManifestTensor`/`PinnedSourceBinding` in `checkpoint.rs`)
   against the reader's `Raw*` structs shows identical field sets in both
   directions: every field the writer emits is required (no
   ignored-field masquerade channel — `deny_unknown_fields` everywhere), and
   every field the reader requires is emitted (no honest-failure channel).
   All struct fields are declared alphabetically on both sides and both
   sides canonicalize through `serde_json::Value` (BTreeMap), so the tensor
   contract hashes byte-identically. Fixed strings match:
   schema/format/profile/token/repository/toolchain values, the
   `sha256:`-prefixed `CN4_CONVERTER_CONTAINER_DIGEST`, and the flags
   convention (writer spec flags never carry `DESCRIPTOR_FLAG_AUX_REQUIRED`;
   the validator masks it from descriptor flags at `rank_manifest.rs:320`).
   The writer's EXL3 component names (`{stem-with-rankN}.{component}`) equal
   the validator's expectation (`{name minus .weight}.rank{N}.{component}`).
8. YES. `validate_rank_set` (`native_reader.rs:515-560`) compares all common
   identities — conversion UUID, model config, tokenizer bundle, chat
   template, weight policy, kernel ABI, header flags, tensor count,
   per-tensor descriptor semantics and names, decoded codec-metadata
   semantics (deliberately excluding the rank field of EXL3 metadata, which
   is instead bound per-file to the header rank), and manifest consensus over
   profile, conversion commit, operation manifest, budget, review/spec
   hashes, and byte totals — while `manifest_consensus` deliberately omits
   `tensor_contract_sha256`, so the four distinct rank contracts are
   retained (each already bound per-rank to its own pinned constant). The
   four distinct manifest/descriptor/payload hashes enter the conversion-UUID
   recomputation instead of being compared for equality.
9. YES. `native_rank_proof` (`glm-cli/src/main.rs:492-613`) requires: a real
   non-symlink directory containing exactly `rank-0.g5n`..`rank-3.g5n` and
   nothing else; four `NativeRankReader::open` successes (which enforce the
   pinned inventory); `validate_rank_set`; a validated production manifest in
   all four files; `operation_manifest_sha256` equal to the hash of this
   binary's embedded operation manifest; header weight policy equal to
   compiled `pinned_exl3_weight_policy_sha256()`; tensor count equal to
   `PINNED_RANK_TENSOR_COUNT`; kernel ABI equal to compiled `KERNEL_ABI`
   hash; and a full `verify()` payload stream of every rank (with
   change-detection fingerprints before and after) before emitting
   `NATIVE_RANK_SET_PASS`. Rank order is fixed by construction from the
   filenames. The payload digests are verified against the (file-carried)
   header values and reported for operator comparison; they are not bound to
   an engine constant — see QUESTION 1.
10. No bypass or practical exhaustion found beyond MINOR 2. Length-prefixed
    reads are capped (`CONTROL_REGION_MAX_BYTES`,
    `CONTROL_REGIONS_TOTAL_MAX_BYTES`; descriptor region must equal
    count x 192, bounding descriptor/name vectors); EXL3 scratch capacity is
    bounded by bytes actually present in the file and is reported in
    `maximum_reader_scratch_bytes`; the source hasher buffer is clamped to
    8 MiB. Duplicate JSON keys and non-canonical numbers are rejected by the
    byte-exact canonical round-trip; unknown fields by
    `deny_unknown_fields`; future `glmaxx.rank-manifest.*` schemas fail
    closed (`UnsupportedSchema`), other schemas return None and are rejected
    by the production proof; unreviewed profiles fail (`Profile`).
    serde_json's default 128-level recursion limit bounds nesting. All
    offset/size arithmetic I traced uses checked ops
    (`FileRegion::from_header`, `checked_subregion`, `aligned_end`, payload
    sums); region chaining forbids overlap/gap tricks and zero-fills are
    verified. Plain-geometry validation rejects zero extents, so the
    padding-coordinate division cannot divide by zero. No superlinear
    control-path work found: manifest validation is O(bytes), the
    payload-slot scan is a single linear window pass, and streaming is one
    sequential pass.
11. Mostly independent; two fixed-point-only assertions identified. (a)
    `rank_manifest_inventory_and_weight_policy_are_stable` ends with
    `assert_eq!(policy, pinned_exl3_weight_policy_sha256())` — the same
    function twice, a determinism check only; the weight policy digest has
    no engine-external oracle (it is bound file-vs-binary at proof time, so
    a wrong-but-stable policy hash would agree with itself). (b) The four
    contract constants are provenance-fixed-points of the emitter, but as
    analyzed under Q1 the same test semantically re-validates every tensor
    against the validator's independent derivation rules and re-derives byte
    totals, and the external anchors in this review (source manifest, map
    digest, index digest, dtype byte inventory) hold, so the oracle detects
    any emitter change and any semantic rule violation; it could not have
    detected a wrong emitter at the moment the constants were first minted
    if that wrongness also satisfied the semantic checks. The mutation
    fixtures otherwise construct manifests by hand and re-sign, which is
    genuinely independent of the writer.
12. No conflict with either pending gate. Format-ABI r2: its gate-name
    contract explicitly binds the legacy v0.2.2 token and schema naming to
    the v0.2.3 format hash 619a3923... — exactly the `spec/format-v0.md`
    pinned here; the operation-manifest hash 8a5f5488... matches; the
    candidate's `REVIEW_ACCEPTANCE_TOKEN`, schema string, and `g5n-v0.2.2`
    format tag follow that contract. Checkpoint-load-transaction r2: its
    stated correction (rank-specific `tensor_contract_sha256`, four digests
    bound individually, a separate normalized semantic catalog for
    consensus) is exactly what this implementation provides
    (`ValidatedRankManifest.tensor_contract_sha256` exposed per rank,
    consensus excluding it). That handoff pins older hashes of
    `checkpoint.rs`/`native_reader.rs` at its own candidate commit, so its
    review must be re-anchored to the current bytes — a sequencing note, not
    a contract conflict. The rank-manifest schema itself is not described in
    `spec/format-v0.md`; nothing there contradicts it.

## Five acceptance statements

- The fixed complete four-rank tensor inventory is accepted: YES.
- The independently derived full source-map identity is accepted: YES.
- Re-signed semantic divergence now fails closed: YES (every semantic policy
  field is bound by the engine-owned contract or identity checks; the
  non-semantic payload-content residual is recorded as QUESTION 1 with
  downstream ownership).
- Writer/reader and four-rank identity handling are accepted: YES.
- This CPU validation may enter checkpoint-load implementation only after
  that design's own r2 token is present: YES (the candidate contains no
  load-transaction implementation; that gate remains pending and its token
  is a precondition).

## Architecture & maintainability

The validation stack is well layered: `checkpoint.rs` owns the immutable
engine-side truth (hard-coded protected shape tables, EXL3 enumeration,
pinned counts and byte inventories, plan and manifest-tensor derivation);
`rank_manifest.rs` owns the canonical-JSON manifest contract with the pinned
per-rank digests and map identity; `native_reader.rs` owns file-format
paranoia (fingerprints, hard-link/symlink rejection, canonical layout,
zero-gap verification, bounded streaming) and four-rank consensus; the CLI
composes them into proofs without duplicating rules. Fail-closed error enums
are total and non-lossy, arithmetic is uniformly checked, and the
canonical-JSON strategy (byte-exact round-trip plus alphabetical struct
fields on both sides) is a simple, robust equivalence — though it silently
depends on serde_json's sorted-map default; a comment or a compile-time guard
against enabling `preserve_order` would cheapen future maintenance. The main
structural cost is triple maintenance of the tensor schema (Pinned* writer
structs, Raw* reader structs, TensorDescriptor cross-checks); the pinned
digests would catch drift, but a shared field-list definition would reduce
churn. Test hygiene is strong (mutation fixtures, canonical-parser
negative cases, publisher-exception exactness), with the two fixed-point
assertions noted in Q11 as the residual weak spots.

## Token decision

Provenance clean: all 11 input hashes verified at start and finish with no
drift, worktree HEAD pinned, review-proof PASS, external source-map anchor
independently reproduced, tests 63/63. Zero BLOCKER and zero MAJOR findings;
all five acceptance statements are an unqualified YES. The v2 acceptance
token is emitted below. Per the handoff, it accepts only the CPU manifest
validator: it does not accept the pending format-ABI or load-transaction
gates, authorize cn4, approve full conversion, or establish checkpoint
startup, quality, or speed.

production-rank-manifest-validation-v2-accepted
