# GLM-5.2 recurrent MTP execution v1

Date: 2026-07-29

Status: source-derived design candidate; adversarial review required before
CPU ABI implementation

GPU evidence: none

## Purpose and boundary

This document freezes the GLM-5.2 layer-78 program, its recurrent proposal
schedule, and the target/draft cache transaction needed for MTP depths zero
through six. It specializes the complete target program in
`docs/target-layer-execution-v1.md`; it does not define another model family
or load six independent draft layers.

The source audit found a material ambiguity in the current engine prose.
Layer 78 is trained and served as a one-token-shifted, teacher-forced
transition. A recurrent proposal row after recurrence zero uses the preceding
draft hidden, not the authoritative target hidden. Its KV is therefore
proposal scratch even when that token is later accepted. It cannot be
published as the durable draft sidecar for the accepted target position.

This candidate resolves that ambiguity by:

1. storing durable layer-78 records in successor-token slots;
2. making slot zero a canonical sentinel;
3. rebuilding every newly accepted transition with authoritative target
   hidden states in a teacher synchronization pass;
4. retaining recurrent rows after the synchronization root as scratch only;
5. representing the one authoritative emitted-but-not-yet-materialized target
   token explicitly; and
6. keeping sealed draft pages deterministic from the same token-prefix key as
   their paired target page.

These choices require coordinated amendments to the engine, format,
step-input, sampling, and serving-transaction candidates. This document does
not silently override those files. They remain unchanged until independent
review accepts or corrects this design.

## Immutable source identities

The model and source identities are:

| Input | Identity |
|---|---|
| model | `zai-org/GLM-5.2` |
| model revision | `b4734de4facf877f85769a911abafc5283eab3d9` |
| config SHA-256 | `185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a` |
| official Transformers commit | `5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` |
| modeling source SHA-256 | `adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72` |
| configuration source SHA-256 | `5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997` |
| pinned vLLM `deepseek_mtp.py` | `3a8a0b30e5dc5eb8c1f0ddb2ce317c375dc094de5b5ba8ba78f71d5481deae6d` |
| pinned NVIDIA V32 `mtp.py` | `8e09e33823d4a6feb5071eb4ef3a5822bf79c1fab7ab59b9e5220be67b5571ca` |

The vLLM files are read-only inputs under `../glm52-opt`. They are not
implementation dependencies and must not be modified from this repository.

The pinned proposer proves the shift:

- the first draft pass shifts every target token ID left within its request;
- the last target row receives the newly authoritative target token;
- target positions and target hidden rows are not shifted;
- pass zero computes one winner list per teacher row;
- the final teacher row's winner list is compacted per surviving sequence;
- later recurrent passes reuse that final winner list; and
- the post-final-norm draft hidden is recycled.

For a target token chain `t[0..n]`, the teacher row at draft position `x`
therefore consumes `t[x+1]` and target final hidden `hT[x]`. It does not
consume `t[x]` and `hT[x]`.

## Fixed geometry and weight program

There is exactly one independent draft layer:

```text
checkpoint layer       78
recurrent selector     spec_step_idx mod 1
hidden size            6,144
vocabulary logical     154,856
vocabulary physical    154,880
maximum draft depth    6
sparse winners         2,048
routed experts         256
experts per token      8
draft KV bytes         368 per valid successor slot
draft indexer bytes    132 per valid successor slot
combined record        500 bytes
```

Layer 78 owns the same absorbed MLA, full sparse indexer, routed/shared MoE,
residual, DCP attention, and two TP reduction boundaries as a full-indexer
sparse target layer. It adds:

| Role | Source shape | TP rule / rank shape |
|---|---:|---|
| `enorm.weight` | `[6144]` | replicated |
| `hnorm.weight` | `[6144]` | replicated |
| `eh_proj.weight` | `[6144,12288]` | replicated |
| `shared_head.norm.weight` | `[6144]` | replicated |

It shares the target token embedding and vocabulary head. The embedding and
head retain the target's vocabulary-row TP4 intervals. One embedding batch
therefore performs the same owner-row lookup plus 6,144-value TP sum as the
target program. The head produces 38,720 physical rows per rank, masks the 24
invalid global rows, and uses the reviewed distributed-sampling routes. It
never gathers full logits in production.

The layer-78 descriptor set, source tensor identities, immutable codec
membership, collective templates, successor-slot ABI, and buffer-lifetime
table form `MtpProgram.v1`. Its canonical little-endian digest is
`mtp_program_sha256`. All ranks must accept the same digest before any draft
graph can execute.

## Exact layer-78 row

