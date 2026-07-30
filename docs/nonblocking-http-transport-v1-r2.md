# Nonblocking HTTP transport v1 r2 amendment

Date: 2026-07-30

Status: corrective design candidate; adversarial review required before Linux
reactor implementation

GPU evidence: none

## Purpose and precedence

This amendment corrects the findings in
`docs/reviews/fable-nonblocking-http-transport-v1.md`. It is normative when
read with `docs/nonblocking-http-transport-v1.md` and supersedes only:

- the eventfd wake protocol and its CPU proof;
- the epoll user-data cookie definition;
- pre-accept mailbox overflow behavior;
- listener publication and health ordering;
- runnable-list epoll timeout behavior;
- the Linux evidence boundary;
- the S05 transport-control arrival schedule and pass criteria; and
- reverse-proxy participation in the startup configuration digest.

Every other v1 requirement remains unchanged. This candidate contains no
transport implementation, load result, production-health claim, or GPU
authorization.

## Corrected eventfd wake protocol

Each reactor completion source owns:

- one bounded MPSC queue whose successful publish operation has release
  semantics and whose consumer observation has acquire semantics;
- one `AtomicBool notified`, initialized to `false`; and
- the reactor's nonblocking close-on-exec eventfd.

`notified == true` means either an eventfd wake is pending or the reactor has
retained ownership of runnable work. It is not a queue-length predicate.

### Producer

After acquiring every queue-entry and output-byte permit, a producer:

1. publishes the complete queue entry with release semantics;
2. executes
   `notified.compare_exchange(false, true, SeqCst, SeqCst)`;
3. on success, writes one to the nonblocking eventfd;
4. on failure with the observed value `true`, records a coalesced wake and
   returns without an eventfd write;
5. treats eventfd `EAGAIN` as an already-pending wake; and
6. treats any other eventfd failure as reactor-fatal.

The queue publish is sequenced before the SeqCst transition. No relaxed,
release-only, or acquire-release producer transition is conforming.

### Reactor

When the eventfd is ready or the source is on the reactor-local runnable
list, the reactor:

1. drains eventfd reads to `EAGAIN`;
2. consumes queue entries with acquire semantics up to the configured
   fairness budget;
3. if the budget expires and an acquire observation says work remains, keeps
   `notified == true`, retains the source on its local runnable list, and
   does not enter a blocking epoll wait;
4. only after an acquire observation says the queue is empty, executes
   `notified.swap(false, SeqCst)`;
5. immediately rechecks the queue with acquire semantics;
6. if the recheck observes work, executes
   `notified.swap(true, SeqCst)`, retains the source on the local runnable
   list, and continues draining without blocking; and
7. may remove the source from the runnable list and block only when the
   post-clear acquire recheck observes the queue empty.

The reactor's false-to-true swap in step 6 does not need to write eventfd:
the owning reactor is already running and retains the source locally. If a
producer won that transition first, its redundant eventfd count is drained
on the next pass. Eventfd counts are hints; queue state is authoritative.

The SeqCst RMW in step 4 is the StoreLoad barrier missing from v1. Together
with the producer's SeqCst compare-exchange, it creates these exhaustive
ownership cases:

1. a producer wins false-to-true after the reactor clear and writes eventfd;
2. a producer observes true before the clear, so its prior release-published
   entry is covered by the reactor's post-clear acquire recheck;
3. the post-clear recheck observes the entry and the active reactor retakes
   ownership without blocking; or
4. budget exhaustion leaves `notified` set and the source locally runnable.

No implementation may replace either SeqCst RMW with a plain store or weaken
the post-clear queue observation.

### Required memory-model proof

The implementation gate includes a small exhaustive model, using Loom or an
equivalent reviewed weak-memory model checker. The model contains at least:

- two producers;
- one consumer;
- a bounded queue publication/observation abstraction;
- the `notified` atomic;
- an eventfd-pending abstraction;
- current-pass and locally-runnable consumer states; and
- producer enqueue before, during, and after the clear/recheck window.

It explores producer success, coalesced failure, budget exhaustion,
post-clear local reacquisition, eventfd coalescing, and eventfd `EAGAIN`.
Every terminal model state with a published entry must have that entry
consumed, an eventfd wake pending, or the consumer marked runnable. A
schedule-only stress test cannot satisfy this gate.

The mutation matrix must demonstrate that replacing the reactor SeqCst swap
with a Release store admits the lost-wakeup counterexample. It must also
kill producer transitions weakened below SeqCst and removal of the
post-clear acquire recheck.

