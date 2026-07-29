# Nonblocking HTTP transport v1

Date: 2026-07-29

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Scope

This contract replaces the retained blocking HTTP server with a bounded
Linux nonblocking transport for the GLM-5.2-only service. It defines:

- sharded socket ownership and readiness processing;
- authenticated, quota-bound request ingress;
- off-reactor JSON validation, tokenization, and backend submission;
- backend-to-reactor completion delivery;
- streaming and buffered response backpressure;
- disconnect, cancellation, timeout, fatal, and shutdown behavior; and
- concurrency and fault evidence required before production promotion.

It deliberately targets the production Linux host. It does not promise a
non-Linux transport, TLS termination, HTTP/2, WebSocket, request pipelining,
or a general web framework.

## Current blocking gaps

The current `ApiHttpServer` is a bounded functional control:

1. one accept thread sends sockets through one bounded channel;
2. a worker holds one socket for its complete request and generation;
3. streaming workers block in `recv_timeout`, synchronous writes, and
   per-chunk flushes;
4. a slow or long generation permanently occupies one connection worker;
5. `ApiCompletionHandle` exposes a standard channel receiver that cannot join
   a socket readiness loop;
6. request parsing reads the complete body before authentication;
7. JSON parsing, prompt rendering, and tokenization execute synchronously on
   the connection worker;
8. every response closes the connection;
9. there is no connection-generation guard against stale async completion;
10. metrics do not separate accepted sockets, ingress, write stalls, or
    transport cancellation.

This path is safe enough for CPU proof but cannot establish S05 or sustained
multi-user throughput.

## Chosen architecture

Version one uses direct Linux `epoll`, `eventfd`, nonblocking sockets, and the
existing Rust `libc` dependency. It does not add an async runtime.

There are three bounded ownership domains:

```text
SO_REUSEPORT listener + epoll reactor shard
    -> admission/parse/tokenization worker pool
        -> single serving/backend authority
            -> reactor completion queue + eventfd
```

Each file descriptor, connection slot, request body, output segment, and
completion route has exactly one owner. Cross-domain transfers are move-only
messages through bounded queues.

The reactor count, worker count, connection slots, queue entries, buffer
bytes, per-tick work budgets, and deadlines are immutable startup
configuration. Production health requires every reactor and worker to start
and acknowledge the same configuration digest.

## Reactor sharding

Each reactor owns:

- one nonblocking `SO_REUSEPORT` listener on the same address;
- one epoll descriptor;
- one nonblocking close-on-exec eventfd;
- a fixed-capacity connection slab;
- fixed-size header and output-segment buffer pools;
- one bounded completion/control queue;
- a bounded deadline heap or hierarchical timer wheel; and
- no sockets owned by another reactor.

For an ephemeral port, reactor zero binds first, reports the selected port,
and every other listener binds that exact address with the same reuse
settings before the server becomes healthy.

Accepted sockets set:

- `O_NONBLOCK`;
- `FD_CLOEXEC`;
- `TCP_NODELAY`;
- bounded kernel receive and send buffers; and
- keepalive only if explicitly configured.

Failure to apply a required option closes that socket. Listener or epoll
failure makes the server unhealthy; an individual malformed client does not.

No accepted socket is sent to another reactor. This removes the central
connection queue and cross-thread socket ownership.

## Connection identity and slab

A connection is identified by:

```text
ConnectionKey.v1 {
    reactor_id: u16
    slot: u32
    generation: u64
}
```

Generation zero is invalid. Reusing a slab slot increments its generation
before epoll registration. Overflow retires the slot until restart.

Epoll user data contains the reactor-local slot and generation-derived
cookie, never an unvalidated pointer. Every admission result, backend event,
timeout, cancellation, and control message carries the full key plus request
ID when one exists.

A message for a closed or reused connection cannot write a response. The
backend request still reaches terminal cleanup through its resource permit;
discarding a stale network event never discards serving resources.

The connection state machine is:

```text
ACCEPTED
  -> READING_HEADERS
  -> WAITING_INGRESS_PERMIT
  -> READING_BODY
  -> ADMISSION_QUEUED
  -> WAITING_BACKEND
  -> WRITING_BUFFERED | WRITING_STREAM
  -> KEEPALIVE_IDLE | CLOSING
  -> CLOSED
```

Only one request may be in flight per connection. HTTP pipelining is not
supported. Body reads are capped to the exact remaining declared length. If
the header read already captured bytes beyond that body, the connection is
closed after a fixed malformed-request response. Later bytes remain in the
kernel receive buffer until the prior response completes. A client never
obtains two concurrent request permits on one socket.

## Readiness rules

Reactors use edge-triggered epoll with:

- accept until `EAGAIN`;
- read until `EAGAIN`, buffer/permit limit, or parser terminal state;
- write until `EAGAIN`, per-connection budget, or output empty;
- `EPOLLRDHUP`, `EPOLLHUP`, and `EPOLLERR` as disconnect paths; and
- interest modification only by the owning reactor.

Every readiness path has a configured operation and byte budget. A hot socket
is requeued locally after consuming its budget so one client cannot starve
other connections or the completion eventfd.

`EINTR` retries without changing state. `EAGAIN` is not an error. Zero-byte
read is peer close. Partial reads and writes advance exact cursors and never
re-serialize or duplicate bytes.

The reactor performs no blocking file I/O, DNS, model work, tokenization,
sleep, channel receive, or unbounded allocation.

## Header and ingress boundary

Headers are incrementally parsed into a bounded 32,768-byte buffer. Version
one accepts HTTP/1.1 with exactly one `Content-Length`, rejects
`Transfer-Encoding`, duplicate security-sensitive headers, invalid UTF-8
header names/values, obsolete folding, invalid method/path/version, and a
body above the configured maximum.

After the header is complete:

1. route endpoints that require authentication;
2. authenticate the bearer token with constant externally visible failure;
3. acquire the tenant/global ingress request and declared-body-byte permit;
4. allocate body storage only from the permitted bounded pool; and
5. resume body reads.

This is the quota contract's required authenticate-before-body-allocation
boundary. `/health` may remain unauthenticated and bodyless. `/metrics`
authentication is an immutable deployment choice and cannot expose tenant
labels.

The ingress permit remains held while the request waits for an admission
worker. Queue-full does not block the reactor: it returns a fixed overload
response if safe and releases the permit.

## Admission worker boundary

Reactors do not parse JSON or tokenize. A move-only `AdmissionJob.v1` carries:

```text
connection_key
method and canonical route
authenticated tenant
request body
ingress permit
request deadline
streaming preference after validation
```

The fixed worker pool:

1. parses the exact supported schema;
2. validates GLM-5.2-only fields;
3. renders the pinned chat template;
4. tokenizes with the pinned tokenizer;
5. atomically exchanges ingress charges for a queue permit;
6. submits to the serving backend; and
7. returns a bounded result to the owning reactor.

Admission workers may block on CPU work but never on generation output. They
do not own sockets and cannot write responses.

A stale connection result causes backend cancellation if submission already
succeeded, then releases every untransferred permit. Submission and stale-key
checking have an explicit ordering:

- the worker checks key liveness before backend submission;
- the reactor may still close immediately afterward;
- successful submit registers `(connection_key, request_id)` with the
  backend/resource registry before the result is published; and
- the stale result path sends idempotent cancellation by request ID.

There is no moment when a submitted request lacks both a network route and a
cleanup registry entry.

Backend events can be produced before the owning reactor processes the
successful admission result. Such events enter the route's bounded
pre-accept mailbox and are not serialized. After the reactor installs the
exact `(connection_key, request_id)` mapping, it emits the role chunk and
drains mailbox events in sequence order. If the admission result is stale or
failed, the mailbox is discarded under output-byte accounting and the
request is cancelled. This ordering prevents text from reaching the wire
before the HTTP status/role even when admission and runtime threads race.

## Completion sink ABI

The production backend no longer returns a blocking receiver. Submission
binds a `CompletionSink.v1`:

```text
CompletionSink.v1 {
    connection_key
    request_id
    next_event_sequence
    reactor_queue
    wake_state
}
```

Backend events remain ordered:

```text
role-start
text delta*
one terminal finished | failed
```

Every queued envelope carries its monotonically increasing event sequence.
The reactor rejects a duplicate, gap, event after terminal, or sequence
overflow as a backend-fatal ordering violation.

The role-start event may be synthesized by the reactor after successful
streaming admission. No text or terminal event may precede that result.

Producers use nonblocking `try_push`. A full completion queue or exhausted
output-byte permit marks the request a slow consumer, triggers idempotent
backend cancellation, and preserves serving cleanup. The compute/coordinator
thread never waits for socket progress.

The existing blocking receiver may remain as a test/control adapter, but a
server using it cannot report production transport health.

## Eventfd wake protocol

Every reactor completion queue has one atomic `notified` bit.

Producer:

1. publish the queue entry with release ordering;
2. change `notified` from false to true;
3. on the successful transition, write one to eventfd;
4. treat `EAGAIN` as an already-pending wake; and
5. treat other eventfd failures as reactor-fatal.

Reactor:

1. drain eventfd to `EAGAIN`;
2. drain completion entries to the configured fairness budget;
3. if the budget expires while entries remain, keep `notified` set and put
   the completion source on the reactor-local runnable list before another
   blocking epoll wait;
