# Fable handoff: cn4 read-only environment capture v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`d961d96da3f4b99052272afcd4abf28dbe6f9854`

Required result path:
`docs/reviews/fable-cn4-environment-capture-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`cn4-environment-capture-v1-design-accepted`

GPU, host, process, container, or storage authorization conveyed by this
handoff: none

cn4 posture: do not connect to cn4, query its current state, start a
container, build, test, create a CUDA context, launch work, stop a process,
or modify any host resource for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/cn4-environment-capture-v1.md` | `f3ebd6d63f6aa74c7b797f528bd11778c819f568d14efe5862b3fb92fad4cbaa` |
| `docs/cn4-container.md` | `b68a8bdcd279be3b46340cc55d43f6bfa73ac2c4ea1308b16ca52a0cd1ab26ff` |
| `docs/cn4-release-20260729.md` | `7ea72b51d5aa44250927cb4e3dd363340838ed0c0d35fcb50a4cc1c042f601fe` |
| `containers/cn4-dev.Dockerfile` | `dbf3fe3ba2ab260f64f0377823d6c4dcb34c9c781f02f3c116efc580e60324ff` |
| `scripts/cn4-phase-b-prepare.sh` | `ec10e66a028f8859504007edb03be2361fae6782c41d5bdefa97055dada08a9d` |
| `scripts/cn4-phase-b.sh` | `9ace5b4d4b0e8d2d1ee048bc32295cf86d7393b8420c5653b7d2f9faca23dd6d` |
| `scripts/cn4-rank-bind-smoke.sh` | `452da422e27bedc629a9919462cbe4ef2cdfcbe8b67a1df73d2519bd99525091` |
| `scripts/cn4-checkpoint-load-smoke.sh` | `fb4f82b76fe7d43dfa29168a1a9f9b3dab138a065f410a1dab31e26fe5ac5e36` |
| `docs/production-punchlist.md` | `413b9919f4d4b69b44ec7228c44a586c0252444189935eeed425f37a8d98ccc9` |
| `docs/results-index.md` | `d7c25b3c9e998858707a062ecc9f965f04f45197ea00e31a5263c645f5159770` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-cn4-environment-capture-v1-handoff.md
git diff --check d961d96da3f4b99052272afcd4abf28dbe6f9854^ \
  d961d96da3f4b99052272afcd4abf28dbe6f9854
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. The design cites public interface documentation, but this
review is source-only: do not execute any proposed command.

## Review purpose

Determine whether the design can produce a reproducible, secret-minimized
cn4 environment identity without creating CUDA work or turning inventory
success into launch authority.

Attack the separation between:

1. the no-GPU container build attestation;
2. the copied host executable;
3. the local immutable image identity;
4. current source and CUTLASS bytes;
5. target hardware/topology/occupancy; and
6. later, separately authorized CPU or GPU execution.

## Review boundary

Acceptance covers only the proposed commands, parsers, allowlists, evidence
transaction, identities, verdict precedence, and implementation sequence.

Acceptance does not cover an implementation, binary, container run, cn4
connection, host observation, CPU test, CUDA context, kernel, checkpoint,
model execution, storage, capacity, latency, throughput, or production
health.

## Required adversarial questions

1. Do all thirteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Is implementation explicitly gated on this review, while every cn4 query,
   container/build/test action, and device launch remains separately
   authorized?
3. Can a binary built by one source/toolchain/image be paired with a build
   attestation from another and still pass the executable, source, Cargo
   lock, image, and toolchain bindings?
4. Can a tag, mutable image reference, empty claimed registry identity,
   wrong platform, forged local image ID, or locally built image with
   unbound rootfs/Dockerfile/base digests pass?
5. Can source, CUTLASS, attestation, evidence, mount-root, symlink, overlap,
   ownership, dirty-tree, untracked-file, or origin ambiguity escape the
   stated path and source checks?
6. Can operator text, a PATH substitution, shell metacharacter, environment
   value, command prefix, or optional-host-tool route extend the compiled
   allowlist or express a modifying NVIDIA, Docker, Git, package, process, or
   storage action?
7. Are child stdin, environment, executable identity, time, output, UTF-8,
   exit, signal, kill/reap, and parse boundaries complete and implementable
   without leaking a child or accepting truncated evidence?
8. Can any permitted command or file read expose credentials, environment
   secrets, command-line arguments, Docker config/history, SSH state, model/
   checkpoint/cache contents, filesystem contents, or device serial numbers?
9. Are exactly four distinct RTX PRO 6000 Blackwell Workstation Edition
   devices with compute capability 12.0, UUID/BDF uniqueness, and equal
   reported memory required, while index ordering remains non-authoritative?
10. Are required versus optional NVIDIA fields explicit, and do recorded
    NVIDIA-SMI help/version hashes plus sysfs BDF confinement prevent an
    unsupported field or textual-output change from becoming an invented
    value?
11. Does topology parsing require a complete symmetric four-GPU matrix,
    retain NUMA/parent-path identity, distinguish all named PCIe/NVLink
    relations, record layout changes rather than assume Gen3 switch pairs,
    and avoid inferring performance?
12. Do two occupancy samples, the exact 256 MiB ceiling, application-set and
    identity stability checks, and deterministic verdict precedence prevent
    an occupied, changing, mismatched, or toolchain-invalid host from being
    called idle?
13. Can `INVENTORY_PASS_OCCUPIED` or `INVENTORY_PASS_IDLE` be interpreted as
    authority to stop a process, start a container, run CPU work, create a
    context, or launch a kernel?
14. Does the evidence transaction create only a fresh operator-owned external
    directory, publish the canonical manifest last with sync/no-replace
    semantics, retain every raw hash and child receipt, and refuse a pass
    after partial publication?
15. Does the implementation sequence preserve design review, CPU parser/
    allowlist proof, no-GPU build attestation, renewed read-only window,
    independent evidence review, separately authorized CPU reproduction,
    idle recheck, and new device authorization in that exact order?
16. Are all no-implementation, no-cn4, no-container, no-CUDA, no-kernel,
    no-model, no-storage, no-capacity, and no-performance nonclaims exact?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. build/container/source/executable identity is closed end to end;
2. the command and path allowlists cannot mutate state or ingest secrets;
3. SM120 device, firmware, PCIe, NUMA, and topology identity is exact;
4. idle, occupied, unstable, target, source, and toolchain classifications
   are deterministic and fail closed;
5. evidence publication is atomic, durable, bounded, and externally scoped;
6. an occupied inventory remains useful without becoming launch authority;
7. the staged implementation and later execution order obeys the repository
   gate sequence; and
8. no cn4, CPU, container, CUDA, model, storage, capacity, or performance
   evidence is implied.

Only if all sixteen questions and all eight statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, a binary/attestation/image substitution,
mutable or ambiguous identity, unsafe path, command injection, modifying
operation, secret exposure, target/topology ambiguity, false idle
classification, incomplete publication, authorization leakage, or any
implementation/hardware/performance overstatement.
