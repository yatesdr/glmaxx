# Tenant resource quotas v1

Date: 2026-07-29

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Scope

This contract defines one fail-closed resource ledger for authenticated
multi-tenant GLM-5.2 serving. It covers:

- request-body ingress and tokenized command queues;
- retained prompt tokens and requested context horizons;
- prefix matching, restore work, and shared-page references;
- committed and tentative target/draft HBM pages on all four DCP owners;
- suspended sessions backed by DRAM or NVMe;
- atomic commit, rollback, cancellation, and fatal cleanup; and
- bounded, fixed-label quota observability.

It does not set commercial tenant entitlements, qualify GPU execution, or
replace the cache eviction policy. It freezes the accounting boundary those
features must obey.

## Current blocking gaps

The current control plane has several independent bounds but no complete
admission transaction:

1. HTTP rejects configured bodies above 16 MiB, but reads the body before
   authentication and tenant resource reservation.
2. `CoordinatorBackendConfig` bounds commands globally, but one tenant can
   consume the whole queue.
3. prompt storage has one global byte ceiling and no tenant attribution.
4. `TenantConfig` limits active request count only.
5. tokenization happens before any queued-token or requested-context
   reservation.
6. prefix restore starts before an exact target/draft HBM and transfer
   reservation is owned.
7. `SequencePageTable` reports physical pages, but no quota ledger binds those
   pages to admission, step reservation, rollback, or offload.
8. shared HBM and durable prefix pages have no defined global-physical versus
   tenant-logical charging rule.
9. a request can claim a near-1M horizon without consuming a context-band
   entitlement.
10. fatal drain proves terminal event delivery but not exact resource release.

The engine must continue treating these paths as CPU preparation, not
production multi-tenant capacity, until the contract is reviewed and proved.

## Single authority

One process-wide `ResourceLedger.v1` is the sole quota authority. HTTP
workers, the backend command queue, the scheduler, cache restore workers, and
rank executors never maintain independent admission truth.

The ledger has one serializable mutation order. An implementation may use a
bounded mutex, a single-owner command loop, or fixed-size atomics plus a
transaction lock, but the externally visible result must be equivalent to
one ordered state machine.

All four ranks consume the coordinator's already accepted page reservation.
A rank-local capacity check may report disagreement, but may not choose a
fallback, evict a page, shrink MTP depth, or launch a smaller batch.

Every accepted request owns exactly one move-only `ResourcePermit`:

```text
ResourcePermit.v1 {
    request_id
    tenant_id
    state
    requested_context_tokens
    context_band
    charges
    transition_generation
}
```

Copying, reconstructing, or silently dropping a permit is forbidden. The
permit reaches `RELEASED` exactly once. A debug/evidence build retains every
transition and counter delta.

## Immutable startup profile

The production process loads one canonical `ResourceQuotaProfile.v1` before
reporting healthy:

```text
ResourceQuotaProfile.v1 {
    schema
    profile_id
    memory_plan_digest
    global_limits
    tenants[] {
        tenant_id
        scheduler_weight
        limits
    }
    context_bands
    digest
}
```

Tenant IDs are nonzero and unique. Limits and scheduler weights are immutable
for the process lifetime. A change requires a drained restart; live mutation
would make existing permits ambiguous.

The profile digest, memory-plan digest, model revision, tokenizer revision,
and weight-policy digest are part of startup consensus. The process is
unhealthy if any rank or control-plane component sees a different digest.

At least one configured tenant must be entitled to one requested context of
1,048,576 tokens and one MAX-band request. The global profile must also allow
that claim. This is a startup validation rule, not a promise that a second
near-1M request fits simultaneously.

## Context bands

The immutable band is selected from `prompt_tokens + maximum_output_tokens`:

| Band | Requested context tokens |
|---|---:|
| S | 1–8,192 |
| M | 8,193–32,768 |
| L | 32,769–131,072 |
| XL | 131,073–524,288 |
| MAX | 524,289–1,048,576 |

Every tenant has an in-flight count limit for each band. In-flight begins
when exact tokenization replaces the ingress permit and ends only at terminal
release. Moving a request between queued, restoring, resident, and suspended
states never escapes its band charge.

