# Fable handoff: EXL3 mixed-K source and kernel contract v1 r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or run CUDA for this review.

Review candidate commit:
`23e6e26c172e370b63608a07aa2f781a24faef24`

Required result path:
`fable-exl3-mixed-k-source-and-kernel-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`exl3-mixed-k-source-and-kernel-v1-r2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r2.md` | `38b9ae4c9282c1550cc20fa7b8f4ac35fe38eb0fd3ea7e54c4a15a4c6cd741fe` |
| `docs/cn4-tr3-tier-boundary-20260803.md` | `325f52710787eaf0eae15ceaa0bcb80310a7b4ca018d397b4c3ce1da139a1623` |
| `scripts/cn4-tr3-tier-boundary.sh` | `b45c7bd75680e059bb1fa9d7e24931f2625a74f028d07db3b538517354668687` |
| `docs/exl3-mixed-k-source-and-kernel-v1.md` | `96d6ce4efc5782f0c1e3da613f4e067d1526e2eef6610196a2caaa543b866df9` |
| `docs/fable-exl3-mixed-k-source-and-kernel-v1-handoff.md` | `d36be01955a50f61298a85e6d75bb9d2e6257afa95a539ff33ecf594177e8248` |
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

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

The external cn4 record is discovery evidence only. Its documented 16-entry
manifest hash is not a substitute for publisher authentication, and this
review must not promote it to checkpoint admission evidence. The historical
shell probe canonicalizes `GLMAXX_TR3_DIR` before testing `-L` and parses JSON
with `jq`; it therefore does not prove rejection of an originally symlinked
source-directory argument or duplicate JSON keys. Those are disclosed probe
limitations, not properties that the reviewed Rust implementation may inherit.

## Required decisions

Independently derive the layer boundary, tensor counts, source-plane sizes,
and correction delta. Inspect the pinned code for every current three-bit-only
boundary. Then answer:

1. Does the r2 candidate unambiguously supersede the first design and prevent
   its handoff or token from opening implementation?
2. Are target sparse layers exactly 3 through 77, with 75 layers, while layer
   78 is the separate recurrent draft layer?
3. Does the pinned metadata proof exhaustively require 192 K3 and 64 K4
   experts on every target layer, while requiring all 256 draft experts and
   all 3,072 draft trellis descriptors to be K3?
4. Are the exact target counts 172,800 K3 and 57,600 K4 trellis tensors, with
   exact draft counts 3,072 K3 and zero K4?
5. Are 5,662,310,400 bytes of K4 delta and 75,293,233,152 complete routed
   source-plane bytes per rank independently correct, without treating 3.25
   bpw or an average width as allocation authority?
6. With the disclosed discovery-only limitations above, does the proof script
   fail closed on unexpected raw index/tier identities, layer sets, tier
   schema, width/count census failures, invalid header lengths or JSON,
   direct index/tier/shard symlinks, and evidence-path reuse while reading
   headers only; and does the contract correctly require the future Rust
   implementation to provide complete safe-path, unique-key, and descriptor
   validation instead of inheriting the shell probe as admission authority?
7. Are the script, result record, and r2 contract explicit that raw hash
   pinning is not publisher-manifest authentication and cannot admit a
   checkpoint or authorize conversion?
8. Does the amended CPU-proof matrix independently cover target K3, target
   K4, and draft K3 across both edge ranks and all projections, including
   schema/membership mutation and checked accounting failures?
9. Does descriptor-derived width plus complete authenticated tier consensus
   prevent caller-controlled precision selection, rank-local disagreement,
   and silent target/draft substitution once implemented?
10. Does one outer dispatch into separate compile-time K3/K4 kernels keep the
    width branch outside the weight loop and make any draft K4 plan fatal?
11. Do canonical partition hashing, token/slot order, collective routes, and
    fallback rules preserve identical TP4 decisions across ranks?
12. Does the proposed replay/timing matrix separately expose target K3,
    target K4, target binning, draft K3, collective, and framework costs
    before any MTP or throughput claim?
13. Does the gate sequence preserve design review, CPU proof, implementation
    review, SM120 microbenchmark, target/draft layer replay, authenticated
    checkpoint smoke, MTP0 quality, and only then MTP3?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer every decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all seventeen exact input hashes, then end
with the requested token as the only bare acceptance line.

Acceptance opens only the target/draft tier parser, accounting, and CPU proof.
It does not accept that implementation, authenticate the checkpoint, allow
conversion, accept a CUDA launch, or establish layer, quality, capacity, MTP,
or performance evidence.
