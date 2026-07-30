# cn4 read-only environment capture v1

Date: 2026-07-30

Status: design candidate; adversarial review required before implementation

GPU evidence: none

## Purpose

This contract freezes the H01/D02 evidence boundary for the next cn4
development window. It produces one exact, secret-minimized identity for:

- the GLMAXX source and executable;
- the pinned build container and CUTLASS checkout;
- the host, kernel, CPU, memory, NUMA, filesystem, and toolchains;
- all four SM120 GPUs, their firmware and live PCIe state;
- the PCIe/NUMA topology and absence or presence of NVLink; and
- GPU occupancy before and after capture.

The capture is read-only. It does not build, test, create a CUDA context,
launch a kernel, stop a process, change clocks or power, reset a device,
start a container, inspect secret-bearing container configuration, or grant a
later GPU launch.

## Why the existing scripts are not the authority

The preparation and smoke scripts retain useful historical evidence, but
each captures a different subset. They do not establish one canonical
identity for firmware, host/NUMA state, PCIe link width/generation, executable
bytes, command availability, and before/after occupancy.

Their broad textual records are also not a safe schema boundary. In
particular, generic process arguments, complete container inspection, and
environment dumps can expose credentials. This design replaces neither the
scripts nor their gate-specific evidence; it supplies the common immutable
environment record that later runs must reference.

## Gate and authorization boundary

Design review may occur on any host. It conveys no cn4 authorization.

Implementation begins only after:

```text
cn4-environment-capture-v1-design-accepted
```

The future read-only capture is permitted only when the operator confirms
that an inventory query will not disturb a running test. CPU reproduction,
container creation, compilation, CUDA context creation, device execution,
process control, shared-memory cleanup, and storage probing each remain
separate actions requiring their applicable authorization.

An occupied machine still produces useful evidence. It receives
`INVENTORY_PASS_OCCUPIED`, never an idle or launch-ready verdict.

## Rust ownership and commands

The default-feature Rust CLI is built inside the pinned no-GPU development
container. Before copying, that binary emits
`glmaxx.build-environment-attestation.v1`:

```text
glmaxx build-environment-attestation IMAGE_REFERENCE BUILD_ATTESTATION
```

The attestation binds the source/Cargo lock, its own executable SHA-256, the
declared immutable image reference, and exact in-container Rust, Cargo, CUDA
compiler, CMake, Ninja, Clang, linker, and libc versions. It is created in a
fresh external path mounted into the no-GPU container and copied beside the
binary.

The binary is then run directly on the cn4 host:

```text
glmaxx cn4-environment-capture \
  SOURCE_ROOT \
  CUTLASS_ROOT \
  IMAGE_REFERENCE \
  BUILD_ATTESTATION \
  EVIDENCE_DIRECTORY
```

The command is Linux/x86-64-only. It requires:

1. an absolute canonical GLMAXX Git worktree;
2. an absolute canonical CUTLASS Git worktree;
3. an image reference containing an exact `sha256:` digest;
4. an absolute regular build-attestation file outside the repository;
5. an absolute external evidence path that does not exist; and
6. effective UID equal to the invoking non-root operator.

It fails closed if source, CUTLASS, build-attestation, or evidence paths
overlap; if an input path traverses a symbolic link; if the evidence parent is
not an existing operator-owned directory; or if source or the proposed
evidence leaf resolves to a mount root.

The Rust process owns subprocess creation, bounds, parsing, normalization,
hashing, and final evidence publication. It never invokes a shell. A later
operator wrapper may copy the binary and its expected digest, but it cannot
add capture commands or alter the acceptance rules.

## Source identity

The source identity contains:

- canonical repository origin URL;
- exact 40-hex `HEAD`;
- tree object ID;
- branch or detached state;
- tracked/index/worktree status;
- the complete untracked-name list;
- `Cargo.lock`, `AGENTS.md`, both specifications, container Dockerfile, and
  capture source SHA-256 values;
- initialized submodule identities, with none recorded explicitly; and
- the running executable's canonical path, byte length, mode, owner, device,
  inode, mtime, and SHA-256.

No tracked or index mutation is allowed. The only permitted untracked
subtree is the operator-owned `docs/reviews/` inbox. Every permitted untracked
name is recorded; it is never hashed or read by the capture.