For input token `u`, draft logical position `x`, and prior hidden `h_prev`:

```text
e = TP_SUM(vocab_embedding_rank(u))                    [6144]
e = 0                                                 iff x == 0
e_norm = RMSNorm(e, enorm.weight, epsilon=1e-5)
h_norm = RMSNorm(h_prev, hnorm.weight, epsilon=1e-5)
z = eh_proj(concat[e_norm, h_norm])                    [6144]
(block_output, residual) = layer78(z, position=x)
pre_final = residual + block_output
h_draft = RMSNorm(pre_final, shared_head.norm, epsilon=1e-5)
rank_logits = shared_vocab_head_rank(h_draft)          [38720]
```

Concatenation is embedding first, prior hidden second. Norm reductions and
reciprocal-square-root arithmetic follow the target program's reviewed
BF16/FP32 membership. The final residual add and shared-head normalization
occur exactly once.

The generic source returns a pre-final value for a logits helper that applies
the norm, plus the post-norm recycled value. The NVIDIA source returns the
post-norm value in both tuple positions and applies the head only. The
logical result above makes those conventions equivalent; an implementation
must not apply the final norm zero or two times.

Layer 78 always has a full indexer. A teacher row computes and stores its
132-byte post-RoPE indexer key, computes its own exact top-2,048 winner list,
executes absorbed MLA, then the routed/shared MoE. Its DCP query/candidate/LSE
records, tie order, router arithmetic, TP sums, and failure rules are exactly
the target-layer contract with `layer_id=78` and `index_group=21`.

## Successor-aligned draft sidecar

Let target token positions be `s=0..N-1`. The combined sidecar remains:

```text
draft_sidecar[logical_page][slot=0..63][kv=368][indexer=132]
```

Its interpretation changes from an ambiguous same-token record to:

```text
global successor slot s = logical_page * 64 + slot

s == 0:
    canonical all-zero sentinel; never a valid attention key

s >= 1:
    token input              t[s]
    authoritative prior      hT[s-1]
    draft logical position   x = s-1
    stored KV/key            layer78(t[s], x, hT[s-1])
```

The KV and indexer encodings remain byte-for-byte the existing 368-byte and
132-byte ABIs. RoPE uses `x=s-1`, never storage slot `s`. A draft attention
lookup for logical position `x` maps to successor storage slot `x+1`.

This layout deliberately keeps a page's sidecar dependent only on the target
token prefix through the same page:

- page zero contains the sentinel and transitions for target tokens 1..63;
- page `j>0` contains transitions for target tokens `64j..64j+63`;
- the first record of page `j>0` depends on target hidden from the parent
  prefix, which is already bound by the chained parent page hash; and
- no record in page `j` depends on the first token of page `j+1`.

The nine-block sealed draft record and 32,000-byte payload do not grow.
Target and draft pages retain the same ordinal and DCP owner. A sealed MTP
page is valid only if slot zero obeys the sentinel rule and every other valid
target token slot has its teacher-forced record. The target page key remains
sufficient; no continuation token is added to the namespace.

At the 1,048,576-token limit, valid committed draft positions are represented
by successor slots 1..1,048,575. Slot zero is the sentinel. There is no
attempt to allocate successor slot 1,048,576.

## Teacher state versus recurrent scratch

The engine uses two distinct layer-78 modes.

### `TEACHER_SYNC`

Each row consumes a real successor token and its authoritative predecessor
target hidden. It computes:

- an independent full-indexer winner list;
- one commit-capable 368-byte draft KV record;
- one commit-capable 132-byte draft indexer record;
- post-final-norm hidden; and
- draft logits only for rows selected by the proposal schedule.

Rows for one sequence are ordered by successor slot and attend causally to
the committed prefix plus earlier teacher rows in that same immutable batch.
Different sequences never share hidden state or winner lists.

### `RECURRENT_SCRATCH`

The first scratch row consumes the token sampled from the last teacher row,
the next draft logical position, and that teacher row's recycled hidden.
Later rows consume the preceding scratch token and recycled hidden.

Every scratch row:

- writes KV only to a proposal-generation scratch arena;
- writes no durable indexer key;
- reuses the last teacher row's generation-bound winner list;
- may produce rank-local logits and a retained proposal distribution; and
- is discarded after the corresponding verifier finishes, regardless of
  acceptance.

The scratch arena is not addressable from the active sequence page table,
prefix index, DRAM/NVMe tiering, another request, or a later proposal
generation. Accepted scratch bytes are never relabeled as teacher bytes.

The source's `skip_topk=true` path skips index selection and does not establish
a commit-capable indexer record. That behavior is valid only in
`RECURRENT_SCRATCH`.

