# Persistent SM120 TP4 rank runtime

Date: 2026-07-29

Status: native binding qualified on cn4; checkpoint residency command
prepared but not run; kernel execution and collectives remain pending

## Boundary

The serving coordinator emits one immutable `StepPlan` and one collective
schedule. `Tp4WorkerPool` owns exactly four persistent rank threads. Each
thread owns one mutable `RankExecutor`; plan and schedule verification occurs
before its backend method is called. The executor returns a bounded
`StepOutput` containing the post-collective token decisions in sequence-table
order. Each sequence record distinguishes accepted draft tokens from an
optional final target residual/bonus token; this represents the draft-EOS case
without inventing a target token. All four logical records must match exactly.
Mode geometry, MTP commit provenance, and the GLM-5.2 tokenizer vocabulary
ceiling are validated before the coordinator sees the result. Serving then
enforces that EOS is final and terminates the request at that collective-safe
boundary.

The CUDA rank executor will lazily create its device state on that rank thread
and retain it until the worker generation terminates:

```text
rank 0 thread -> visible CUDA device 0
rank 1 thread -> visible CUDA device 1
rank 2 thread -> visible CUDA device 2
rank 3 thread -> visible CUDA device 3
```

No rank-local device selection or fallback is allowed. Startup fails unless
exactly four devices are visible and every selected device reports compute
capability 12.0.

## Native binding ABI

The thin native library exports:

```text
glmaxx_device_count(count)
glmaxx_device_bind(device_index,
                   compute_capability,
                   multiprocessor_count,
                   total_memory_bytes,
                   device_uuid[16])
glmaxx_device_memory_info(free_memory_bytes,
                          total_memory_bytes)
```

The first function wraps `cudaGetDeviceCount`. The second calls
`cudaSetDevice`, reads `cudaDeviceProp`, and returns only integer properties
plus the 16-byte CUDA device UUID needed by the Rust startup gate. It does not
create a stream, allocate memory, enable peer access, load weights, initialize
collectives, or launch a kernel. The third is owner-thread-only and wraps
`cudaMemGetInfo`; immediately before allocation, each persistent rank rejects
the load unless its live post-context free bytes cover that rank's validated,
hash-bound `SystemMemoryPlan`.

Rust hashes the observed identity using SHA-256 over:

```text
"glmaxx.cuda-device-identity.v1\0"
visible_device_count:u32le
visible_device_index:u32le
compute_capability:u32le
multiprocessor_count:u32le
total_memory_bytes:u64le
cuda_device_uuid:[u8;16]
```

The UUID and every numeric field must be nonzero where the topology contract
requires it. Before checkpoint allocation, the resulting digest must equal
the selected rank entry's `device_identity_sha256`. Copying the planned
digest into load evidence without this observed-device comparison is
forbidden.

Rust then creates one nonblocking stream on the bound rank thread. Its
`NativeRankContext` records the creating thread ID and rejects stream access
or synchronization from another thread. Destruction occurs when the
persistent rank loop exits.

## Failure behavior

Any of these conditions rejects startup or the current step:

- visible device count differs from four;
- rank is outside 0–3;
- `cudaSetDevice` or property discovery fails;
- compute capability differs from 12.0;
- SM count or total memory is zero;
- stream creation returns an error or null handle;
- the context is used from a different host thread; or
- any rank backend returns an execution error;
- a rank returns an output record inconsistent with the immutable plan; or
- token records or their canonical digest differ between ranks.

A plan-order, rank-backend, or output-consensus failure permanently closes the
worker generation. It may not process a later collective schedule.

## First cn4 proof

The first proof creates no device kernel. On an idle authorized cn4 it:

1. records `nvidia-smi` identity and topology;
2. independently requires four compute-capability 12.0 devices;
3. compiles the native library and CUDA-linked Rust binary;
4. starts four host threads;
5. binds ranks 0–3, one per visible device;
6. creates and synchronizes one nonblocking stream per rank;
7. destroys all streams on their owner threads; and
8. proves no compute process remains.

This opens only the rank-runtime startup boundary. It does not qualify weight
loading, kernels, collectives, graphs, checkpoint execution, or performance.

The first successful proof is pinned in
`docs/cn4-rank-bind-result-20260729.md`.
