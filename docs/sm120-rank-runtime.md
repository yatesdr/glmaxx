# Persistent SM120 TP4 rank runtime

Date: 2026-07-29

Status: design candidate; native binding implementation and cn4 evidence
pending

## Boundary

The serving coordinator emits one immutable `StepPlan` and one collective
schedule. `Tp4WorkerPool` owns exactly four persistent rank threads. Each
thread owns one mutable `RankExecutor`; plan and schedule verification occurs
before its backend method is called.

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
                   total_memory_bytes)
```

The first function wraps `cudaGetDeviceCount`. The second calls
`cudaSetDevice`, reads `cudaDeviceProp`, and returns only integer properties
needed by the Rust startup gate. It does not create a stream, allocate memory,
enable peer access, load weights, initialize collectives, or launch a kernel.

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
- any rank backend returns an execution error.

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