The band is based on the admitted maximum, not current committed length.
Reducing `maximum_output_tokens` before admission produces a different
request. The runtime never promotes a request into a larger band.

## Limit dimensions

### Global limits

The profile bounds:

- ingress requests and declared body bytes;
- queued requests, queued prompt tokens, and retained prompt bytes;
- active plus suspended requests;
- aggregate requested-context claims;
- restore operations, HBM restore payload bytes, and aligned tier-I/O bytes;
- pinned durable logical bytes and publication staging bytes;
- target and draft committed physical pages per owner rank;
- target and draft transient physical pages per owner rank;
- active shared-prefix references; and
- completion-event slots and buffered response bytes.

HBM page limits are derived from the accepted `SystemMemoryPlan.v2`, not
operator-entered larger numbers.

### Per-tenant limits

Every tenant bounds:

- ingress requests and declared body bytes;
- queued requests and queued prompt tokens;
- retained prompt bytes;
- active plus suspended requests;
- requested-context claim tokens;
- in-flight requests in each context band;
- resident target and draft logical page references;
- restore operations and logical restore bytes;
- shared-prefix references;
- pinned durable logical bytes for suspended sessions; and
- completion-event slots and buffered response bytes.

Per-tenant logical limits may sum above global physical limits. That supports
time sharing and deduplicated prefixes without allowing one tenant to consume
unbounded queue, pin, or restore capacity.

All counters are unsigned, checked at every addition and subtraction, and
bounded by their configured maximum. Overflow, underflow, an unknown permit,
or a state-incompatible delta is a fatal ledger invariant.

## Physical and logical charging

Shared resources use two simultaneous views.

### Global physical view

A unique HBM allocation is charged once to its owner rank. A unique in-flight
restore or durable record is charged once globally. The first requester owns
the physical reservation; concurrent deduplicated requesters join that
operation without multiplying the physical charge.

### Tenant logical view

Every request reference is charged to its tenant even when another request or
tenant already holds the same page. A tenant therefore cannot bypass its
resident-page, shared-prefix, restore, or durable-pin limit by selecting
popular content.

For concurrent restore deduplication, every waiter consumes its tenant
logical restore bytes and shared-prefix references. Only the canonical
restore ticket consumes global physical restore and tier-I/O bytes. When the
last waiter disappears before attachment, the ticket is cancelled if safe;
otherwise its bounded physical work completes and is immediately made
evictable.

A same-key/different-digest result is fatal as required by online prefix
publication. It is never interpreted as a second chargeable object.

## Exact page arithmetic

One page covers 64 positions. The current record geometry is:

```text
target KV             64 * 78 * 368 = 1,837,056 bytes
target indexer        64 * 21 * 132 =   177,408 bytes
target page logical                     2,014,464 bytes
draft KV              64 *  1 * 368 =    23,552 bytes
draft indexer         64 *  1 * 132 =     8,448 bytes
draft sidecar logical                      32,000 bytes
MTP-capable page logical                2,046,464 bytes
```

The online-publication contract aligns durable target and MTP-capable records
to 2,019,328 and 2,052,096 bytes respectively. HBM payload counters use
logical bytes. NVMe tier-I/O counters use the actual aligned transferred
bytes. Evidence reports both; neither may be relabeled as the other.

The capacity profile has, per owner rank:

```text
4,096 committed pages
   64 page-growth slack pages
    7 MTP tentative pages
4,167 physical arena pages
```

Thus one fully MTP-capable 1,048,576-token sequence owns 16,384 committed
pages, exactly 4,096 on each rank. Its logical HBM record bytes are:

```text
target plus indexer    33,004,978,176
draft sidecar             524,288,000
total                   33,529,266,176
```

The committed limit remains 4,096 pages per rank even though the arena has
4,167. Tenant data may not consume the 64-page growth and 7-page tentative
escrows as steady-state committed capacity. During a step, actual committed
plus canonical batch reservations may use the arena up to 4,167 pages, but
commit is allowed only when the post-commit committed count remains within
4,096.

