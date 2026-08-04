# Native checkpoint-load timing v2

Date: 2026-08-04

Status: host instrumentation candidate; independent review and real rank-image
execution pending

## Scope

The checkpoint-load report becomes
`glmaxx.sm120-tp4-native-checkpoint-load-smoke.v2`. It preserves every v1
identity, byte count, receipt, cleanup check, and explicit
`model_kernel_launched=false` boundary. It adds measurements only; it does not
alter loading, verification, adoption, memory allocation, or cleanup.

## Host partition

`load_native_checkpoint` captures adjacent monotonic-clock boundaries for:

1. configuration, memory-plan, and linked-codec validation;
2. four rank-reader opens plus synthetic-identity preflight planning;
3. four persistent rank workers, their retained rank readers, CUDA contexts,
   and streams;
4. worker-observed device-identity consensus;
5. the final device-bound load plan;
6. the four-rank weight load, readback, and global adoption transaction; and
7. rank evidence collection and receipt consensus.

The seven non-overlapping terms are checked-additive. Their sum must not
exceed the internally measured total; overflow or a negative remainder fails
the command. The report retains the independently measured outer call time so
instrumentation and caller overhead remain visible.

The command also reports its rank-set preflight, budget/manifest preflight,
and executable-identity/evidence-setup phases. Its command-total clock begins
at entry, before those phases, and ends after rank cleanup during report
assembly. Final JSON serialization and file publication remain outside this
load-only total and cannot be mislabeled as production health publication.

Per-rank evidence separately retains storage-read, host-to-pinned copy, H2D
submission, H2D drain, and full-arena-readback nanoseconds. Those rank terms
overlap across four worker threads and therefore are not added to the host
partition.

Unimplemented collective, graph, KV, and production-health phases remain JSON
`null`, and the coverage string names them explicitly. A zero must never
stand in for an unimplemented phase.

## Gate

CPU/type checking must prove checked phase arithmetic, the v2 schema, and
unchanged v1 success/cleanup conditions. The first real run must use four
authenticated rank images and retain all raw phase values. It is load-only
evidence and cannot establish model execution, cold-boot superiority,
quality, capacity, latency, throughput, or serving.
