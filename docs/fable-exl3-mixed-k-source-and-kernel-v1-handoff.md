# Fable handoff: EXL3 mixed-K source and kernel contract v1

Date: 2026-08-03

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or run CUDA for this review.

Review candidate commit:
`849c1d12bf42d92aecffe9003530a2a13dcc3dfe`

Required result path:
`fable-exl3-mixed-k-source-and-kernel-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`exl3-mixed-k-source-and-kernel-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/exl3-mixed-k-source-and-kernel-v1.md` | `31aa9ee83958dd46d489777824122b8b63cd762bb69520574bc035804f1e5f71` |
| `docs/cn4-tr3-mixed-k-proof-20260803.md` | `8b67fabc9d1222f9ca5734cff11be0afd611730e5f424186aadf876516e1b141` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/stream.rs` | `b6d7dae8adf6fbb7ebd0f08c79c3d7f9dbba6269408f6b760fa43b18028a22fb` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required decisions

Independently rederive the K=3 and K=4 shape and byte rows, plus the stated
per-rank delta. Inspect the pinned implementation to locate every current
3-bit-only boundary. Then answer:

1. Does deriving width from the exact trellis descriptor and independently
   cross-checking the authenticated tier map eliminate caller-controlled or
   rank-local precision selection?
2. Does the full gate/up/down and rank-0-through-3 consensus rule fail closed
   on every missing, extra, unsupported, or inconsistent tier membership?
3. Is expanding the existing wire-v1 `bits` value set to exactly `{3,4}` safe
   and unambiguous while advancing the device ABI to v2?
4. Are all K=3/K=4 component and aggregate byte figures exact, checked, and
   sufficient to prevent the 3.25 average from becoming allocation authority?
5. Does the diagnostic/production split preserve mandatory immutable source,
   manifest, index, tier-map, and payload authentication without introducing
   an exception for the observed checkpoint?
6. Does the CPU matrix independently prove both bit mappings, real tensor
   reconstruction, tier consensus, metadata round trips, and malformed-input
   rejection before CUDA work?
7. Does a one-time validated dispatch into separate compile-time K=3/K=4
   specializations keep the width branch out of the weight loop and bind exact
   trellis geometry on both Rust and CUDA sides?
8. Is deterministic width binning compatible with canonical token/slot order,
   identical TP4 collective routing, and an absolute ban on rank-local
   fallback?
9. Do the kernel and end-to-end benchmark controls preserve precision
   membership, batch, context, routing, and cache posture while separating
   kernel, binning, collective, and framework costs?
10. Are the cn4 evidence and isolation claims narrowly recorded as discovery
    evidence rather than treated as production admission or GPU acceptance?
11. Does the gate sequence obey the repository contract and keep checkpoint
    smoke, quality, and performance blocked until their prerequisites pass?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer every decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all thirteen exact input hashes, then end
with the requested token as the only bare acceptance line.

Acceptance opens only CPU mixed-K implementation and proof. It does not
accept that implementation, the current checkpoint manifest, native
conversion, any CUDA launch, quality, capacity, or performance result.
