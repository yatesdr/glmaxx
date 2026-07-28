# Fable adversarial review — GLM-5.2 SM120 native engine v0

Date: 2026-07-28

Reviewer: Fable (claude-fable-5)

Review inputs and the SHA-256 of the bytes actually reviewed:

| File | SHA-256 reviewed | Matches handoff hash |
|---|---|---|
| `spec/engine-v0.md` (normative) | `fe89ebb87f0b10558630481d863c36542e9ac4b5420aee302a50db70dcf98205` | **no** (handoff records `fc2dabf9…`) |
| `spec/format-v0.md` (normative) | `8edfda4a31ac4bc677814f5e8d78e21892622745b2d040c5eb860201a7406275` | yes |
| `docs/native-engine-plan.md` (supporting) | `111c2956b5117f102e95fceb64fd1eaadc57ba9669e8b54d5f3fc4e53e0bedf1` | **no** (handoff records `9ed35201…`) |

**Provenance discrepancy (reportable):** the handoff's recorded hashes for
the engine spec and the plan do not match the files on disk, whose
modification times predate this review's reads. The spec author edited
after computing the handoff hashes (or recorded stale ones). This review is
against the on-disk bytes hashed above. Any further edits after 2026-07-28
16:52 local time are not covered; re-run the hash check before acting on
this review, and regenerate the handoff table when the specs change.

Also consulted: `docs/charter.md`, `docs/sm120-design.md`, `docs/roadmap.md`,
`docs/benchmark-contract.md`, `docs/quantization-workflow.md`,
`docs/hardware-lab.md`, `docs/references.md`, `AGENTS.md`, `README.md`.

Scope note: I could not open `../glm52-opt` evidence or the pinned Hugging
Face config from this review. Where a finding depends on model geometry, I
derive it strictly from the constants the specs themselves pin, and I flag
the derivation assumptions as questions. Verify my parameter-count
assumptions against the pinned `config.json` before acting on the capacity
findings.

---

## 0. What was checked and found correct

Credit where due — the following survived deliberate attack:

- **All byte arithmetic reproduces.** 368-byte KV record field map sums
  exactly (256 + 32 + 4 + 4 + 8 + 64). 78 × 64 × 368 = 1,837,056. Sealed tier
  record 4,096 + 1,837,056 + 2,048 = 1,843,200 = exactly 450 × 4 KiB.
  368 × 78 × 1,048,576 = 30,098,325,504 B = 28.03125 GiB; 7.0078125 GiB/rank
  at DCP4. 16,384 logical pages; 4,096 pages and 262,144 committed slots per
  rank. 64 × 7 = 448 verifier rows.
- **Header, descriptor, and codec-metadata offset maps are internally
  consistent** (header fields tile to 4,096 bytes exactly; descriptor to 256;
  NVFP4 metadata to 128; no overlaps).
- **The NVFP4 numerical definition is right**, including the frequently
  botched rule that the encoder must use the *decoded stored* E4M3 scale to
  produce E2M1 codes, and `global_scale = amax / (448 × 6)` matching the
  representable maximum `e2m1(6) × e4m3(448)`. The KV `s_t = amax/(6 × 448)`
  convention is consistent with it, and the group-scale range works out
  (block_amax/s_t ≤ 6·448, so /6 ≤ 448, always E4M3-representable).
- **Fail-closed posture is uniformly applied** — unknown codecs, flags,
  static-KV namespace, cross-rank UUID/policy agreement, and the four-rank
  MIN vote / rank-invariant `StepPlan` discipline are the right shape for a
  no-NVLink PCIe TP4 machine.
- **The `capacity-exl3` profile is feasible on my arithmetic** (~253 GiB
  routed experts at 3.0 bpw + ~57 GiB BF16 protected + 28.03 GiB KV +
  ~10 GiB runtime ≈ 348 GiB against ~372 GiB usable aggregate), which is
  consistent with the reported 1,101,312-token EXL3 control.
- The gate sequence complies with the repository-mandated order; no
  milestone silently authorizes GPU work.

---

## 1. BLOCKER findings

### B1 — The `speed-nvfp4` profile as specified cannot physically load, let alone serve

**Cites:** `spec/engine-v0.md` §5.2, §8; `spec/format-v0.md` (absence of any
physical-byte budget); `docs/fable-review-handoff.md` ("cannot be assumed").

The handoff suspects tightness; the arithmetic says it is not tight — it is
infeasible by a wide margin. Derivation from the pinned constants:

- Routed expert parameters: 3 matrices × 6,144 × 2,048 = 37,748,736 per
  expert; × 256 experts × 75 sparse layers (78 − 3 dense front; see Q3)
  = **724.8 B parameters**, ~96% of the 753 B total.
- NVFP4 1D block-16 (codec `0x0100`): 0.5 B/value + 1/16 B scale
  = 0.53125 B/param → **358.6 GiB** for routed experts alone.
- NVFP4 2D 16×16 (codec `0x0101`): 0.50390625 B/param → **340.1 GiB**.
- Protected set (≈28.6 B params: attention, dense front MLPs, shared
  experts, router/indexer, embedding + LM head, MTP): **~26.6 GiB at FP8**,
  ~57 GiB at BF16.
- Aggregate HBM: 4 × 96 GB = 384 GiB nominal, realistically **~372–376 GiB
  usable** after contexts, modules, and driver reserve (`cuMemGetInfo`
  governs, as the plan itself demands).

Totals for `speed-nvfp4` as written (§5.2: routed experts **SHALL** all be
NVFP4):

| Configuration | Experts | Protected | KV | Runtime/graphs/escrow | Total | Verdict |
|---|---:|---:|---:|---:|---:|---|
| 1D, FP8 protected, 1M KV | 358.6 | 26.6 | 28.0 | ~10 | **~423 GiB** | over by ~50 GiB |
| 2D, FP8 protected, 1M KV | 340.1 | 26.6 | 28.0 | ~10 | **~405 GiB** | over by ~30 GiB |
| 2D, FP8 protected, **zero KV** | 340.1 | 26.6 | 0 | ~10 | **~377 GiB** | still at/over usable |

Even the most aggressive variant — 2D scaling, FP8 for everything protected,
and *no KV at all* — does not fit. §5.2's escape clause ("not required to
admit the full model position limit") does not save it: the profile fails at
weight load, not at KV admission. Per-rank framing gives the same answer
(1D experts alone are ~89.7 GiB/rank of 96).

Consequences that must be reflected in the spec:

1. **`speed-nvfp4` cannot exist as a serving profile on this hardware.** It
   can only exist as (a) a laboratory profile over a checkpoint *subset*
   (fine for M2/M3/M4 bring-up, which is what NVFP4-first actually needs),
   or (b) a hybrid. The spec must say so.
2. **Hybrid is not the optional third profile — it is the only NVFP4-bearing
   serving profile.** For a 1M-KV serving profile the expert budget is
   roughly 376 − 26.6 − 28 − 10 ≈ **311 GiB ≈ 3.69 bpw average** across
   routed experts. That means a substantial fraction of experts must be
   sub-4-bit (EXL3) or the protected set must shrink below what quality
   evidence currently supports.
3. Therefore **EXL3 is on the critical path for any NVFP4-bearing serving
   milestone (M5–M7)**, which inverts the plan's "EXL3 not on the initial
   critical path" claim for those milestones. It remains true for M1–M4
   bring-up only. The milestone table should be re-annotated.
4. Neither spec contains the physical-byte budget table that §8 requires the
   *planner* to compute. The spec itself must carry a normative worked
   budget per profile, updated when the manifest freezes, so that a profile
   that cannot fit is a spec contradiction rather than a runtime discovery.

**Required change before any full 753B conversion or any M2 prioritization
that assumes `speed-nvfp4` serves:** add the budget table; re-scope §5.2 as
subset/laboratory-only or delete it as a serving profile; promote hybrid to
the primary NVFP4 serving profile; re-annotate EXL3 criticality per
milestone. None of this blocks the CPU-proof phase (see §5 verdict).

### B2 — Engine §15.3 "emitted sequence MUST equal MTP0 greedy output" is unimplementable without a batch-invariance contract the spec never states

**Cites:** `spec/engine-v0.md` §15.3, §24 (M4/M5, M6/M7); §11 (row buckets).

Verification executes target rows in bucketed batches (up to 448 rows,
graph-captured, padded); MTP0 executes the same positions at different M
through different graphs. GPU GEMM/reduction accumulation order varies with
batch shape, so target logits differ at the ULP level between the verify
pass and the MTP0 pass, and near-tie argmaxes flip. Emitting the verify
pass's own argmaxes is self-consistent, but it is *not* guaranteed equal to
an MTP0 run, which is what §15.3 demands as a MUST. §15.1 already concedes
"declared numerical tolerance" against the pinned reference; §15.3 then
demands exactness against a moving internal reference. As written the gate
will fail spuriously and permanently.