The accepted Git origin is exactly:

```text
https://github.com/yatesdr/glmaxx.git
```

URL spellings carrying credentials, query strings, alternate remotes, or
implicit SSH user state are rejected rather than normalized.

CUTLASS must be clean, detached or branch-pinned at:

```text
e05f953a5b3d38adc240df2ff928e0421c2abba3
```

No `git fetch`, `pull`, submodule update, checkout, or other mutation occurs.

## Child-process sandbox

Every child command:

- is selected from a compiled allowlist;
- uses a canonical absolute executable whose SHA-256 is recorded;
- receives an explicit argument vector with no shell expansion;
- has stdin closed;
- starts with an empty environment plus only `LC_ALL=C`, `LANG=C`, `TZ=UTC`,
  and the minimum command-specific variables;
- has a ten-second deadline unless this contract names a smaller bound;
- has separate 4 MiB stdout and stderr caps;
- records start/end monotonic times, exit status, byte counts, and stream
  SHA-256 values; and
- is killed and reaped on timeout or output overflow.

Missing required commands, signals, nonzero exits, invalid UTF-8 where
structured text is required, truncation, duplicated rows, unsupported
required fields, and parse warnings are typed failures. Optional host
compiler diagnostics and optional GPU fields may be `not-installed` or
`not-supported` only where named below.

The implementation must mutation-test the allowlist. No argument prefix or
free-form operator argument may introduce a modifying subcommand.

## Exact command allowlist

The host capture may execute only read-only forms of:

| Authority | Required observation |
|---|---|
| `git` | source/CUTLASS root, origin, HEAD/tree, status, submodules |
| `uname` | kernel release, machine, build identity |
| `hostname` | static hostname only |
| `ldd` | copied GLMAXX executable linkage |
| `nvidia-smi` | version, explicit GPU fields, compute apps, topology |
| `lscpu` | JSON CPU/NUMA inventory |
| `lsblk` | JSON device/size/model/transport inventory without serials |
| `findmnt` | source and evidence-parent filesystem identity only |
| `docker image inspect --format` | allowlisted image identity fields only |

The build-attestation command, inside the pinned no-GPU container, may also
execute only the version-reporting forms of `rustc`, `cargo`, `nvcc`,
`cmake`, `ninja`, `clang`, `clang++`, the linker, and libc tooling. The host
capture may probe those same absolute names only as optional diagnostics;
their absence does not stand in for missing attested container versions.

The implementation may read these bounded virtual files directly:

```text
/etc/os-release
/proc/sys/kernel/random/boot_id
/proc/meminfo
/proc/driver/nvidia/version
/sys/bus/pci/devices/<validated GPU BDF>/current_link_speed
/sys/bus/pci/devices/<validated GPU BDF>/current_link_width
/sys/bus/pci/devices/<validated GPU BDF>/max_link_speed
/sys/bus/pci/devices/<validated GPU BDF>/max_link_width
/sys/bus/pci/devices/<validated GPU BDF>/numa_node
```

Every PCI sysfs child is derived only after parsing a canonical GPU PCI bus
ID and must remain beneath that exact device directory.

Forbidden commands/data include:

- `sudo`, package managers, network clients, and Git network operations;
- any NVIDIA-SMI device-modification/reset/clock/power/ECC/MIG option;
- `docker run`, start, stop, exec, rm, prune, pull, build, or full inspect;
- environment dumps, shell startup files, credential stores, SSH material,
  Docker auth, `/proc/*/environ`, or `/proc/*/cmdline`;
- generic `ps ... args`, process command lines, or open-file enumeration;
- NVMe serial numbers, filesystem contents, model paths, checkpoint names,
  and raw cache/evidence payloads; and
- arbitrary operator-selected files or commands.

## Container identity

`IMAGE_REFERENCE` must name the intended development image by immutable
digest. The only Docker inspection template may return:

- local content-addressable image ID;
- repository digests;
- OS, architecture, and variant;
- ordered rootfs diff IDs; and
- image creation timestamp.

Configuration environment, history commands, labels, mounts, secrets,
container inspection, and daemon-wide state are not captured.