## Prompt prefill and prefix restoration

For an uncached prompt `t[0..N-1]`, target prefill first produces final target
hidden rows `hT[0..N-1]` and pending target logits after `t[N-1]`.

MTP-capable prefill then executes one causal teacher batch:

```text
input tokens       t[1], t[2], ..., t[N-1]
prior hidden       hT[0], hT[1], ..., hT[N-2]
draft positions    0,     1,     ..., N-2
successor slots    1,     2,     ..., N-1
```

An empty row set is legal for a one-token prompt. Slot zero is initialized to
the canonical sentinel without running layer 78. Every teacher row computes
its own winner list; `index_share_for_mtp_iteration` does not share winners
across prompt rows.

A shared prefix record does not persist a 6,144-value target hidden boundary
or full logits. Restoring a prefix that ends immediately before an uncached
token therefore performs one target boundary replay for the last cached
token:

- it reads committed positions strictly before that token;
- it produces current-token KV in scratch rather than overwriting the cached
  record;
- it reconstructs the exact final target hidden and, when needed, head
  logits; and
- its time is reported separately in warm-prefix evidence.

The recovered hidden drives the first uncached teacher row. An optional
future boundary-state record would be a new reviewed cache ABI; it cannot be
invented by one rank.

## Pipelined proposal bundle

High-throughput serving retains one explicit authoritative target token ahead
of materialized target KV. The state is:

```text
materialized_target_end        exclusive target KV/indexer position
emitted_token_end              materialized_target_end or +1
pending_target_token           optional token at materialized_target_end
pending_target_kind            INITIAL | GREEDY_MISMATCH | RESIDUAL | BONUS
pending_draft_sidecar_slot     same position as pending target token
proposal_count                 0..6
proposal token IDs             positions pending+1 .. pending+proposal_count
proposal q state               rank-local retained filtered distributions
proposal_generation            nonzero u64
```

The pending token is sampled from authoritative target logits and is safe to
emit. It is not a draft and requires no acceptance draw. It has a prepared
teacher sidecar record, but neither that sidecar nor target KV is committed
until the next verifier materializes the token.

`emitted_token_end - materialized_target_end` is therefore exactly zero or
one, never larger. A request with no pending token has no proposals. Prefix
publication, suspension, and session export stop at
`materialized_target_end`; they never expose the pending tail.

### Bootstrap

After target prefill or an MTP0-to-MTPK transition:

1. sample one authoritative target token `a` from pending target logits;
2. if `a` is terminal, emit it and create no bundle;
3. execute `TEACHER_SYNC(a, hT[last], position=last)`;
4. retain `a`'s teacher sidecar as tentative;
5. emit `a`;
6. sample proposal `d0` from the teacher row's draft logits; and
7. execute up to `K-1` recurrent scratch rows to produce `d1..dK-1`.

Early draft EOS shortens `proposal_count`. The chosen proposal count,
generation, q-state digests, tickets, row mask, and winner-list generation are
immutable input to the verifier.

### Verification

For a bundle with pending token `a` at position `n` and `R` proposals:

```text
target input rows    a, d0, d1, ..., d(R-1)
target positions     n, n+1, ..., n+R
row count            R+1
```

`a` is guaranteed. Target logits after row `a` verify `d0`; logits after row
`d[i-1]` verify `d[i]`; logits after the final accepted proposal provide the
correction or bonus distribution.

Greedy or probabilistic acceptance follows the distributed-sampling ABI over
the `R` proposals only. On first rejection at `i`, rows after `d[i-1]` are
discarded and the authoritative residual/correction token `b` is at position
`n+i+1`. If all proposals are accepted, the bonus `b` is at position
`n+R+1` when EOS, output-limit, and context rules allow it.

The committed target prefix advances through:

- pending `a`; and
- exactly the accepted proposals.

The correction or bonus `b` is the next pending authoritative token. It is
emitted but not target-materialized. If no next token is allowed, no pending
bundle is created.

### Synchronization and next proposal

Before target/draft commit, execute one `TEACHER_SYNC` batch over:

```text
accepted proposal tokens, followed by optional next pending token b
authoritative predecessor target hidden for each token
successor slots in strictly increasing order
```

The accepted rows replace their recurrent scratch lineage with
teacher-forced records and become commit-capable. The optional final `b` row
remains tentative and supplies the root hidden, winner list, and logits for
the next proposal generation. Later recurrences of that next generation are
scratch.

If verification accepts zero proposals, the synchronization batch contains
only `b`. If all six are accepted, it contains six accepted rows plus `b`.
The graph uses a fixed seven-row-per-sequence capacity with a rank-identical
valid mask and stable compaction order.