Counters are per rank, not only aggregate. Four free pages on rank zero do
not compensate for an exhausted owner rank.

## Request lifecycle

The permitted states are:

```text
INGRESS
  -> QUEUED
  -> RESTORE_PLANNED
  -> RESTORING
  -> RESIDENT
  -> SUSPENDED
  -> RESTORE_PLANNED ...
  -> TERMINATING
  -> RELEASED
```

Failure may transition any nonreleased state to `TERMINATING`. No transition
skips the corresponding atomic charge conversion.

### Ingress

HTTP parsing is split at the header boundary:

1. read bounded headers;
2. authenticate the tenant;
3. reject unsupported transfer encoding and invalid content length;
4. acquire an ingress request/body-byte permit; and
5. only then allocate and read the body.

The current read-body-before-authenticate path must be versioned. A socket
close, parse error, timeout, or authentication failure releases the ingress
permit. Authentication failures do not reveal tenant quota state.

### Tokenization and queue admission

After validation and tokenization, one atomic transition replaces ingress
charges with:

- one queued request;
- exact queued prompt tokens;
- exact retained prompt bytes (`4 * prompt_tokens`);
- one requested-context claim;
- one context-band claim;
- bounded completion-event capacity; and
- the tenant's scheduler identity.

The transition verifies `prompt + maximum_output <= 1,048,576` with checked
arithmetic. The fully tokenized command and its permit enter the bounded
queue together. Queue insertion failure rolls the transition back and
releases the permit before returning an error.

There is no uncharged interval between body release and queue ownership, and
no interval in which both complete ingress and queue charges remain. The
backend registry gate and fatal flag cover this same transaction.

Tokenization CPU time can still be abused by many small authenticated
requests, so the ingress-request limit remains held through tokenization.

### Queue selection

Commands enter bounded per-tenant queues behind one global limit. Selection
uses the scheduler's reviewed weighted fairness, including restore-pending
and suspended work; a tenant cannot gain service by repeatedly changing
states.

Lack of current HBM or restore capacity leaves a valid request queued. It
does not silently reduce context, disable MTP, discard a prefix, or return a
rank-local failure. Queue timeout or client cancellation follows normal
terminal release.

### Restore planning

Prefix lookup first produces an immutable read-only plan:

```text
RestorePlan.v1 {
    request_id
    matched pages and capabilities
    already-resident page references
    missing target pages per owner
    missing draft pages per owner
    tenant logical restore bytes
    global HBM payload bytes
    aligned DRAM/NVMe I/O bytes
    dedup ticket identities
    plan digest
}
```

Planning performs no copy, pin, allocation, or catalog mutation. The ledger
then atomically converts queued charges to `RESTORE_PLANNED`, acquiring:

- one active request;
- tenant logical restore, shared-reference, and future resident-page charges;
- exact global restore/tier-I/O charges;
- exact target/draft HBM destination-page reservations by owner; and
- durable pins required until attach or rollback.

Only an accepted plan may start workers. If another request changes
residency before execution, the coordinator either deterministically joins
the canonical ticket and amends the plan under the same ledger transaction,
or invalidates and replans before any copy.

### Restore completion and scheduler admission

Successful, hash-verified restore converts destination reservations into
committed physical pages. Existing shared pages only gain logical
references. The page table attach, scheduler admission, prompt reservation
transfer, prefix lease, and ledger transition to `RESIDENT` are one control
transaction.

Any failure releases restored allocations, dedup waiters, pins, future
logical page charges, and restore counters before the request returns to the
queue or terminates. A failed admission leaves no scheduler row, event, page
reference, or retained token orphan.

Prompt token bytes remain charged until prefill no longer needs them, as in
the existing coordinator. Releasing those bytes does not release the
requested-context or band claim.

### Step reservation

Before compiling `StepInput`, the page transaction supplies an exact,
page-aligned batch delta. The ledger reserves, for every owner:

- newly touched target pages;
- newly touched draft pages;
- target and draft tentative positions;
- per-tenant future logical resident references; and
- the post-commit committed-page counts.

All batch rows and ranks succeed or none mutate. The canonical quota
reservation generation and digest become inputs to the page transaction and
rank consensus. A CUDA rank cannot allocate from the escrow.