Two coherent resolutions — the spec must pick one explicitly:

1. **Mandate batch-invariant target kernels** in the kernel ABI (fixed-split
   reductions independent of M/bucket; masked rows provably non-contributing).
   This is achievable (split-K/tile reduction order fixed per shape family)
   and also strengthens M3/M4 replay debugging, but costs kernel freedom and
   some performance; it belongs in the kernel ABI as a testable property.
2. **Relax §15.3** to: emitted tokens are the verify pass's target argmaxes
   (self-consistency), plus a statistical equivalence gate against MTP0
   (token match rate over a pinned corpus above a declared threshold, with
   mismatches shown to be tie-adjacent).

This is also a **missing blocking OPEN item** for the handoff's list: the
determinism/batch-invariance contract of the kernel ABI, which additionally
governs whether the M3 one-layer replay comparisons can be bit-exact or must
be tolerance-based.

### B3 — "Deterministic conversion" is contradicted by the format's own header

**Cites:** `spec/format-v0.md` §5 (`file_uuid` "random build identity",
`created_unix_seconds`), §26 ("deterministic conversion");
`docs/native-engine-plan.md` M1 exit ("byte-stable NVFP4 format"),
"Correctness and quality gates → Format → deterministic pack bytes".

The M1 exit gate requires deterministic pack bytes; the header mandates a
random UUID and a wall-clock timestamp inside the hashed header region. Two
runs of the same converter on the same inputs cannot produce identical
files, so the M1 gate is unsatisfiable as specified. `header_crc32c` and any
whole-file hash also become non-reproducible.