## Exact epoll user-data encoding

Epoll user data is one reactor-local 64-bit cookie, not a pointer and not a
truncated hash:

```text
bits  0..23  slab slot
bits 24..63  exact connection generation
```

The reactor ID is implicit in the owning epoll descriptor. Startup rejects a
reactor slab capacity above `2^24` slots. Generation zero is invalid. A slot
is permanently retired for the process before its next generation would
exceed `2^40 - 1`; generation never wraps or truncates.

Before acting on an epoll event, the owning reactor:

1. decodes the slot and 40-bit generation;
2. bounds-checks the slot;
3. loads the slab entry;
4. requires the entry to be live and owned by this reactor; and
5. compares the slab's full `u64` generation for exact equality with the
   zero-extended cookie generation.

A failed check is a stale event and performs no socket I/O, state transition,
cancel, or release. The cookie is only the reactor-local stale-event filter.
Every admission result, backend event, timer, cancellation, and control
message still carries and validates the full
`ConnectionKey { reactor_id:u16, slot:u32, generation:u64 }`. The full key is
authoritative across threads.

CPU tests must cover the maximum encodable slot and generation, generation
zero, slot-capacity rejection, permanent retirement at the generation
ceiling, delayed events across close/reuse, and a stale event whose file
descriptor number has already been reused.

## Pre-accept mailbox overflow

The pre-accept mailbox belongs to the backend request registry and is bounded
by both entry and pre-charged output-byte permits. Backend events never block
while waiting for the owning reactor to process an admission result.

If an event cannot enter this mailbox:

1. the registry atomically invalidates that request's completion route with
   terminal reason `slow_consumer`;
2. the rejected event and all retained mailbox events release their exact
   entry and output-byte permits;
3. the backend receives one idempotent cancellation for the request ID;
4. a later successful admission result cannot install the invalidated route
   and instead closes the connection or writes the fixed bounded overload
   response when safe; and
5. serving cleanup remains independent of network terminal delivery.

This is request-local slow-consumer handling. It is never a backend-fatal
ordering error, never blocks a producer, and never discards a resource
permit. Sequence gaps, duplicates, or events after terminal on a route that
was successfully installed remain backend-fatal as specified by v1.

CPU tests must force overflow before the admission result, race overflow
with admission installation, deliver a terminal event as the overflowing
entry, and deliver a stale admission result after cancellation. Each case
must prove one cancel, one terminal registry transition, and zero retained
mailbox entries, output bytes, or request/network permits.

## Listener publication and health

For an ephemeral address, reactor zero may bind first solely to select the
port. The process must not publish that address to a discovery file, proxy,
health response, parent-process readiness channel, or cooperating client
until:

1. every reuseport listener has bound the exact selected address with
   identical required socket options;
2. every listener is registered with its owning epoll descriptor;
3. every reactor, eventfd, admission worker, completion queue, and backend
   dependency has acknowledged the same startup configuration digest; and
4. the server has atomically transitioned to healthy.

Any partial bind or startup failure closes all listeners and publishes no
address. Tests must connect only after the healthy transition and must inject
failure after each listener bind and epoll registration.

## Runnable-list blocking rule

The timeout passed to `epoll_wait` must be exactly zero while any
reactor-local runnable source or connection remains. A positive or infinite
timeout is allowed only when the runnable list is empty after all
clear/recheck handoffs.

Each zero-timeout turn still applies configured per-source and
per-connection budgets and rotates the runnable list. A continuously active
source therefore cannot monopolize a reactor, and retained work never waits
for a timer tick or unrelated kernel readiness.

## Linux-only evidence boundary

The production reactor is compiled only for `target_os = "linux"`. Its CPU
promotion cases, including epoll, eventfd, reuseport, descriptor reuse,
startup rollback, fault injection, and sustained load, must execute on a
pinned Linux kernel and libc. The evidence record includes:

- source commit and binary hash;
- Rust toolchain and target triple;
- Linux kernel release and boot ID;
- libc implementation/version;
- CPU model and topology;
- exact command and immutable transport configuration digest; and
- raw result-bundle hash outside Git.

Portable parser, state-machine, slab, and model-check tests on macOS are
useful development controls but cannot satisfy the Linux CPU gate. A Linux
cross-compile without executing the binary also cannot satisfy it.

## Frozen S05 transport control

The first S05 CPU transport-control run is distinct from the larger S07
fault matrix. It uses the deterministic CPU mock backend and this exact
workload:

```text
schedule schema       glmaxx.s05-transport-control.v1
arrival process       fixed-rate, absolute monotonic deadlines
seed                  0x474c4d4158585335
offered rate          1,000 requests/second
measured duration     120 seconds
warmup                 15 seconds, excluded from measured requests
measured requests     120,000
active client lanes   64
idle keepalive        4,096 established sockets
request mix           3 nonstreaming : 1 streaming by request ordinal
mock response         role at +0 ms, 8 text events at +4..+32 ms,
                      terminal at +36 ms
connection assignment request ordinal modulo 64
```

All pools are allocated and faulted in before warmup. Absolute arrival
deadline `i` is `start + floor(i * 1_000_000_000 / 1_000)` nanoseconds and is
never shifted by an earlier completion. Requests use the retained
deterministic transport fixture. SplitMix64 with the stated seed selects the
fixed body and response-fixture variants but never changes connection
assignment, arrival time, or streaming membership. The 15-second warmup uses
the same 1,000 requests/second schedule and is excluded by an interval
counter snapshot before measured request zero. Each lane receives one
request every 64 ms, leaving 28 ms after the mock terminal before its next
assignment.

Request ordinals congruent to three modulo four are streaming. Version-one
streaming responses close after `[DONE]`, so the driver reconnects that lane
before its next assigned request. The measured interval therefore has
exactly 30,000 streaming closes and 30,000 replacement connections; the
replacement after the last streaming request is still established before
measurement ends. Nonstreaming lanes retain their connections. Model tokens
and model speed are not in scope.

The run passes only when:

- exactly 120,000 requests are offered, accepted, and reach one successful
  terminal state;
- there are zero transport, parse, sequence, stale-route, timeout,
  cancellation, overload, backend-fatal, and driver errors;
- no scheduled arrival is omitted or shifted, driver dispatch-lag p99 is at
  most 1 ms, driver dispatch-lag maximum is at most 10 ms, and the driver
  averages at most 300% of one logical CPU while confined to a pinned
  four-logical-CPU cpuset;
- all 4,096 idle sockets remain valid throughout the measured interval;
- every queue, slab, buffer pool, timer structure, and permit high-water is
  at or below its immutable configured capacity, with no heap fallback;
- after warmup and pool prefault, server RSS never exceeds its baseline by
  more than 16 MiB and its least-squares RSS slope over the measured interval
  is at most +1 MiB/minute;
- measured connection accounting records exactly 30,000 streaming closes
  and 30,000 replacements, the server descriptor count never leaves the
  listener/reactor baseline plus 4,096 idle and 64 active sockets except
  during a bounded close/accept handoff, and after client shutdown returns
  exactly to the listener/epoll/eventfd baseline;
- shutdown proves zero live connection slots, routes, mailbox entries,
  queued output bytes, request/network permits, and admission jobs; and
- a second run with identical bytes produces identical request terminal
  classes, response bytes, and lifecycle totals; both runs independently
  satisfy every resource ceiling.

The run manifest freezes the exact queue/pool capacities, driver CPU
ceiling, mock-backend fixture hash, request fixture hash, Linux evidence
fields, and raw bundle hash. Failure to sustain the fixed rate is a failed
control, not permission to lower the offered rate. Passing this CPU control
does not establish GLM-5.2 throughput, SM120 performance, S07, or any GPU
gate.

## Reverse-proxy identity

The startup configuration digest includes:

- transport and quota configuration;
- accepted backend and completion ABI identities;
- direct/private or proxied deployment mode; and
- for proxied mode, the reverse-proxy binary name, exact version, canonical
  configuration SHA-256, and trust-boundary policy SHA-256.

Direct/private mode uses an explicit domain-separated `no-proxy` value, not
an omitted field or all-zero digest. Production internet-facing health is
forbidden unless a non-`no-proxy` identity is accepted by every reactor and
worker. End-to-end evidence records the same proxy fields and cannot combine
runs across configuration-digest changes.

## Corrected promotion boundary

After an unqualified adversarial acceptance of this amendment, implementation
may begin for the Linux parser, slab, ingress, admission, completion sink,
eventfd protocol, output, timers, and reactor. Promotion still requires:

1. the exhaustive wakeup model and mutation proof;
2. the complete Linux loopback/fault matrix;
3. the frozen S05 CPU transport control;
4. retained blocking-adapter parity as a nonproduction control; and
5. authorized real-backend and proxy qualification in the original order.

This amendment does not accept an implementation, pass S05 or S07, authorize
cn4, or authorize a CUDA launch.