Prefill reserves its exact scheduled positions. MTP0 reserves one target
position. MTPK reserves `K+1` target and draft-capable positions after model
limit clamping. No reservation may raise a request above its immutable
requested-context claim.

### Commit and rollback

Commit converts exactly the pages made reachable by consensus output into
committed global physical and tenant logical charges. Unused tentative pages
return to the escrow before the next step. The requested-context and band
claims do not shrink as tokens commit.

Rollback restores the exact pre-step charges and page identities. It is
idempotent by `(request_id, step_id, reservation_generation)`. A retry uses
the same logical quota state; it does not acquire a second reservation.

Cancellation during a step waits for commit or rollback at the
collective-safe boundary, then enters terminal cleanup.

### Suspension and resume

Only sealed, durably represented pages may leave HBM. Suspension:

- removes the sequence from schedulable resident rows;
- releases global HBM physical and tenant resident logical-page charges after
  rank removal acknowledgment;
- retains the requested-context and context-band claims;
- acquires tenant logical durable-pin bytes and the corresponding global
  unique physical pins; and
- records no HBM address in the suspended sequence.

Private unsealed tails remain in HBM or are sealed under a separately
reviewed policy before suspension. They are never discarded.

Resume follows the normal restore-plan transaction. Suspended requests
compete under the same tenant weight and restore quotas as new requests.
Repeated suspend/resume cannot reset fairness service units or the context
band claim.

DRAM and NVMe capacity increases the number of suspended sessions, not the
number of simultaneously resident decode rows. Capacity reports must state
both separately.

### Terminal cleanup

Finish, stop-string cancellation, explicit cancellation, timeout, slow
consumer, restore failure, rank failure, process shutdown, and fatal command
drain all use one idempotent cleanup routine:

1. stop new work for the request;
2. settle or roll back any in-flight step;
3. remove scheduler and device page-table reachability;
4. wait for the required four-rank removal generation;
5. release HBM pages, prefix references, restore waiters, and durable pins;
6. release prompt, event, context, band, queue, and active charges;
7. mark the permit `RELEASED`; and
8. emit at most one terminal event.

A terminal event may be undeliverable to a slow or disconnected consumer.
Resource release is not conditional on event delivery.

Fatal drain covers accepted commands still in the backend channel as well as
active coordinator requests. Their permits are present in the same registry,
so clearing only request ownership without releasing permits is fatal.

## Admission outcomes

The ledger returns one of:

- `ACCEPT`: transition and charges committed;
- `WAIT`: request remains in its prior valid state and may be retried by the
  coordinator;
- `REJECT`: immutable request or tenant limit cannot admit the request; or
- `FATAL`: arithmetic, identity, generation, or rollback invariant failed.

Fixed external codes include:

| Code | Class |
|---|---|
| `TENANT_INGRESS_LIMIT` | reject, HTTP 429 |
| `TENANT_QUEUE_LIMIT` | reject, HTTP 429 |
| `TENANT_PROMPT_TOKEN_LIMIT` | reject, HTTP 429 |
| `TENANT_CONTEXT_LIMIT` | reject, HTTP 429 |
| `TENANT_CONTEXT_BAND_LIMIT` | reject, HTTP 429 |
| `TENANT_RESTORE_LIMIT` | wait or timeout, HTTP 429 |
| `GLOBAL_QUEUE_LIMIT` | reject, HTTP 503 |
| `GLOBAL_RESTORE_LIMIT` | wait or timeout, HTTP 503 |
| `GLOBAL_HBM_CAPACITY` | wait or timeout, HTTP 503 |
| `GLOBAL_TIER_PIN_LIMIT` | wait or timeout, HTTP 503 |
| `RESOURCE_LEDGER_FAILED` | fatal, HTTP 500 |

Capacity waits do not occupy restore workers or HBM destinations. HTTP
status is returned only if the request has not started streaming. A streamed
request receives the equivalent structured terminal SSE error.

## Fairness rules

Weighted service is charged for:

- prefill query tokens;
- decode query rows;
- MTP verify rows, including examined rejected rows;
- restore logical bytes; and
- tier read/write bytes caused by the request.