4. clear `notified` with release ordering only after observing the queue
   empty;
5. acquire-recheck the queue; and
6. if nonempty, set `notified` and continue draining or self-wake.

The CPU proof explores a producer enqueue at every boundary between steps
two through five. No published entry may remain asleep indefinitely, and no
number of coalesced entries requires one eventfd count per event.

## Output ownership

Output bytes are charged before entering a reactor queue.

Each request has:

- a bounded completion-entry count;
- a bounded serialized-output byte count;
- a bounded number of fixed-size segments; and
- one response cursor owned by its reactor.

Serialization occurs in an admission/output worker or in a budgeted reactor
routine only when its maximum size is known. Metrics snapshot rendering also
uses a bounded worker, not a reactor. A model token must not trigger an
unbounded allocator call on the coordinator thread.

For streaming responses, the wire order is:

1. HTTP 200 SSE headers;
2. role chunk;
3. zero or more content chunks;
4. one final chunk or structured error;
5. `data: [DONE]`; and
6. connection close.

Version one intentionally closes streaming connections after `[DONE]`; it
does not need chunked transfer coding or ambiguous response delimitation.

Nonstreaming responses use exact `Content-Length` and may enter bounded
keepalive idle after the complete write. `/health`, authenticated
`/metrics`, and cancellation responses may also use keepalive. The
configuration caps requests per connection and idle duration.

Partial writes preserve byte-exact segment order. `EPOLLOUT` is enabled only
while output exists and disabled immediately when drained.

## Slow-client policy

A client is slow when any configured boundary is crossed:

- completion queue entry limit;
- per-request serialized-output bytes;
- per-tenant or global buffered-response quota;
- no forward write progress for the write-stall deadline; or
- connection lifetime/request deadline.

The reactor:

1. marks the connection closing;
2. sends one idempotent cancel if a request is registered;
3. releases queued network bytes;
4. closes the socket; and
5. leaves serving/resource cleanup independent of terminal delivery.

It does not block the backend to send a final explanatory chunk to a client
that already violated backpressure.

One slow stream must not reduce event processing or output progress for a
peer reactor slot beyond its bounded per-tick work.

## Deadlines

All deadlines are monotonic:

- header completion from accept;
- body completion from ingress permit acquisition;
- admission-worker queue wait;
- total model request from accepted queue permit;
- write stall from last positive write;
- keepalive idle; and
- graceful shutdown.

Timer handling carries the connection generation. A stale timer cannot close
a reused slot.

Deadlines use a bounded timer structure. Lazy deletion is allowed only with a
hard bound on stale entries; otherwise cancellation must remove the timer.
Wall-clock time is used solely for OpenAI response timestamps, never timeout
ordering.

## Disconnect and cancellation

A disconnect before backend submission releases ingress/body/admission
resources. A disconnect after submission sends exactly one idempotent cancel
and invalidates the completion route.

Explicit `DELETE /v1/requests/{id}` continues to verify authenticated tenant
ownership. The deletion request is independent of the generation connection
and returns after the backend accepts cancellation, not after GPU cleanup.

Stop-string completion can cause backend cancellation after text termination.
The network route still accepts only one terminal lifecycle result.

Race order is defined by the reactor's serialized connection state:

- terminal event before disconnect: queue terminal bytes, then disconnect may
  discard them and release network charges;
- disconnect before terminal event: invalidate route and cancel; the stale
  terminal event is discarded;
- timeout concurrent with terminal: the first serialized transition wins;
  the loser performs no second close/cancel/release.

## Health, fatal, and shutdown

Production transport health requires:

- every listener, epoll descriptor, eventfd, reactor, and admission worker
  alive;
- completion/control queues connected;
- backend four-rank health;
- quota and transport configuration digests accepted; and
- no fatal transport invariant.

One reactor fatal transitions the entire API to fatal. Partial reactor
service would make connection routing and load claims ambiguous.

Graceful shutdown:

1. change health to draining and stop accepting;
2. reject new admission jobs;
3. allow registered requests to finish until the grace deadline;
4. cancel all remaining request IDs;
5. drain completion/control queues without writing after socket close;
6. release every network and quota permit;
7. close descriptors owned by each reactor; and
8. join every worker and prove zero live slots/bytes/requests.

Process-fatal backend drain may precede transport shutdown. Structured errors
are queued only when capacity and socket liveness permit; cleanup never waits
for delivery.

## TLS and deployment boundary

Version one speaks cleartext HTTP/1.1 on a trusted host interface. Internet
TLS, HTTP/2, compression, request normalization, and public rate limiting
belong to a pinned reverse proxy.

The proxy must disable response buffering for SSE, preserve
`Authorization`, impose compatible or tighter body/header/time limits, and
record its version/config digest in end-to-end evidence.

