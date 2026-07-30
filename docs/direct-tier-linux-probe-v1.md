# Linux direct-tier feature and fault probe v1

Date: 2026-07-30

Status: design candidate; adversarial review required before implementation

GPU evidence: none

## Purpose

This contract freezes the nonproduction Linux proof between the accepted
direct-tier CPU state machines and a production `io_uring` authority. It
answers one narrow question:

> Can the exact host, kernel, filesystem, mount, file, and anonymous-memory
> path execute the required fixed-buffer direct-I/O operations without a
> buffered fallback, lost completion, early resource reuse, or ambiguous
> cleanup?

It does not qualify an NVMe device, implement the durable store, move HBM
bytes, or claim serving performance.

## Dependencies

Implementation is blocked until all of these gates are accepted:

- `direct-tier-io-v1-accepted`;
- `direct-tier-extent-cpu-v1-accepted`;
- `direct-tier-state-cpu-v1-accepted`;
- `direct-tier-durable-format-v1-design-accepted`; and
- `direct-tier-scheduler-cpu-v1-r3-accepted`.

The first token is already machine-accepted. The other four are pending at
this design candidate. Review of this document may proceed, but no Linux
implementation may claim those dependencies early.

The implementation must pin the selected Rust `io_uring` crate and every
transitive dependency in `Cargo.lock`. It must not acquire a dynamic
`liburing` dependency or route through C++. The unavoidable syscall and
anonymous-memory operations live in one small Linux-only Rust module with
documented `unsafe` invariants.

## Command and authorization boundary

The future command is:

```text
glmaxx direct-tier-linux-probe SCRATCH_DIRECTORY EVIDENCE_DIRECTORY
```

The command is Linux-only and fail-closed on every other operating system.
It requires two absolute external paths:

- `SCRATCH_DIRECTORY` is the filesystem being probed;
- `EVIDENCE_DIRECTORY` receives immutable JSON and small metadata records.

The command refuses to start unless:

1. both paths already exist and are distinct directories;
2. neither path is inside the repository, a model/checkpoint directory,
   `/`, `/dev`, `/proc`, `/sys`, `/run`, `/boot`, or a mount root;
3. neither path or any opened child is a symbolic link;
4. the scratch directory is empty except for an exact caller-created
   `.glmaxx-direct-probe-v1` sentinel;
5. the evidence directory is empty;
6. an exclusive nonblocking lifetime lock is acquired on the sentinel;
7. the source tree and Git index are clean except for the operator-owned
   `docs/reviews/` directory;
8. the exact executable and source commit can be hashed; and
9. the operator has authorized activity on the named host and filesystem.

The probe never discovers a scratch path, follows a mount automatically,
touches a production cache directory, or deletes an object it did not create
under its unique run identifier.

This document conveys no cn4 authorization. Running the command on cn4 or
its production NVMe path requires a new explicit operator window.

## Fixed baseline

The baseline ring uses:

```text
submission entries       16
completion entries       32
descriptor capacity      16
maximum buffers          16
buffer bytes       2,052,096
buffer alignment       4,096
```

The completion queue is explicitly requested with
`IORING_SETUP_CQSIZE`. The returned SQ and CQ sizes must be exactly 16 and 32
or startup fails; kernel clamping is not accepted. The ring is created with
`IORING_SETUP_SINGLE_ISSUER`. The creating authority thread performs the
first and every later submission.

The baseline forbids:

- `IORING_SETUP_SQPOLL`;
- `IORING_SETUP_IOPOLL`;
- shared submission from another task;
- provided-buffer rings;
- linked timeout semantics as a correctness dependency;
- cooperative/deferred task-run flags; and
- any optional feature that changes the matched control.

Optional flags may receive a later measured profile only after the baseline
passes and the evidence records a distinct route.

## Filesystem and file identity

The probe opens the scratch root once and resolves children relative to that
directory descriptor with no symlink or path traversal. Linux
`openat2(2)` resolution must enforce beneath/no-symlink/no-magic-link
semantics. Absence of that syscall is a typed startup failure, not a fallback
to string path checks.

One run creates:

```text
probe-<run-id>.data
probe-<run-id>.barrier
```

Both names are created exclusively and contain the unpredictable 128-bit run
identifier. Existing names are never reused or truncated.

The data file is opened:

```text
O_RDWR | O_CREAT | O_EXCL | O_DIRECT | O_CLOEXEC | O_NOFOLLOW
```

The barrier file is opened with the same identity and safety flags, without
requiring `O_DIRECT`. The evidence binds each descriptor to:

- mount ID;
- filesystem type and UUID where available;
- device major/minor;
- inode;
- opened flags;
- block and direct-I/O alignment reported by the kernel/filesystem; and
- the physical block device identity or an explicit `not-resolved` field.