The exact unit normalization requires a scheduler review before
implementation. Irrespective of normalization:

- service debt survives queue, restore, resident, and suspended transitions;
- cache hits may reduce work charged but never reset accumulated debt;
- deduplicated physical restore work is divided deterministically among
  waiters while every waiter keeps its logical quota charge;
- no tenant obtains priority from request ID or hash-map iteration order; and
- a MAX-band request can make bounded progress without starving smaller
  bands, while small requests cannot permanently starve it.

The proof harness must retain selection order and service-unit deltas rather
than reporting aggregate throughput only.

## Observability

Prometheus labels remain fixed-cardinality:

- lifecycle state;
- context band;
- quota dimension;
- outcome (`accept`, `wait`, `reject`, `release`, `fatal`);
- fixed reason code;
- target/draft capability; and
- storage tier.

Tenant IDs, request IDs, prompt hashes, page keys, seeds, and arbitrary error
strings are forbidden metric labels. Per-tenant current usage is available
only through authenticated bounded administrative snapshots and is not
exported as a Prometheus label.

Required gauges and counters include:

- usage and configured limit by global dimension;
- request count by lifecycle state and band;
- committed and transient pages by rank and target/draft;
- restore logical, HBM payload, and tier-I/O bytes;
- shared physical pages versus logical references;
- suspension/resume count and duration;
- waits and rejections by reason; and
- cleanup releases and fatal ledger invariant count.

The results bundle records high-water values, not only final-zero gauges.

## CPU and fault proof required before integration

The implementation gate must cover at least:

1. header authentication and ingress reservation before body allocation;
2. invalid, truncated, timed-out, and oversized body release;
3. concurrent tokenization with exact ingress-to-queue conversion;
4. queue insertion full/disconnected rollback;
5. per-tenant queue isolation under a full global queue;
6. checked prompt-plus-output boundary at 1,048,576;
7. every context-band boundary and a MAX-band entitlement;
8. one cold target-only and one cold MTP-capable restore;
9. warm same-tenant and cross-tenant shared prefixes;
10. concurrent deduplicated restore with one physical and N logical charges;
11. one waiter cancellation and last-waiter cancellation;
12. target-only prefix rejection or replan for an MTP request;
13. late owner-rank capacity failure with zero partial charge;
14. C1 and C64 prefill/decode/MTP1–6 page reservations;
15. every page-tail occupancy 0–63;
16. commit of `1..K+1` MTP tokens and rejected-suffix rollback;
17. worker/output/consensus failure rollback and retry idempotence;
18. cancellation before selection, during restore, during step, and after
    terminal output;
19. slow consumer and disconnected completion channel cleanup;
20. fatal rank step with active plus still-queued permits;
21. suspend/resume with shared pages and an unsealed private tail;
22. DRAM/NVMe pin pressure without false HBM capacity;
23. one 1,048,576-token MTP-capable sequence using exactly 4,096 pages/rank;
24. refusal to use the 71-page/rank escrow as committed tenant capacity;
25. two tenants whose logical references exceed unique physical pages;
26. overflow/underflow/generation mismatch as fatal;
27. deterministic weighted selection across repeated runs; and
28. final zero usage after every success and injected failure case.

The proof uses actual GLM page geometry and bounded production limits. A
toy-size-only ledger is insufficient.

## Gate and implementation order

1. adversarially review this contract together with the page transaction,
   online publication, active page table, memory plan, scheduler, backend,
   and observability inputs;
2. implement a pure CPU ledger oracle and exhaustive transition tests;
3. version HTTP header/body handling and the backend permit boundary;
4. integrate per-tenant queues and deterministic weighted selection;
5. integrate restore planning and page-transaction reservations;
6. integrate suspend/resume and publication pins;
7. run the full CPU/fault matrix and pin its artifact;
8. run authorized SM120 pressure and rollback tests; and
9. qualify sustained multi-user serving separately from single-request
   1M-context capacity.

Until steps 1–7 pass, the existing active-request and prompt-byte limits are
useful safety bounds but are not evidence that S06 is complete.
