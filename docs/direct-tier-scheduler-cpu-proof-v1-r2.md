# Direct-tier scheduler CPU proof v1 r2

Date: 2026-07-30

Status: corrective CPU implementation passed focused checks; adversarial
review required

GPU claim: none

Storage claim: no syscall, filesystem, or device evidence

## Superseded candidate

This correction supersedes direct-tier scheduler candidate
`6cdbeae417e053d08751c8102304064bf86c360e` and its v1 review request.
The v1 implementation was locally green but had a decision-order defect
found during self-adversarial review:

```text
publication admission
publication admission
...
ready read or already-admitted W0 service
```

Because admission was checked before accepted-W0 service and had no
between-admission latch, a caller presenting multiple eligible W0 candidates
and ample shared resources could receive consecutive admission receipts.
A bounded queue made the delay finite, but it violated the intended
low-latency read posture and the accepted-W0 bounded-completion contract.

No v1 acceptance result exists. Fable must not issue its token.

## Correction

The scheduler now:

1. evaluates due accepted-W0 service before new-W0 admission;
2. records `publication_admitted_since_service=true` after one admission;
3. refuses another admission while that latch is set;
4. clears the latch on every R0, R1, W0, or W1 service decision; and
5. exposes the latch in `DirectIoSchedulerStats` for mutation-visible
   accounting.

Consequences:

- with no reads, `admit W0-A -> service W0-A` occurs before W0-B admission;
- with reads below the high watermark,
  `admit W0-A -> service R0/R1` occurs before another admission;
- once the accepted-W0 byte bound is due, W0 service wins over another
  candidate even if that candidate is also admission-due; and
- normal one-for-one admission/read alternation remains possible when shared
  resources exist, without allowing admission receipts themselves to starve
  useful service.

The prior exact ordering, byte projections, read reserves, two-CQE W0
reservation, cleaner watermarks, fallible construction, and 65,536-command
hard cap are unchanged.

## Focused proof

The focused scheduler suite now contains 17 tests. The new regression holds
two W0 candidates, one ready R0 command, and enough shared resources to expose
the old behavior. It requires:

```text
admit W0-A
service R0
service W0-A
admit W0-B
service W0-B
```

It also checks the latch after admission and service. Removing either the
service-before-admission order or the latch fails the regression.

The complete focused commands pass:

```text
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

`scripts/local-checks.sh` then passes with 395 Rust tests, formatting,
warning-denied workspace Clippy, CUDA-FFI host checks, deterministic
format/cache/engine/serving fixtures, and provenance for all 109 then-present
handoffs with 8/91 configured results accepted and zero withheld. The pinned
tokenizer proof is skipped because its configured directory is absent, and
CUDA compilation is skipped because this host has no `nvcc`.

## Acceptance boundary

Acceptance covers the corrected CPU scheduling policy and the original v1
CPU boundary only.

It still does not accept or implement:

- an io_uring authority or authoritative resource ledger;
- terminal W0 resource replenishment or scheduled-command cancellation;
- registered buffers/files, direct I/O, CQEs, fsync, async cancel, or
  eventfd;
- the durable codec, recovery, online publication, or cleaner;
- CUDA, HBM transfer, KV reconstruction, attention, or model execution;
- storage-device behavior, capacity, latency, throughput, or health; or
- cn4 access.
