# Fable adversarial re-review — engine/format v0.2.0

Date: 2026-07-28

Reviewer: Fable (claude-fable-5)

Prior review: `fable-adversarial.md`, SHA-256
`4a6e6e36d2f226f8fadc6135b910281db1aa9e5d11cded3b6bd43e287e975137`
(left unmodified to preserve the disposition's provenance chain).

Reviewed bytes, hashed at start and finish of this re-review (both match
the v0.2 handoff candidates):

| File | SHA-256 |
|---|---|
| `spec/engine-v0.md` (0.2.0) | `15c8a9033420828307ebc10d472c70ed739e611f4ed0308b88c42972be1666a3` |
| `spec/format-v0.md` (0.2.0) | `0e56b4b0cb1c4a64b666022a9d43c4ebc48a4609503d72e25b7526032a19fac6` |
| `docs/native-engine-plan.md` | `91950cea58ca2ff4f64f3588897b7f130288828b87cd34789f5e0bdb825787d1` |

## 0. Corrections accepted, and what was re-verified

**Sol's correction to my B1 arithmetic is right and I accept it.** I wrote
0.53125 B/param for 1D NVFP4; the correct figure is 8 value bytes + 1 scale
byte per 16 values = 9/16 = **0.5625 B/param** (4.5 bpw). My error
understated the infeasibility by ~21 GiB; the corrected 379.6875 GiB makes
the original conclusion strictly stronger. Noted for the record so the
wrong constant doesn't propagate from my earlier document.

Re-verified exactly in v0.2 (all pass):

- §8.1: 3 × 6,144 × 2,048 × 256 × 75 = 724,775,731,200; 337.5 + 42.1875 =
  379.6875 GiB; 384 − 379.6875 = 4.3125 GiB; hypothetical 2D bound
  340.136719 GiB; lower-bound table sums to 398.813243 GiB.
- Draft sidecar: 368 × 1,048,576 = 385,875,968 B = 0.359375 GiB;
  0.08984375 GiB/rank; MTP-capable total 28.390625 GiB (7.09765625/rank);
  sealed draft record 4,096 + 23,552 + 1,024 = 28,672 = exactly 7 × 4 KiB.
- Revised tier header offsets tile correctly with the paired-key field
  (244 + 32 = 276; 276 + 3,820 = 4,096).
- §5.1 UUID derivation has no circular dependency (region hashes exclude
  the header; `header_crc32c` computed last; manifests forbidden from
  containing the UUIDs).
- §14.2 LSE merge algebra is the standard exact flash-decoding combine;
  the (score desc, position asc) merge is deterministic because each
  position is scored by exactly one owner.
- Plan is now consistent with the specs (state machine includes
  `HBM_TENTATIVE`/`INVALID`; no `speed-nvfp4` references; figures match).

## 1. Findings

### BLOCKER

None.

### MAJOR

**V2-M1 — The sparse indexer's key data has no home in any contract, yet
§14.2 step 2 depends on it.**
Cites: `spec/engine-v0.md` §14.2 (step 2), §8/§8.2, §16;
`spec/format-v0.md` §20–§21 (absence).

Step 2 requires each DCP owner to "compute sparse-index scores only for
its owned committed positions." Scoring requires per-position indexer key
features (32 index heads × 128 dims per the §4 constants). The 368-byte
record contains only the NoPE latent, scales, and RoPE payload — no
indexer keys. So the engine must either (a) maintain a **separate
indexer-key cache**, which appears nowhere in the KV ABI, page geometry,
tiering records, prefix keys, or the §8.2 budget inequality — and which is
material: e.g. a 128-byte FP8 key per position per refresh group is
GiB-scale at 1M and must survive eviction/restore identically to the
records it scores, or (b) **recompute keys from the decompressed latent**
each refresh, which is a per-step GEMM over every owned committed position
(at 1M context, a nontrivial recurring cost that belongs in the decode
phase ledger and the operator-inclusive timing rule). Either answer is
legitimate; having no answer is not, because §14.2 is normative and
M3-gated. Required: extend OPEN items 1 and 4 to name the indexer-key
representation, precision, residency, tiering/restore behavior, and its
term in the §8.2 inequality; verify which mechanism the pinned reference
and the existing glm52-opt stack actually use before choosing.

**V2-M2 — DCP4-always imposes a per-layer four-rank collective chain on
*every* decode step, including short contexts where DCP buys nothing.**
Cites: `spec/engine-v0.md` §2 (DCP4 process-immutable), §14.2, §17
(`j mod 4` striping).

Cutting DCP1/DCP2 resolved my M3 cleanly, but the cost surfaced elsewhere:
with pages striped mod 4 from ordinal 0, a 200-token sequence already has
owners on multiple ranks, so every decode token pays query all-gather +
candidate exchange (per refresh group) + partial-state return, serialized
across 78 layers. At PCIe Gen3 small-payload latencies this is a
milliseconds-per-token floor added to the *most common* serving regime
(short/medium context), where single-owner attention would need none of
it. Two rank-invariant mitigations should be named in the spec now, and
the M3 exclusive phase ledger must isolate this chain as its own line:

1. the coordinator MAY plan an **owner-subset schedule** in the
   `StepPlan` — owners with zero committed pages for every sequence in the
   step are omitted from that layer's exchange (deterministic, since the
   coordinator owns the page tables);
