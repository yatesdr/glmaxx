# cn4 GLMAXX experiment isolation v1

Date: 2026-08-03

Status: active operational contract

## Boundary

All GLMAXX activity on cn4 belongs under `/home/derek/glmaxx`. Production
vLLM worktrees, environments, images, containers, volumes, caches, ports,
shared-memory objects, checkpoints, and result directories are not GLMAXX
scratch space. They may be inspected read-only when an admitted input or
matched control requires it, but they are never mounted writable, reused,
renamed, stopped, cleaned, or incorporated into a GLMAXX result by pathname.

The model checkpoints remain read-only inputs at their existing paths. A
checkpoint path is not a workspace and no sidecar, lock, conversion, cache, or
verification marker may be written beside it.

## Directory and name ownership

Use only these GLMAXX-owned trees:

```text
/home/derek/glmaxx/worktrees/<full-commit>/
/home/derek/glmaxx/build/<run-id>/
/home/derek/glmaxx/cache/<tool-or-purpose>/
/home/derek/glmaxx/tmp/<run-id>/
/home/derek/glmaxx/evidence/<run-id>/
```

`run-id` is
`YYYYMMDDTHHMMSSZ-<gate>-<short-commit>-r<attempt>`. Every path is resolved
before use and must remain below `/home/derek/glmaxx`; symlinks escaping that
root fail closed. A pre-existing run, build, or temporary path is never
overwritten. A failed or malformed attempt is retained and a new attempt
number is used.

Docker images, containers, volumes, networks, IPC objects, PID files, NVTX
ranges, and local service names use a `glmaxx-<run-id>` prefix. Services bind
only a checked-free GLMAXX port and record it. Cleanup is permitted only when
the recorded owner identity, process start time or container ID, and run ID
all match. Global `/dev/shm`, CUDA, Docker, NVMe, or process cleanup is
forbidden.

## Evidence record

Each run directory contains these logical sections, even when a section is
empty because the run failed before reaching it:

```text
provenance/  immutable inputs and environment identity
raw/         stdout, stderr, metrics, profiler output, and exit statuses
derived/     machine-readable summaries produced from raw inputs
binaries/    hashes or retained GLMAXX binaries used by the run
```

The record captures at minimum:

- full source commit, tracked status, and dirty patch hash;
- exact command, environment allowlist, config, and per-rank launch plan;
- container digest, executable/library hashes, Rust/CUDA/CUTLASS versions;
- checkpoint revision, index/config/tier-map hashes, and every read shard;
- tokenizer, template, prompts or token IDs, datasets, seeds, and quality
  posture;
- GPU UUIDs, PCI topology, driver, clocks, power limits, memory, and processes
  before and after;
- rank routes, collective posture, MTP depth, KV format/capacity, cache state,
  prefix state, and cold/warm classification;
- start/end UTC timestamps and every command exit status; and
- raw-to-derived input hashes and the schema version of each summary.

Cold prefill uses unique prompt identities. Prefix reuse is a separate warm
result. Cold boot and resident-weight reload are different experiments and
must report storage reads and H2D bytes independently. A failed sample remains
in the ledger; it cannot be silently replaced by a passing rerun.

`evidence.sha256` is generated last over every retained file in canonical
relative-path order. Its own SHA-256 is the concise result's external artifact
identity. Once sealed, a run directory is immutable; corrections use a new
attempt and explicitly supersede, but do not delete, the old record.

## Git boundary

Raw or bulky evidence never enters Git. A concise checked-in result records
the run ID, external path, evidence-manifest hash, all decisive input hashes,
exact commands, pass/fail/nonclaims, and any superseded attempts. Performance
claims additionally retain every sample and name the matched control fields.

This contract organizes evidence; it does not accept a checkpoint, kernel,
quality result, capacity result, or speed result.