The record binds the local image ID to the expected CUDA and Rust base-image
digests in `docs/cn4-container.md`. A tag alone, an empty repository-digest
identity where the operator claimed a registry digest, an architecture other
than `linux/amd64`, or an image-ID/reference mismatch is fatal. A locally
built image may have no repository digest only when `IMAGE_REFERENCE` is its
exact local content-addressable image ID; its ordered rootfs diff IDs,
Dockerfile SHA-256, and base-image digests remain mandatory.

The capture does not start the image. A later CPU-reproduction run must cite
this identity and create its own evidence record.

## Host and toolchain identity

Required normalized fields are:

- static hostname exactly `cn4`;
- boot ID, kernel release/build, architecture, and OS release ID/version;
- online/logical CPU count, model, sockets, cores, threads, NUMA nodes, and
  NUMA CPU lists;
- total/available host memory and huge-page state;
- block device major/minor, byte size, rotational flag, model, transport,
  and mount relation, excluding serials;
- source and evidence-parent mount source, mount ID where available,
  filesystem type, and read/write option;
- exact in-container Rust, Cargo, CUDA compiler, CMake, Ninja, Clang, linker,
  and libc versions from the bound build attestation;
- presence and version of host-side tools when installed, recorded as
  diagnostic rather than silently substituted for container tools; and
- SHA-256 of every resolved tool executable.

The pinned reproduction target is Rust 1.92.0, CUDA compiler 13.3.33,
CUTLASS at the revision above, and the image digest declared by the checked
in container contract. A missing host compiler is permitted. A missing or
mismatched attested container toolchain produces a valid inventory record
with a typed `TOOLCHAIN_MISMATCH` verdict, not a false reproduction pass.

## GPU identity

The core NVIDIA-SMI query records, in PCI-bus order:

- natural index, product name, architecture, UUID, and PCI bus/device ID;
- compute capability, driver version, and VBIOS version;
- GSP firmware version when the target driver exposes it;
- total/free/used memory;
- persistence, compute, ECC, and MIG modes;
- current and maximum SM/memory clocks;
- current power draw, enforced power limit, and temperature; and
- reported current/maximum PCIe generation and width when exposed.

The exact `nvidia-smi --version` and `--help-query-gpu` outputs are hashed
because NVIDIA documents that textual NVSMI output is not backward stable.
Required core fields must parse; optional GSP/ECC/MIG/link fields may record
`not-supported`, never an invented default.

Sysfs link speed, width, and NUMA node are bound to each validated PCI BDF and
cross-checked with NVIDIA-SMI. Idle power management may lower current link
generation; only the observation is recorded. A later benchmark must record
the live link state at its own start and finish rather than reusing this
snapshot.

An SM120 target identity requires exactly four distinct devices with:

```text
product name = NVIDIA RTX PRO 6000 Blackwell Workstation Edition
compute capability = 12.0
unique UUID
unique PCI bus ID
memory total within one reported MiB across ranks
```

Device index is never a durable identity. Every later rank map uses the
captured UUID and PCI bus ID.

## Topology identity

The capture retains the complete bounded output of `nvidia-smi topo -m` and a
normalized symmetric four-by-four relation matrix keyed by UUID.

It additionally records GPU CPU affinity, memory affinity, NUMA node, and
the sysfs parent-path digest for each device. The normalized topology:

- rejects missing/asymmetric/unknown GPU relations;
- distinguishes `PIX`, `PXB`, `PHB`, `NODE`, `SYS`, NVLink relations, and
  self;
- records rather than assumes the two-switch-pair shape;
- explicitly records whether any NVLink relation exists; and
- produces a topology SHA-256 used by later TP/DCP route evidence.

A changed motherboard or PCIe placement is a new topology identity, not a
failed attempt to masquerade as the prior Gen3 layout. No route or
performance conclusion is made from topology labels alone.

## Occupancy identity

Compute occupancy is sampled twice: immediately before all other host
queries and immediately after them. Each sample contains only:

```text
GPU UUID
PID
process name
used GPU memory
```

No process command line, environment, owner home, or open file is inspected.
Rows are sorted by UUID, PID, and name. The capture also retains per-GPU
used memory and utilization at both points.

Verdicts are:

- `INVENTORY_PASS_IDLE`: both samples contain no compute applications and no
  GPU reports more than 256 MiB used;
- `INVENTORY_PASS_OCCUPIED`: either sample contains a compute application or
  any GPU reports more than 256 MiB used, with all environment identities
  otherwise valid;