Cheap fix, but it must land before M1: derive `file_uuid` and
`conversion_uuid` deterministically from content (e.g., truncated SHA-256 of
manifest + rank + payload hashes), and either move `created_unix_seconds`
to an unhashed provenance sidecar or define "deterministic" as
byte-identical *modulo an enumerated provenance field list* with the
determinism test masking exactly those bytes. Pick one and write it down;
do not let the packer implementation choose (spec's own rule in engine §25).

---

## 2. MAJOR findings

### M1 — MTP draft-layer KV is architecturally unaccounted for

**Cites:** `spec/engine-v0.md` §4 (78 target layers; 1 MTP layer), §16–§18;
`spec/format-v0.md` §20–§23 (layer count fixed at 78 everywhere).

If the pinned draft layer contains attention (the DeepSeek-style MTP design
this recurrence describes), it produces and consumes KV of its own. Every
KV structure in both specs — the 368 × 78 reservation, the
`[layer=78]` HBM geometry, the 1,837,056-byte page, the sealed tier record,
the prefix content key — hard-codes 78 layers and has no slot for draft KV.
Consequences if draft KV exists:

- the 28.03125 GiB reservation is short by 1/78 (~0.36 GiB aggregate) —
  small, but the spec's own standard is exact accounting;
- a prefix restored from DRAM/NVMe has **no draft KV**, so MTP depth ≥ 1
  after restore/attach is undefined: recompute it, disable MTP for restored
  sequences, or carry layer 79 in pages — three different designs;
- rollback of rejected speculative tokens must also roll back draft-layer
  KV writes, which §15.5 does not mention.

Related and budget-relevant: **does the MTP block contain its own 256
routed experts** (as DeepSeek-V3's MTP module contains a full MoE block)?
If yes, that is ~9.7 B more parameters whose codec assignment (§5 profiles
say "protected components… manifest-declared") materially moves the B1
arithmetic. OPEN item 4 covers recurrence semantics but not draft-KV
residency, tiering, or rollback — extend it explicitly.

### M2 — The decode-regime DCP attention route is unspecified, and the naive route is infeasible

**Cites:** `spec/engine-v0.md` §14 (prefill transport only), §2 (DCP
scope); `docs/native-engine-plan.md` "Decode plan" (hypothesis 4 only).

§14 specifies packed-CKV vs query transport for *prefill*. For decode at
deep context under DCP page ownership, the record-gather route is
arithmetically dead: top-2,048 winners × 368 B × 78 layers ≈ 58.7 MB per
token, ~44 MB of it remote, i.e. several milliseconds of PCIe Gen3 transport
per emitted token — an inter-token-latency floor that forfeits the regime.
The viable route is query transport with rank-local partial attention over
owned pages and a cross-rank log-sum-exp merge (M=1 payloads are KB-scale).
The spec must:

1. name the decode DCP attention route(s) and their selection key, as it
   already does for prefill;
2. define the **deterministic merge contract** (fixed rank order, defined
   accumulation precision) for the partial-softmax combine — the specs
   demand deterministic global merge for the indexer but are silent for
   attention itself;
3. state where the top-2,048 index winners are computed and exchanged in
   decode mode with rank-invariant results.

This must be resolved before M3 (one-layer TP4 replay), which is the first
gate that executes it.

### M3 — DCP degrees 1 and 2 are in scope but entirely unspecified

**Cites:** `spec/engine-v0.md` §2 ("DCP degrees one, two, or four"), §8,
§17, §21; `spec/format-v0.md` §21, §24.

Every ownership, capacity, and namespace rule is written for DCP4 only
(`j mod 4`, 262,144 slots/rank, 4,096 pages/rank). For DCP1/DCP2 nothing
defines: replication vs single-owner residency of head-independent MLA
records (at DCP1, do all four TP ranks replicate the full 28 GiB, costing
112 GiB aggregate?), the capacity floor, page-table shape, or graph
families. Worse, the prefix namespace includes "TP/DCP ownership ABI," so a
posture change silently invalidates the entire DRAM/NVMe cache estate
(see M6). Either **cut DCP1/DCP2 from v0 scope** (my recommendation — the
1M requirement forces DCP4 anyway) or specify them fully. Also state
whether DCP degree is process-immutable; nothing currently says so.

### M4 — The REQUIRED graph set and graph memory budget are undefined

**Cites:** `spec/engine-v0.md` §7 ("capture or load every REQUIRED graph"),
§8 (item 6; 512 MiB escrow), §11.

"REQUIRED" is never enumerated. The reachable key space is 7 sequence
buckets × 7 MTP depths × ≥4 context bands × modes (prefill chunks, verify,
CKV/query transport) — hundreds of candidates for a 78-layer model whose
per-graph node memory and capture-time workspace are not trivial. The specs
say "capture only hot combinations" (plan) but the normative spec has no
budget formula, no declared capture list artifact, and no policy for what a
production step does when its exact key was deliberately not captured
(§11 says reject-or-queue — under which SLO?). Required: the capture list
becomes a reviewed manifest artifact per profile; the memory planner gets a
per-graph cost model validated at M4; the 512 MiB escrow is provisional
until measured — I recommend raising the *initial* escrow to 1 GiB/rank and
letting measurement argue it back down, since escrow failure after HEALTHY
is defined as fatal (§21) and an optimistic escrow converts fragmentation
into process death.

### M5 — Probabilistic verification is ambiguous about pre- vs post-filter distributions

**Cites:** `spec/engine-v0.md` §15.4, §22 (temperature, top-p, top-k).

Rejection sampling preserves the target distribution only if `p` and `q`
are the *actual sampling distributions* — i.e., after temperature, top-k,
top-p, and any penalty processing. The spec never says whether `p_i`/`q_i`
are raw softmax or post-filter, and applying the acceptance rule to raw
distributions while sampling fallbacks from filtered ones (or any mixed
convention) silently breaks the §15.4 equivalence guarantee. This is a
known production trap. The sampling ABI (OPEN item 5) must pin: filter
order, that both draft and target use identical filter configurations, the
distribution used at each of the three sites (acceptance ratio, residual
`max(p−q,0)`, bonus token), and FP arithmetic/RNG-counter semantics per
site. Add this to the OPEN item's text so it cannot be "resolved" by
implementation default.

### M6 — Namespace design guarantees total cache loss on posture change; the tier record is nearly posture-neutral already

**Cites:** `spec/format-v0.md` §22–§24; `spec/engine-v0.md` §19.

The sealed tier payload order `[layer][token][record_byte]` is deliberately
ownership-independent, yet the namespace hash includes the TP/DCP ownership
ABI and the header embeds `owner_rank`. Result: changing DCP posture, or
any ownership-mapping revision (OPEN item 7), discards every NVMe/DRAM page
even though the bytes are valid. Recommendation: split the namespace into
(a) a *content namespace* (model, weight policy, tokenizer, template, KV
ABI, page size) governing tier records and prefix keys, and (b) an
*attachment compatibility check* (ownership ABI, kernel ABI) applied only
at HBM attach time. `owner_rank` in the header becomes advisory routing
metadata. This preserves the fail-closed property while making the cache
estate survive the one change (final DCP mapping) the spec itself lists as
likely.

### M7 — Mandatory full SHA-256 verification on every boot costs minutes and is pinned into the format

**Cites:** `spec/format-v0.md` §5 ("verify SHA-256 regions before
allocating device payloads"), §26.

Rank files are ~85–90 GiB each. SHA-256 at realistic single-stream rates
adds minutes to every engine start, multiplied by restart-heavy development
and by §21's fatal-error-means-restart posture. Define an integrity policy
rather than an unconditional rule: full cryptographic verify on first load
after write/transfer and on demand; a fast checksum tier (per-payload
CRC32C/xxHash already computable from the same pass) for routine restarts;
and consider a tree/parallel hash (e.g., BLAKE3) as the payload hash in a
format-minor revision if verify time proves material. Keep fail-closed;
change *when* the expensive proof runs.

### M8 — LM-head/logits sharding and sampling locality are unspecified, and the naive gather is a step-time hazard

**Cites:** `spec/engine-v0.md` §4 (vocab 154,880), §15, §22, §25 (item 5).

With a column-parallel LM head each rank holds ~38,720 logits per row. A
full-vocabulary gather for host sampling at the verifier ceiling is
448 rows × 154,880 × 2 B ≈ 139 MB per step — tens of milliseconds on Gen3
PCIe, i.e., larger than the compute it follows. The spec never states where
sampling executes, how greedy argmax and probabilistic sampling are
computed over the sharded vocabulary (distributed argmax/top-k merge,
Gumbel-based distributed sampling, or head replication), or how that
interacts with the deterministic RNG contract. This belongs inside OPEN
item 5's scope statement now, because it shapes kernels (GPU sampling is in
the kernel inventory) and the `StepPlan` collective schedule.

---

## 3. MINOR findings

1. **Plan/spec state-machine conflict (reportable per handoff):**
   `docs/native-engine-plan.md` "Tiered KV state machine" omits
   `HBM_TENTATIVE` and `INVALID`, which `spec/engine-v0.md` §18 includes.
   The spec is normative; update the plan.
2. **Streaming detokenization semantics** (engine §22): incremental UTF-8
   boundary handling and stop-string matching across chunk boundaries are
   unspecified; both are classic correctness bugs. Defer to the serving
   gate but track as an OPEN serving item.
3. **DRAM tier recovery** (engine §18): "shared mapping or equivalent
   recoverable allocation" — recoverable by whom, validated how after a
   crash? Either claim crash-recoverability with a generation/validation
   protocol like NVMe's, or drop the implication.
4. **NVMe endurance** (engine §18, plan): metrics are required but no
   configured write-rate/endurance bound exists; a pathological eviction
   pattern can burn the 1.6 TB drive. Add a configurable byte/day cap.
5. **`StepPlan` field semantics** (engine §9): `verifier_row_bucket` as u32
   for values ≤ 448 and `active_sequences` u16 for ≤ 64 are fine but
   wasteful; more importantly, define invalid/unused-field values per
   `mode` (e.g., `prefill_transport` during DECODE) so the plan hash is
   well-defined. `CACHE_ONLY` mode's collective schedule (empty? tier
   transfers only?) is undefined.
6. **Boot logistics** (hardware-lab): the four rank files (~340–360 GiB),
   the EXL3 control (336 GB), and any BF16 source cannot coexist on the
   listed free NVMe (~784 GiB + 1.3 TiB). Plan placement before conversion
   day, and note cold weight load itself is ~2 minutes of NVMe read.
7. **Escrow interplay:** plan says the converter targets a weight budget
   from the smallest same-phase `cuMemGetInfo`; spec §8 fixes profile
   reservations statically. State which governs (recommend: spec-owned
   budget table, converter consumes it, `cuMemGetInfo` validates it).
8. **KLD gate threshold** (engine §24 M4/M5, benchmark contract): the gate
   references the provisional `0.1195` control but no numeric acceptance
   threshold or equivalence band is defined anywhere; define it when the
   pinned series completes, before M5, so PASS is not negotiated post hoc.

---

## 4. QUESTIONS

1. **Q1:** Does the pinned MTP block contain attention and/or its own
   routed-expert MoE (DeepSeek-V3-style)? Both B1 and M1 arithmetic depend
   on it. Answer from the pinned `config.json`/weights inventory, not the
   model card.
2. **Q2:** Confirm sparse-MoE layer count = 75 (78 − 3 dense front) and add
   it to the §4 constants table explicitly — the specs never state it, and
   every capacity number depends on it.
3. **Q3:** Are the three dense front MLPs (6,144 × 12,288) in the protected
   set or quantizable? (~0.68 B params; second-order for B1 but must be in
   the budget table.)
4. **Q4:** Is DCP degree immutable for a process lifetime? (Interacts with
   M3 and M6.)
5. **Q5:** 78 layers are not divisible by the indexer refresh frequency 4.
   Do the dense front layers participate in DSA/IndexShare, and what is the
   tail-group behavior? (Manifest OPEN item 1 should be required to answer
   this explicitly.)
6. **Q6:** Activation NVFP4 global scale (format §16): dynamic per-step
   amax (requires an extra reduction pass or fused two-phase kernel) or
   calibrated static? This changes the operator cost model that the whole
   "inclusive operator timing" methodology measures.
7. **Q7:** For the hybrid profile, is per-*expert* codec selection allowed
   to differ per *layer* for the same expert index? (§5.3 says "tensor or
   routed expert"; descriptors identify `(layer, expert)`, so presumably
   yes — say so.)

---

## 5. Verdict on starting the NVFP4 CPU-proof phase

**PASS, with conditions.** The CPU-proof phase (M1: codec math, container,
packer determinism, KV oracle, page/tier state machines, MTP transition
tests, budget calculator) is well-specified, its numerical definitions are
correct, and none of the blockers invalidate that work. Conditions:

1. **Fix B3 first** — M1's own exit gate ("deterministic pack bytes") is
   currently unsatisfiable. One-paragraph spec edit.
2. **Decide the two zero conventions now** (format OPEN 2; engine OPEN 6).
   These are decidable by fiat today and the oracle cannot be written
   without them. Proposal: weight zero-block → scale byte `0x00`, all codes
   `0x00`; KV all-zero NoPE → canonical `s_t = 1.0f`, group scales `0x00`,
   codes zero; KV all-zero RoPE → `rope_scale = 1.0f`, values `0x00`.
   Whatever is chosen, the oracle test vectors freeze it.
3. **Record the B1 re-scope decision before authorizing any full 753B
   conversion or committing M2 kernel priorities** that assume a
   `speed-nvfp4` serving profile exists. The CPU oracle work itself is
   unaffected; the *program logic* built on top of it is.
4. **Extend OPEN items** to cover the missing blocking questions this
   review identifies: kernel batch-invariance/determinism contract (B2),
   MTP draft-KV residency/tiering/rollback (M1), decode DCP attention route
   and deterministic merge (M2), sampling locality over the sharded vocab
   (M8), and pre/post-filter distribution semantics (M5).

Minimum specification changes before the phase, in one list:
B3 edit; zero conventions; §5.2 re-scope note (even a one-line "laboratory
profile pending budget table" honesty marker); OPEN-item list extension.
Everything else can proceed in parallel with M1 implementation.

## 6. Handoff answers on the acknowledged blocking questions

- **Already decidable now:** both zero conventions (see above); the
  activation global-scale *contract shape* (Q6 must be decided even if the
  value policy is measured later); DCP1/DCP2 scope cut (M3) is a decision,
  not research.
- **Missing from the list:** B2, M1, M2, M5, M8 as enumerated above, plus
  the physical-byte budget table (B1) which the spec currently delegates to
  an implementation-time planner.
- **Work incorrectly permitted before its dependency:** nothing in the
  M1 slice, with one caveat — plan item "implement physical-byte and
  1M-admission accounting" should be *promoted*, not just permitted: it is
  the artifact that resolves B1 and should be the first deliverable of M1,
  its output pasted into the spec as the normative budget table.

## 7. Safely deferrable issues

Deferred until EXL3 work: entire codec `0x0200` internals (format §17);
hybrid allocation policy inputs; per-expert bit-allocation reproduction.

Deferred until concurrent serving (M6): fairness policy tuning; adaptive
MTP depth controller; chunked-prefill/decode mixing qualification;
streaming detokenization and stop-string semantics (minor 2); cancellation
cleanup-time bounds.

Deferred until NVMe tiering gate: bounded index compaction (OPEN 8
correctly scoped); GPUDirect Storage; endurance cap (minor 4); DRAM
recovery semantics (minor 3); paused private-tail record type.

Correctly deferred already by the specs: exact FFI `StepPlan` layout;
CUTLASS revision pin and NVFP4-L1/L2 layout freezes (blocking ABI freeze,
not oracle work); route-table qualification protocol details.

---

*End of review. No GPU work is authorized by this document.*
