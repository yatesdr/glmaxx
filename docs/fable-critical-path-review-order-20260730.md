# Fable critical-path review order

Date: 2026-07-30

Purpose: prioritize the existing review queue by implementation-unblock value.

This is coordination metadata, not an acceptance artifact. Every review must
still use its own handoff, candidate commit, input hashes, output path, scope,
and exact token contract. Review moving `main` only to discover the handoff;
perform substantive review in a detached worktree at the pinned candidate.

The complete queue remains
`docs/fable-review-queue-all-20260730.md`. This document does not supersede or
remove any row from it.

## P0 — repair already-substantive kernel verdicts

These three r2 review texts report substantive passes but fail machine
acceptance only because their attestations omit pinned inputs. Follow the
exact repair packet:

```text
docs/fable-kernel-r2-attestation-repair-request.md
```

Repair in this order:

1. `docs/fable-manifest-abi-v022-r2-handoff.md`
2. `docs/fable-exl3-source-projection-v1-r2-handoff.md`
3. `docs/fable-exl3-warp-decode-v2-r2-handoff.md`

Do not merely append a token. Each repaired result must attest every candidate
input at review start and finish and preserve the substantive finding record.
These repairs unblock the current kernel source/format baseline.

## P1 — direct-tier implementation chain

The broad `direct-tier-io-v1` design is already machine-accepted. The
following exact reviews unlock the first real Linux fixed-buffer/file I/O
implementation.

Reviews 1–4 can run in parallel:

1. `docs/fable-direct-tier-extent-cpu-v1-handoff.md`
2. `docs/fable-direct-tier-state-cpu-v1-handoff.md`
3. `docs/fable-direct-tier-durable-format-v1-handoff.md`
4. `docs/fable-direct-tier-scheduler-cpu-v1-r3-handoff.md`

Reviews 5 and 6 are sequential implementation deltas:

5. `docs/fable-direct-tier-checksum-authority-cpu-v1-handoff.md`
6. `docs/fable-direct-tier-checksum-workers-cpu-v1-handoff.md`

The Linux design review can run in parallel with all six, but implementation
cannot start until its own design token and the dependencies named by that
design are machine-accepted:

7. `docs/fable-direct-tier-linux-probe-v1-handoff.md`

Required outcome before Linux implementation:

```text
direct-tier-extent-cpu-v1-accepted
direct-tier-state-cpu-v1-accepted
direct-tier-durable-format-v1-design-accepted
direct-tier-scheduler-cpu-v1-r3-accepted
direct-tier-checksum-authority-cpu-v1-accepted
direct-tier-checksum-workers-cpu-v1-accepted
direct-tier-linux-probe-v1-design-accepted
```

The checksum-worker token is a post-design dependency added by implementation
progress. Even though the older Linux design could not name it, production
work must not bypass the fixed checksum authority now present on `main`.

## P2 — executable SM120 model path

After the P0 attestation repairs, review the following in this dependency
order:

1. `docs/fable-sm120-rank-executor-v1-r2-handoff.md`
2. `docs/fable-step-execution-abi-v3-handoff.md`
3. `docs/fable-exl3-warp-staging-cpu-v2-handoff.md`
4. `docs/fable-nvfp4-fused-routed-moe-v1-r3-handoff.md`
5. `docs/fable-nvfp4-laboratory-manifest-v1-handoff.md`
6. `docs/fable-hybrid-serving-manifest-v1-handoff.md`
7. `docs/fable-small-checkpoint-runner-v1-r2-handoff.md`
8. `docs/fable-tp4-layer6-replay-v1-handoff.md`

Items 1–4 establish the executor and both direct weight paths. Items 5–8
freeze fit-capable checkpoint posture and progressive execution gates. A
design token is not device evidence and does not authorize cn4.

## P3 — cache transactions and production transport

Once P1 is moving, prioritize:

1. `docs/fable-fixed-page-transaction-v1-r2-handoff.md`
2. `docs/fable-cache-arena-budget-v2-r2-handoff.md`
3. `docs/fable-online-prefix-publication-v1-r2-handoff.md`
4. `docs/fable-rank-residency-content-identity-v1-r2-handoff.md`
5. `docs/fable-restore-operation-quota-v1-r2-handoff.md`
6. `docs/fable-nonblocking-http-transport-v1-r2-handoff.md`
7. `docs/fable-coordinator-api-backend-v3-handoff.md`

These reviews unblock fixed page-mirror implementation, online durable prefix
publication, bounded restore ownership, and the real nonblocking HTTP
transport.

## Machine verification

After writing any result into its declared path, run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-acceptance-lint HANDOFF STAGED_REVIEW
```

For the complete operator-owned staging directory:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-acceptance-lint-all docs/reviews \
  /tmp/glmaxx-review-staging.json
```

The batch command intentionally exits nonzero while any staged artifact is
rejected. It does not install, move, rewrite, or accept reviews.

Before promoting any result into its handoff-declared path, run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof-all . /tmp/glmaxx-review-provenance.json
```

Current verified baseline at `22a152c`:

```text
122 handoffs
104 configured result paths
8 machine-accepted results
0 machine-withheld installed results
```

The operator-owned `docs/reviews/` directory is not a Git input and must not
be modified, staged, committed, or deleted by implementation work.