- `INVENTORY_UNSTABLE`: the application set, GPU visibility, UUID/BDF
  mapping, driver, VBIOS/GSP, or topology changes during capture;
- `TARGET_MISMATCH`: the exact four-device SM120 identity fails;
- `SOURCE_MISMATCH`: source origin/cleanliness, executable/build-attestation
  binding, or source commit/tree/Cargo lock identity fails;
- `TOOLCHAIN_MISMATCH`: target hardware is valid but a pinned tool, image, or
  CUTLASS requirement fails; or
- a typed capture/parse/safety failure.

The final classification precedence is capture/safety failure,
`INVENTORY_UNSTABLE`, `TARGET_MISMATCH`, `SOURCE_MISMATCH`,
`TOOLCHAIN_MISMATCH`, `INVENTORY_PASS_OCCUPIED`, then
`INVENTORY_PASS_IDLE`. Every applicable subordinate finding remains in the
record even when a higher-priority verdict wins.

Only `INVENTORY_PASS_IDLE` can satisfy the occupancy prerequisite for a
separately authorized device action, and even it grants no authorization.
`INVENTORY_PASS_OCCUPIED` is a successful read-only inventory, not a failed
capture and never permission to stop the observed processes.

## Evidence transaction

`EVIDENCE_DIRECTORY` is created with mode 0700 under an existing external
parent. Each raw child is created once with no replacement. The command uses
one unpredictable run ID only for filenames and one content-derived evidence
UUID for identity.

Publication order is:

1. validate arguments, ownership, paths, source, and executable;
2. create and lock the unique evidence directory;
3. record the first occupancy sample;
4. run every allowlisted query;
5. record the second occupancy sample;
6. parse and cross-check all required fields;
7. hash every raw record;
8. write canonical `environment.json.tmp`;
9. fdatasync each raw record and the temporary manifest;
10. rename without replacement to `environment.json`;
11. fsync the evidence directory; and
12. print only the final path, manifest SHA-256, evidence UUID, and verdict.

Failure before final publication writes a bounded `failure.json` when safe.
It never writes `environment.json`, never deletes another run, and never
reports a pass after an incomplete sync.

The canonical schema is:

```text
glmaxx.cn4-environment.v1
```

It includes:

- source, CUTLASS, executable, build-attestation, container, host, toolchain,
  GPU, topology, filesystem, and occupancy identities;
- exact child-command executable hashes, argument arrays, exits, timings,
  byte counts, and stdout/stderr hashes;
- the ordered raw-record hash table;
- every typed mismatch/warning;
- content-derived evidence UUID and manifest SHA-256;
- UTC timestamps and monotonic duration; and
- explicit nonclaims.

Raw records remain external and out of Git. A small repository summary may
cite the immutable manifest and raw-record hashes after independent review.

## CPU proof and later execution sequence

After design acceptance:

1. implement a pure parser/normalizer over checked-in synthetic fixtures;
2. mutation-test every missing, duplicate, malformed, reordered, unsupported,
   and cross-source mismatch;
3. prove the command allowlist cannot express a modifying operation;
4. obtain an adversarial CPU implementation review;
5. build the default-feature binary and emit its toolchain/build attestation
   in the pinned no-GPU container;
6. under a renewed safe inventory window, copy and hash the binary and
   attestation on cn4;
7. run only this read-only capture;
8. independently review the external evidence;
9. run the complete local CPU suite in a separately authorized no-GPU
   container bound to the captured source/image/toolchain; and
10. only after an idle recheck and a new device authorization begin kernel
    qualification.

Acceptance of this design, its CPU parser, or an occupied inventory does not
pass H01/D02, authorize cn4 execution, create a CUDA context, qualify a
kernel, validate a checkpoint, or establish capacity or performance.

## Primary interface references

- NVIDIA documents selective GPU and compute-process queries, topology, and
  the distinction between query and modifying options:
  <https://docs.nvidia.com/deploy/nvidia-smi/index.html>
- Docker documents allowlisted formatted image inspection:
  <https://docs.docker.com/reference/cli/docker/image/inspect/>
- Linux documents the PCI sysfs device-resource boundary:
  <https://docs.kernel.org/PCI/sysfs-pci.html>