The data file is preallocated before the first ring submission. Preallocation
failure, including `ENOSPC`, publishes no passing evidence and starts no I/O.
The file contains sixteen maximum-size, 4,096-aligned slots so every
descriptor can own a disjoint file and memory range.

## Anonymous registered buffers

Each of sixteen buffers is a distinct anonymous private mapping of exactly
2,052,096 bytes. Anonymous `mmap` must return at least page alignment; the
probe additionally rejects an address not divisible by 4,096.

Before registration:

1. the complete mapping is zeroed;
2. `MADV_DONTFORK` succeeds;
3. the mapping and its index are recorded in the fixed buffer table;
4. the aggregate locked/pinned accounting is checked against the effective
   process and cgroup limits; and
5. no mapping is exposed to a descriptor.

All sixteen mappings are registered in one fixed-buffer table. The probe
uses `IORING_OP_READ_FIXED` and `IORING_OP_WRITE_FIXED`; a non-fixed opcode
is a test failure. The kernel registration result and effective locked-memory
accounting are evidence fields.

The data and barrier descriptors are registered in one fixed-file table.
Every I/O SQE uses `IOSQE_FIXED_FILE`. A raw file descriptor in an accepted
trace is a failure.

The implementation must account for a future second pin by CUDA host
registration but does not perform it in this probe. The production planner
must reserve at least twice the registered byte total before that integration
can become healthy. No result here claims CUDA-pinned memory.

## Operation identity

The CPU-reviewed `DirectCompletionToken` remains the only SQE `user_data`
encoding. One fixed descriptor entry retains:

```text
buffer slot and full u64 generation
operation generation
READ | WRITE | FSYNC role
original-pending bit
cancel-pending bit
expected file slot, offset, and exact length
```

The kernel-facing token is a lookup key, never the complete resource
identity. A CQE is resolved against the descriptor table before its result is
interpreted.

The authority rejects:

- zero or unknown `user_data`;
- stale descriptor or buffer generation;
- wrong original/cancel role;
- a CQE for an unsubmitted operation;
- duplicate completion;
- result bytes different from the exact requested length;
- a completion for a different fixed file, buffer, offset, or length; and
- any completion after the descriptor was made reusable.

## Required feature matrix

The probe executes both accepted physical lengths:

```text
target-only  2,019,328 bytes = 493 * 4,096
MTP          2,052,096 bytes = 501 * 4,096
```

For each length it must:

1. materialize the canonical direct extent with deterministic nonzero
   logical pieces and zero padding;
2. submit one `WRITE_FIXED` through a registered file and buffer;
3. require one exact positive CQE result equal to the requested length;
4. submit `IORING_OP_FSYNC` with the data-only form and require success;
5. zero a distinct registered buffer;
6. submit one `READ_FIXED`;
7. require the exact length;
8. run the accepted direct-extent decoder over the read buffer; and
9. compare the physical digest, every logical-piece digest, and every byte
   with the pre-write oracle.

The barrier file separately receives:

1. an ordinary bounded metadata write before ring submission;
2. one ring `FSYNC` without the data-only flag; and
3. a reopen/readback check after the ring and registered file table are
   closed.

Opcode-probe metadata is recorded but is not sufficient. Each required
opcode must complete successfully on the exact opened files and registered
buffers.

## CQ capacity and NODROP independence

The returned `IORING_FEAT_NODROP` bit is recorded. Correctness is identical
whether it is absent or present.

The saturation trace:

1. owns all sixteen descriptors and buffers;
2. submits sixteen disjoint fixed reads without consuming a CQE;
3. requests one async cancel per original token;
4. never has more than 32 expected original/cancel CQEs;
5. reaps every CQE and classifies each cancel result;
6. requires exactly one original and one cancel CQE per descriptor;
7. accepts an original result racing ahead of cancel and the corresponding
   cancel-not-found result;
8. never releases a descriptor or buffer until both CQEs are reaped; and
9. requires the kernel CQ overflow counter to remain zero.

A platform on which the sixteen reads finish before cancels are submitted
still proves exact double-CQE accounting, but does not claim a
cancel-won race. The CPU state proof remains responsible for exhaustive
original/cancel order semantics.

The probe never intentionally exceeds the returned CQ size. Deliberate CQ
overflow would test data loss by causing the condition the production design
forbids.

## Fail-closed fault matrix

The Linux boundary is split into an injectable submission/completion adapter
and the real syscall adapter. The same authority state machine consumes
both. The injected adapter is deterministic and cannot be selected by a
production-health constructor.

The injected matrix covers, at every legal transition:

- submission `EINTR` before any SQ tail change;
- submission `EAGAIN` before any SQ tail change;
- CQE `-EINVAL` for address, offset, or length misalignment;
- short positive read and write;
- CQE `-EIO`;
- device-loss/filesystem-unavailable results such as `-ENODEV`;
- CQE `-ENOSPC`;
- async-cancel success, not-found, already-completed, and failure;
- data-file fsync failure;
- barrier/journal-role fsync failure;
- fixed-file registration/update invalidation;
- fixed-buffer registration/update invalidation;
- unknown, stale, wrong-role, and duplicate `user_data`;
- an unexpected CQ overflow observation; and
- shutdown during queued, submitted, and partially reaped operations.

`EINTR` or `EAGAIN` may be retried only when the adapter proves that no SQ
tail or descriptor state was committed. A short positive result is never
continued with a second direct operation; the whole extent fails and its
buffer is quarantined.

The real adapter must independently produce and reject:

- a 4,095-byte-misaligned userspace address;
- a 4,095-byte-misaligned file offset;
- a length reduced by 4,096;
- a short read caused by an EOF one block before the requested end; and
- a fixed-file SQE after the registered slot has been removed.

The exact kernel errno/result is recorded. The required property is
fail-closed rejection and final ownership, not one hard-coded errno across
filesystems.

Real device loss and destructive ENOSPC injection are not performed on an
arbitrary filesystem. They remain mandatory for the later operator-approved
target-storage fault gate.

## Cleanup transaction

Normal and error cleanup use one transaction:

1. close admission;
2. logically abandon every waiter;
3. submit at most one cancel for each eligible original;
4. reap every original, cancel, and fsync CQE;
5. prove zero descriptor, CQE, waiter, permit, and lease ownership;
6. unregister fixed files;
7. unregister fixed buffers;
8. close the ring;
9. unmap every anonymous buffer;
10. close both scratch descriptors;
11. unlink only the two run-ID-owned files;
12. fsync the scratch directory; and
13. release the sentinel lock.

Failure to prove zero ownership prevents an unmap or descriptor close during
normal execution. The evidence reports incomplete cleanup and the command
terminates fail-closed, relying on process exit for kernel reclamation. It
never reports health after a teardown error.

Registration update or ring destruction is not assumed to make old
resources immediately reusable. The probe requires explicit drain and
unregistration before unmap.

## Evidence record

The single result is
`glmaxx.direct-tier-linux-probe.v1`. It includes:

- PASS or typed failure, written last;
- source commit, executable SHA-256, dirty-state result, Rust version,
  target triple, and complete `Cargo.lock` SHA-256;
- UTC start/end timestamps and monotonic elapsed nanoseconds;
- hostname, boot ID, kernel release, architecture, effective UID/GID,
  process limits, cgroup identity, and container identity when present;
- scratch/evidence path identities without credentials;
- mount, filesystem, inode, alignment, and block-device observations;
- requested and returned SQ/CQ sizes, setup flags, feature bits, and opcode
  probe results;
- registered file/buffer counts and aggregate bytes;
- one record for every SQE and CQE in deterministic logical order;
- target/MTP write, fsync, read, byte-compare, and digest results;
- every injected and real boundary-fault result;
- maximum outstanding descriptors/CQEs and observed CQ overflow counter;
- teardown receipts and final zero accounting; and
- explicit nonclaims for NVMe qualification, CUDA, model execution,
  capacity, latency, throughput, and serving.

Raw kernel timestamps and completion arrival order may vary. The normalized
result sorts by logical operation identity and records arrival ordinal
separately. Repeated runs must agree on every semantic field while
environment/time/run-ID fields are explicitly classified as variable.

The evidence directory is external and remains out of Git. A small
provenance summary may be committed only after hashing the immutable raw
record.

## Implementation and acceptance gates

After adversarial design acceptance:

1. implement the injectable adapter and exhaustive CPU fault matrix;
2. obtain a separate CPU implementation review;
3. implement the Linux syscall adapter behind an off-by-default Cargo
   feature;
4. run it on a nonproduction Linux filesystem under explicit authorization;
5. obtain an evidence review;
6. run the same candidate on the exact target store path under a new
   operator-approved window; and
7. only then integrate the adapter into the production direct-tier
   authority.

Acceptance of this design or a nonproduction probe does not pass K03 or K05,
authorize cn4, qualify storage, permit destructive migration, or establish
any performance claim.

## Primary ABI references

- Linux `io_uring_setup(2)` and setup flags:
  <https://man7.org/linux/man-pages/man2/io_uring_setup.2.html>
- Linux `io_uring_register(2)`:
  <https://man7.org/linux/man-pages/man2/io_uring_register.2.html>
- Linux registered-buffer behavior:
  <https://man7.org/linux/man-pages/man7/io_uring_registered_buffers.7.html>