The engine does not trust forwarding headers for authentication, tenant ID,
client IP, scheme, or host decisions. Binding beyond loopback/private
interfaces without the reviewed proxy is a deployment failure.

## Memory bound

Startup computes and reports:

```text
connection slab metadata
+ fixed header-buffer pool
+ admitted body-byte pool
+ admission queue/job metadata
+ completion queue metadata
+ serialized output-segment pool
+ timer metadata
+ reactor control queues
+ fixed emergency error responses
```

The sum uses checked arithmetic and must fit the configured host-memory
budget before binding. Kernel socket buffers and reverse-proxy buffers are
reported separately.

Header capacity is not eagerly allocated as 32 KiB for every idle
connection. Fixed header chunks are acquired while reading and returned on
completion. A connection unable to acquire its next bounded chunk is rejected
without heap fallback.

No configuration may derive memory from `connections * maximum_body_bytes`;
body bytes are a separately reserved global/tenant pool. Likewise, maximum
buffered response bytes are pooled and quota-bound rather than eagerly
reserved per socket.

## Observability

Fixed-cardinality transport metrics include:

- listeners/reactors/workers healthy;
- accepted, active, idle, and closed connections;
- connection closes by fixed reason;
- ingress/admission/completion queue current and high-water entries;
- body/header/output pool current and high-water bytes;
- epoll events, accepts, reads, writes, `EAGAIN`, and partial I/O;
- eventfd writes, coalesced wakes, and drain batches;
- admission-worker queue/tokenization latency;
- first-response-write and write-stall latency;
- slow-consumer cancellations;
- stale completion/timer messages;
- keepalive request count; and
- shutdown drain duration and forced cancellations.

Reactor ID may be a bounded label. Tenant, request, connection generation,
token, prompt, API key, and arbitrary error text are forbidden labels.

Transport latency is reported separately from model TTFT and ITL. A network
benchmark never substitutes reactor service time for kernel or end-to-end
model time.

## Required CPU and fault proof

Before production promotion, tests cover:

1. listener/epoll/eventfd startup and complete rollback on partial startup;
2. thousands of idle sockets with bounded thread and memory count;
3. accept/read/write loops through repeated `EINTR`, `EAGAIN`, and partial I/O;
4. fragmented headers at every byte boundary;
5. duplicate length/auth headers, invalid folding, invalid UTF-8, oversized
   header/body, chunked encoding, truncation, and extra pipelined bytes;
6. authentication and ingress permit before body allocation;
7. admission queue saturation without reactor blockage;
8. stale admission result before and after backend submission;
9. connection slot and file-descriptor reuse with delayed completion/timer;
10. eventfd producer enqueue at every clear/recheck interleaving;
11. completion queue full with exact cancel and resource release;
12. one slow stream beside at least 63 progressing peers;
13. disconnect before submit, after submit, during text, and during terminal;
14. request, write-stall, header, body, admission, and idle timeout races;
15. nonstreaming maximum response minus/equal/plus one byte;
16. exact SSE role/text/final/error/`[DONE]` ordering under partial writes;
17. stop-string cancellation with no duplicate terminal or release;
18. authenticated cross-tenant DELETE rejection;
19. backend fatal with active, queued-admission, and idle connections;
20. one reactor/eventfd/admission-worker injected fatal;
21. graceful shutdown before and after its deadline;
22. zero connection slots, request routes, permits, and pooled bytes after
    every success and injected failure;
23. deterministic response bytes and lifecycle totals across repeated runs;
24. C1/C64 active generations plus 4,096 idle/queued connections;
25. mixed streaming/nonstreaming/health/metrics/cancel traffic;
26. bounded fairness under a continuously readable and writable hot socket;
27. reverse-proxy buffering disabled in a retained deployment smoke; and
28. a sustained load record with open-loop arrival rate, achieved throughput,
    TTFT/ITL/request latency, queue high-water, errors, cancellations, CPU,
    RSS, file descriptors, and context switches.

The CPU mock backend must support delayed events, bursts, malformed terminal
order, dropped routes, and injected fatal states. Passing only loopback
request/response examples is insufficient.

## Promotion gates

1. adversarial review of this transport, resource quota, backend,
   observability, and benchmark contract;
2. pure CPU parser/state/eventfd interleaving proof;
3. loopback functional and fault matrix;
4. sustained CPU mock-backend load with fixed resource ceilings;
5. authorized real-backend load on SM120 without claiming kernel speed from
   the CPU controls;
6. reverse-proxy deployment smoke;
7. matched cold/warm, context, concurrency, and MTP end-to-end matrix.

The retained blocking server remains a test control until steps 1–4 pass.
It must be clearly named nonproduction and cannot satisfy S05.
