# Cache lifecycle CPU proof v1

Date: 2026-07-29

Status: implemented CPU/reference proof

GPU authorization or evidence: none

## Purpose

This proof joins the previously separate durable-store, prefix-index,
residency, and active-page-table oracles into one fail-closed lifecycle. It
addresses the CPU-testable portion of production punchlist K05 without
claiming that byte-owning host simulation is qualified GPU, `io_uring`, or
NVMe execution.

Run:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  cache-lifecycle-proof <fresh-external-evidence-directory>
```

The directory must not already exist. It contains the temporary 5.9-MiB tier
store and `cache-lifecycle-proof.json`; it must remain outside Git.

## Proved lifecycle

The command:

1. derives three chained 64-token prefix keys in one immutable namespace;
2. publishes three target-KV, target-indexer, and unified token-major draft
   sidecars through the file-backed journal;
3. appends a 113-byte torn trailing journal fragment;
4. closes and reopens the store, recovering exactly the three fully durable
   generations and no partial record;
5. rebuilds the prefix index and restores a 192-token MTP-capable match;
6. confirms that DCP1 and DCP4 metadata postures reuse the same content
   namespace and page keys—the proof does not authorize DCP1 execution;
7. exercises a one-request restore bound and observes deterministic
   saturation;
8. constrains HBM and DRAM to one page each and drives pages through
   HBM→DRAM→NVMe pressure;
9. pins the sole HBM victim and proves that another completion fails closed
   without evicting it;
10. admits the sealed prefix into the active page table, shares full pages,
    copy-on-writes a mutable tail, rolls back seven tentative positions, and
    commits only three of a later seven-position reservation;
11. removes every sequence and proves target/draft page accounting returns to
    zero; and
12. corrupts a durable target-KV byte, observes the asynchronous restore
    checksum error, aborts the restore transaction, and proves the corrupt
    page remains NVMe-only.

The exact deterministic fixture is
`fixtures/cache-lifecycle-proof-v1.json`. `scripts/local-checks.sh`
regenerates it in a fresh temporary directory and compares every byte.

## Scope boundary

This is stronger than isolated unit tests but remains a CPU metadata and
byte-store proof. It does not establish:

- pinned host memory or asynchronous CUDA transfers;
- direct-I/O alignment at the actual filesystem/device boundary;
- `io_uring`, registered-buffer, cancellation, or cleaning behavior;
- HBM allocation or page-table upload on SM120;
- model attention correctness from restored KV;
- 1,048,576-token live execution;
- concurrent cache-thrash latency isolation;
- accepted online-publication or direct-tier-I/O contracts; or
- cold/warm model-serving performance.

Accordingly K05 remains open until the same lifecycle runs through the real
rank executor and model. This proof is the deterministic CPU fixture that the
later GPU/host/NVMe implementation must preserve.