## MTP0 and dynamic depth

`MTP0` remains the target correctness reference.

An MTP-capable sequence may keep the one-token pipeline at depth zero:

- materialize the prior pending token with one target row;
- sample the next authoritative target token from its logits;
- run one teacher row to keep the sidecar synchronized;
- emit the new pending token; and
- create no draft proposals.

This produces exactly the same logical target distribution as target-only
decode. It separates output commitment from target-KV materialization by one
step, so matched MTP0 quality and failure tests must use this explicit
posture. A target-only sequence may instead use the nonpipelined decode path.

Depth is selected for the *next* bundle. Once proposals exist,
`proposal_count` and its maximum verify depth cannot be changed. Dynamic
policy may choose a new depth only after the current bundle verifies and all
ranks agree on the next immutable plan.

Switching an MTP-capable sequence to target-only serving first flushes its
pending token through target execution and either commits or discards all
proposal scratch. Switching back performs the bootstrap route. Neither
transition attaches a target-only prefix as draft-capable.

## Reservation, commit, and rollback

Before bootstrap or verification launches, the coordinator reserves:

- every target slot that may be materialized;
- every teacher sidecar slot that may commit;
- one optional next-pending target/draft slot;
- recurrent scratch capacity for six rows; and
- retained distributed `q` state for six proposals.

For verifier proposal count `R`, target execution touches `R+1` rows. The
optional all-accepted bonus requires successor slot `n+R+1`, so the
reservation spans up to `R+2` successor slots even though only `R+1` target
rows execute. Context and output clamping removes the bonus before launch
when that slot is unavailable.

Private mutable page metadata permits:

```text
draft_prepared_end == materialized_target_end
draft_prepared_end == materialized_target_end + 1  iff pending token exists
```

Sealed, shared, DRAM, and NVMe records always require equal published target
and draft valid ranges. The one-ahead record is private and tentative.

Four-rank consensus atomically:

1. commits target KV/indexer rows for `a` and accepted proposals;
2. commits `a`'s prior prepared teacher sidecar and newly synchronized
   accepted sidecars;
3. advances target materialization and token/RNG state;
4. retains at most one next-pending teacher record as tentative; and
5. installs the next proposal generation.

Rejected target rows, rejected or obsolete recurrent scratch, old q state,
and superseded winner lists become unreachable before slot reuse.

Any worker, collective, descriptor, nonfinite, sampling, output, or consensus
failure commits none of the step's new target rows, synchronized sidecars,
tokens, RNG counters, or proposal generation. A previously emitted pending
token belongs to the prior committed step and is never retracted. Failure to
materialize it terminates that request/worker generation rather than
rewinding client-visible output.

Terminal EOS or stop may leave the final authoritative token emitted but not
materialized. It is excluded from reusable prefix/session state. A
nonterminal suspension or exported session must first flush the pending token
and prove equal published target/draft ends.

## Distributed proposal state

Greedy proposals retain token IDs, local top candidates, winner margins, and
the proposal program/generation digest.

Probabilistic proposals retain, per rank and proposal:

- the post-filter local probability representation needed to reconstruct
  exact `q_i(token)` over that rank's vocabulary interval;
- sampled token ID and finite positive `q_i(d_i)`;
- support and probability digests;
- the DRAFT ticket; and
- the immutable sampling parameters.

A digest alone is insufficient for residual sampling. The bounded TOP_K
route may retain its at-most-256 global support representation. MASS retains
one FP32 probability per local valid vocabulary row, or an independently
reviewed algebraically equivalent representation. At TP4, depth six is at
most `6 * 38,720 * 4 = 929,280` local bytes per live sequence before
alignment. Graph capacity and admission account for this memory explicitly.

Because proposal creation and target acceptance occur in adjacent physical
steps, `SamplingCounter.v2` must bind tickets to a proposal-generation ID:

- DRAFT tickets commit when the bundle is installed;
- ACCEPTANCE and RESIDUAL/BONUS tickets consume from the exact continuation
  when that bundle verifies; and
- a failed step commits neither its ticket continuation nor replacement
  bundle.

The total logical-cycle ticket formulas remain those in the sampling ABI, but
their physical-step partition must be made explicit before probabilistic MTP
is implemented.

## Collective and graph schedule

Every teacher or recurrent layer-78 batch has the immutable order:

1. row-sharded embedding TP sum;
2. replicated norms and `eh_proj`;
3. full indexer query/key production for teacher rows only;
4. DCP query transport, candidate merge, and partial-LSE attention;
5. attention O-projection TP sum;
6. post-attention norm and routed/shared MoE;
7. MLP TP sum and final residual;
8. shared-head final norm;
9. sharded-head sampling collectives for selected rows; and
10. result/proposal-generation consensus.

Recurrent scratch skips step 3 and consumes the retained winner list, but it
still writes its own scratch KV and executes attention/MoE. No rank removes a
collective or changes a participant mask locally.

Graph profiles cover C1 through C64 for:

- target verifier rows `R+1`, `R=0..6`;
- teacher synchronization rows `1..7`;
- recurrent scratch depths `0..5`;
- greedy, bounded TOP_K, and MASS proposal routes;
- early EOS compaction; and
- successor slots at every 64-token page boundary.

Compaction keys are
`(request_id, proposal_generation, successor_slot, row_kind)`, not transient
batch indices. The winner list retained from the final teacher row carries
the same identity and may be reused only by that proposal generation.

## Required contract amendments after acceptance

Acceptance of this design requires a coordinated version change:

1. `spec/engine-v0.md`: replace same-token draft semantics with the
   successor-slot teacher/scratch distinction and pipelined bundle.
2. `spec/format-v0.md`: define slot zero sentinel, `x=s-1` RoPE, and private
   one-ahead valid-range rules without changing the 500-byte record.
3. `manifests/glm52-operation-v1.json`: pin teacher lineage, scratch
   noncommitability, layer-78 index group 21, and successor mapping.
4. `docs/step-execution-io-v1.md`: add materialized/emitted ends, pending
   target token, proposal generation/count/mask, q-state digest, and next
   depth.
5. `docs/serving-page-transaction-v1.md`: reserve an optional one-ahead
   draft slot and permit only private draft-prepared length to lead target by
   one.
6. `docs/distributed-sampling-abi-v1.md`: version ticket ownership across
   proposal-install and later verification and require retained q data.
7. `docs/online-prefix-publication-v1.md`: publish only equal materialized
   target/draft ranges and validate the page-zero sentinel.
8. `crates/glm-cache/src/mtp.rs`: replace the depth-only arithmetic oracle
   with the proposal-bundle state machine.

No CUDA code may consume a partial mix of old and new contracts.

## Required CPU/reference gate

After adversarial acceptance, CPU proof must cover:

1. a source-lineage oracle reproducing the pinned shifted-input proposer;
2. exact layer-78 tensor-role completeness and one independent layer;
3. final norm exactly once for both pinned source conventions;
4. successor-slot mapping, `x=s-1` RoPE, slot-zero sentinel, and checked
   page/owner arithmetic at slots 0,1,63,64,65, and 1,048,575;
5. byte-identical sealed sidecars for the same token prefix regardless of
   prefill chunk and page split;
6. teacher rows using authoritative target hidden and recurrent rows using
   recycled draft hidden;
7. a fixture where those hidden lineages differ, proving accepted recurrent
   bytes cannot pass the teacher commit gate;
8. independent winner lists for every teacher row and generation-safe reuse
   only after the final teacher row;
9. MTP0 through MTP6 bootstrap, verify, mismatch at every proposal, all
   accepted, early draft EOS, accepted EOS, correction EOS, bonus, and output
   and context clamps;
10. materialized/emitted end arithmetic, one-ahead private state, flush,
    suspension, cancellation, retry, and terminal cleanup;
11. every tail occupancy and cross-page reservation with no orphan published
    sidecar;
12. exact greedy output equivalence to authoritative target logits;
13. probabilistic q retention and proposal-generation/ticket continuation for
    TOP_K and MASS;
14. C1/C64 stable compaction with mixed proposal counts and early EOS;
15. fixed collective records and bytes for teacher, scratch, verifier, and
    sampling phases; and
16. fault injection before and after every target, teacher, sampling,
    consensus, and publication boundary.

The CPU proof opens no GPU gate.

## Later SM120 gates

Only after the design review and CPU proof pass:

1. replay one teacher row and one recurrent scratch row at an actual layer-78
   shape, checking target versus recycled hidden lineage;
2. replay a teacher batch crossing slots 63/64 and compare exact 368/132-byte
   records;
3. run C1 and C64 MTP0–6 proposal/verify/synchronize graphs;
4. prove no scratch address is present in a committed page-table upload;
5. compare MTP0 target logits and tokens before enabling MTP1;
6. run per-position greedy and probabilistic quality gates; and
7. report proposal, verifier, synchronization, sampling, collective, and
   end-to-end time separately.

No result from this document authorizes cn4, a CUDA launch, production MTP,
full-checkpoint conversion, or a quality/performance claim.