2. candidate exchange is already amortized by IndexShare (every 4 layers);
   confirm query gather and partial return can batch across the
   IndexShare group where the manifest permits.

If the measured chain still dominates short-context inter-token latency at
M3, the fallback is an ownership mapping that keeps the first N pages on
one rank before striping — that is exactly the kind of change the
attachment-ABI split (M6 resolution) was designed to make cheap, which is
an argument for measuring before freezing OPEN item 7's mapping.

### MINOR

**V2-m1 — §8.1's "optimistic lower bound" is not strictly a lower bound.**
The table prices the remaining 28.224B parameters at 1 byte each, but a
hostile reader can price them at NVFP4's 0.5625 B/param (≈15.9 GiB) or the
2D floor (≈14.2 GiB). The conclusion survives — the total is still
≈386.7–387.3 GiB, above nominal HBM before contexts/modules/workspaces —
but the proof should state the true floor so the infeasibility claim is
airtight. One-line fix: add "even at 0.5625 B/param for every remaining
parameter, the total exceeds 386 GiB."

**V2-m2 — §8.2 inequality omits named terms.** Immutable model metadata
(planner item 2), KV page tables (item 9's second half), and the V2-M1
indexer-key store (if cached) have no explicit term; they can only hide in
`allocator_padding_bytes`. Add explicit terms — the whole point of the
inequality is that nothing hides.

**V2-m3 — the bounded top-p rule isn't propagated to the API section.**
§15.5 rejects `top_p < 1` with `top_k = 0`, but §22 advertises
temperature/top-p/top-k without the constraint. Common client defaults
(`top_p=0.9`, top_k unset) will be rejected at runtime. Document the
constraint and its structured error in §22, and state the recommended
client substitution (`top_k=256`) without silently applying it.

**V2-m4 — format §14 OPEN L2 says "before format minor 2 is frozen" but
the draft already stamps `format_minor = 2`.** Same pattern existed in
v0.1 (minor 1). Reword to "before this minor revision's ABI freeze" to
remove the reading that the freeze already happened.

**V2-m5 — §15.1 tolerance language predates the §15.3 machinery.** "Match
the pinned reference token sequence under the declared numerical
tolerance" should reuse the stable-position/tie-adjacent classification
now defined for MTP, so MTP0-vs-reference and MTPK-vs-MTP0 use one
equivalence vocabulary. Fold into OPEN item 6.

### QUESTION

**V2-Q1:** Which indexer-key mechanism does the pinned GLM-5.2 reference
implementation use — a cached key tensor per position or recomputation
from the latent — and what does the existing glm52-opt serving stack do?
(Determines V2-M1's resolution; check before the operation manifest
freezes.)

**V2-Q2:** Is "28.224B remaining parameters" (§8.1) derived from the
pinned tensor inventory or from the rounded 753B marketing count? The
budget artifact will pin the true number; until then label it as an
estimate in the spec text.

## 2. Answers to the five requested decisions

**1. Are B1, B2, B3 resolved as specification-contract blockers? — YES,
all three.**
B1: `speed-nvfp4` deleted, `nvfp4-laboratory` correctly caged (no
`HEALTHY`, no serving API, no capacity claims), `hybrid-serve` sole
NVFP4-bearing serving profile, EXL3 mandatory before M5, and the
infeasibility bound is now normative text with correct (and
stronger-than-mine) arithmetic. B2: the verifier-authoritative rule is the
right resolution — it makes the emitted sequence well-defined without
mandating batch-invariant kernels, and the tie-adjacent gate is properly
OPEN-blocked before MTP1 rather than silently defaulted. B3: content-
derived UUIDs, zero in-file timestamp, provenance sidecar, and the
"byte-identical rank files" definition satisfy the M1 determinism gate;
the derivation has no circularity.

**2. Are all eight majors resolved or safely gated? — YES**, with one new
gap adjacent to old-M2: draft-KV (sidecar geometry, pairing, atomic
publication, orphan rejection, MTP0-only degradation — coherent across
engine §§15–19 and format §§21–25, with recurrence semantics correctly in
OPEN 4); decode route (resolved, except the indexer-key input gap now
filed as V2-M1); DCP1/2 (resolved by cut, with the V2-M2 latency
consequence to measure at M3); graph budget (graph-profile artifact +
admission rejection + 1 GiB escrow — resolved); sampling distributions
(post-filter, identical pipelines, ABI-scoped — resolved); namespace split
(verified fail-closed: weight-policy hash stays in content identity,
records are TP/DCP-independent, semantic kernel changes route through the
KV ABI string — resolved); boot integrity (`FULL_SHA256`/`FS_VERITY` with
first-load rules — resolved); sharded sampling (resolved; the bounded
top-p tradeoff is acceptable for v0, see V2-m3).

**3. May M1–M2 NVFP4 CPU-reference and codec-proof work begin? — YES.**
M1 may begin immediately; nothing found in this pass touches the codec
math, container, KV oracle, or budget-calculator work. M2 *preparation*
(benchmark matrix, kernel ABI drafting) may proceed; M2 execution remains
an explicit operator authorization, as both handoffs state. V2-M1 and
V2-M2 block M3, not M1.

**4. Minimum edits before the CPU-only phase:** one required, three
recommended. Required: extend OPEN items 1 and 4 to name the indexer-key
contract (V2-M1) so the manifest work scheduled inside M1 answers it
rather than discovering it. Recommended riders: V2-m1 lower-bound wording,
V2-m2 inequality terms, V2-m4 wording. V2-m3/V2-m5 can wait for the next
editorial pass.

**5. Safely deferred:** EXL3 codec internals (OPEN 2/5) until the EXL3
phase; NVMe compaction (OPEN 10 / format 7), paused tails (format 6), and
DRAM-volatility ergonomics until tiered serving; KLD/task thresholds
(OPEN 11) until M5; detokenization/stop-strings (OPEN 12) until the
serving gate; adaptive MTP depth, GDS, and fairness tuning until M6/M7;
V2-M2's mitigation choice until the M3 ledger produces numbers — but the
ledger line item itself must be in the M3 gate now.

## 3. Sidecar-versus-unified answer (handoff re-review question 2)

The separate draft sidecar is the right call over a unified 79-layer
record. It preserves MTP0-only attachment of target pages (a real serving
mode after crashes and for target-only tenants), keeps the target record
ABI byte-stable regardless of MTP capability, and makes the orphan rule
enforceable at the index layer. The cost — a second header and ~21%
padding overhead on the small draft record — is negligible against the
1.84 MB target record it accompanies. The atomic-generation pairing rule
closes the consistency hole a unified record would have closed, at far
lower blast radius.

## 4. Verdict

v0.2.0 is a materially stronger contract than v0.1 and every disposition
was implemented as described — no gap between the disposition document and
the spec text was found. With the OPEN-item extension for the indexer-key
contract, this re-review finds **no blocker to beginning M1**. The next
independent review point should be the generated model operation manifest
(OPEN 1), which is where V2-M1, V2-Q1, the IndexShare pattern, and the
draft-recurrence semantics all converge.

*End of re-review. No GPU work is authorized by this document.*
