# Direct-tier scheduler CPU proof v1 r3

Date: 2026-07-30

Status: consolidated corrective CPU implementation passed focused checks;
adversarial review required

GPU claim: none

Storage claim: no syscall, filesystem, or device evidence

## Superseded candidates

This candidate supersedes scheduler v1 at `6cdbeae` and r2 at `e188fc7`.
Neither has an acceptance result.

The r2 correction prevents admission receipts from starving useful service,
but a second self-adversarial pass found two remaining fairness holes:

1. configuration did not relate maximum read-command bytes to the W0
   admission/service byte bounds; a single oversized R0/R1 command could
   force every W0 ahead of it, or make the bound impossible to honor; and
2. strict R0-before-R1 selection allowed continuous resumed-request traffic
   to starve prefix/admission restores indefinitely.

Those are policy defects, not storage or CUDA findings. The r3 correction
consolidates them with every earlier scheduler invariant.

## Read-size contract

`DirectIoSchedulerConfig` now carries:

```text
maximum_read_command_bytes
maximum_resume_read_bytes_before_admission_read
maximum_read_bytes_before_publication_admission
maximum_read_bytes_before_publication_service
```

Construction requires:

```text
maximum_read_command_bytes > 0
maximum_resume_read_bytes_before_admission_read
    >= maximum_read_command_bytes
maximum_read_bytes_before_publication_admission
    >= maximum_read_command_bytes
maximum_read_bytes_before_publication_service
    >= maximum_read_command_bytes
```

R0/R1 insertion rejects any command above
`maximum_read_command_bytes` before queue bytes, IDs, order keys, or heaps
change. W0/W1 byte fields remain policy accounting values and are not
misclassified as reads.

This makes all three projected fairness bounds satisfiable by one complete
read operation. Production configuration must later bind the maximum to the
largest accepted physical extent; this CPU gate does not claim that runtime
configuration wiring.

## Bounded R0 preference

R0 retains its lower queue-latency priority, but no longer has unbounded
priority over R1. While any R1 waits, the scheduler counts only serviced R0
bytes:

```text
resume_read_bytes_since_admission_read_waited
```

R1 runs when:

- no R0 is waiting;
- the counter equals the configured bound; or
- the next deterministic R0 would cross the bound.

The counter resets when the first R1 begins waiting and after every R1
service. It remains zero while no R1 waits. W0 admission/service and W1
service do not falsely consume the R0 byte allowance.

The read selected by this policy is also the read projected by both W0 byte
checks. Therefore W0 fairness cannot be calculated against an R0 command
when the next actual service would be R1.

## Preserved r2 invariants

The following remain unchanged:

- fixed deterministic R0/R1/W0/W1 heaps and exact in-class order;
- fallible preallocation and 65,536-command hard cap;
- duplicate/order/overflow rejection without partial insertion;
- service-before-admission and one admission between service decisions;
- independent new-W0 and accepted-W0 read-byte bounds;
- one-buffer/one-descriptor/two-CQE W0 reservation;
- complete read-reserve preservation;
- W1 read/publication low-watermark isolation; and
- final zero accounting.

## Focused proof

The scheduler suite now contains 18 tests. New checks cover:

- rejection of each configuration whose R0/R1 or W0 bound is smaller than
  the maximum read command;
- atomic rejection of an oversized read;
- two 100-byte R0 services followed by R1 exactly at a 200-byte bound; and
- a 150-byte R0 followed by R1 before a 60-byte R0 would cross the same
  bound.

The overflow regression uses a separate valid `u64::MAX` configuration, so
queued-byte overflow remains reachable and mutation-visible without weakening
ordinary read-size enforcement.

Focused commands pass:

```text
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

`scripts/local-checks.sh` then passes with 396 Rust tests, formatting,
warning-denied workspace Clippy, CUDA-FFI host checks, deterministic
format/cache/engine/serving fixtures, and provenance for all 110 then-present
handoffs with 8/92 configured results accepted and zero withheld. The pinned
tokenizer proof is skipped because its configured directory is absent, and
CUDA compilation is skipped because this host has no `nvcc`.

## Acceptance boundary

Acceptance covers the consolidated deterministic CPU scheduling policy and
the named tests only.

It does not accept:

- production binding of the physical extent maximum;
- the authoritative resource ledger, terminal replenishment, or scheduled
  command cancellation;
- io_uring, registered memory/files, direct I/O, CQEs, fsync, async cancel,
  eventfd, filesystem, or target-device behavior;
- durable codec/recovery, online publication, or cleaning;
- CUDA, HBM transfer, KV reconstruction, attention, or model execution;
- capacity, decode isolation, latency, throughput, or health; or
- cn4 access.
