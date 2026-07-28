# Hardware and lab plan

## Target machine snapshot

Read-only inventory on 2026-07-28:

| Component | Current cn4 state |
|---|---|
| GPUs | 4× RTX PRO 6000 Blackwell, 96 GB each, SM120 |
| GPU memory | 384 GB aggregate |
| CPU | Xeon W-2195, 18 cores / 36 threads |
| Host RAM | 125 GiB visible |
| Motherboard | ASUS WS C422 SAGE/10G |
| GPU links | PCIe Gen3 x16, two switch pairs |
| OS NVMe | 2 TB Samsung, about 784 GiB free |
| Secondary NVMe | 1.6 TB Intel, about 1.3 TiB free |
| Host quant toolchain | not installed |

This is sufficient for inference, kernel development, and small conversion
tests. It is not a comfortable full GLM-5.2 BF16 checkpoint factory.

The free-space figures are not permission to co-locate the BF16 source,
EXL3 control, an NVFP4 candidate, conversion scratch, and a large persistent
KV cache. Before any full conversion or transfer, produce a placement ledger
that reserves:

- immutable source/checkpoint bytes;
- one candidate's exact rank-file bytes;
- temporary transfer and verification bytes;
- configured NVMe KV-tier capacity;
- index/journal headroom and a filesystem free-space floor.

Convert away from cn4 when practical and stage only one fit-capable candidate
at a time. Cold loading and cryptographically verifying hundreds of GiB is an
order-minutes operation on this storage; measure and report it separately
from engine initialization rather than hiding it in TTFT.

## Planned motherboard

The Supermicro M12SWA-TF supports:

- Threadripper PRO 3000WX/5000WX processors;
- six PCIe 4.0 x16 slots;
- eight-channel DDR4;
- up to 2 TB RDIMM;
- four PCIe 4.0 M.2 sockets and two U.2 sockets.

The current Xeon W-2195 is incompatible with this board. A compatible
Threadripper PRO CPU and DDR4 memory population are required.

The GPUs support PCIe Gen5, so this motherboard will operate them at Gen4.
That is still approximately twice the current per-lane signaling rate and
removes the C422 board's known Gen3 constraint. Topology and peer bandwidth
must be remeasured after installation rather than inferred from specifications.

## Recommended local build

For repeated full-model quantization:

- 512 GB ECC RAM: minimum sensible target;
- 1 TB ECC RAM: preferred;
- 32–64 CPU cores;
- at least 4 TB genuinely free fast NVMe scratch;
- 8 TB scratch if retaining source plus multiple candidates;
- immutable source, work, output, and evaluation directories on separate
  paths.

GLM-5.2 is roughly 753B parameters. Its BF16 repository is about 1.4 TiB, and
a development run may simultaneously need the source, a 200–500 GiB output,
offload state, calibration assets, temporary packing state, and caches.

## Rental policy

B300 is not required for post-training quantization. The published
LLM Compressor GLM-5.2 recipe was run on six A100s with disk offload.

Rent x86 A100/H100/H200 capacity with at least 512 GB host RAM and several
terabytes of local NVMe when a full conversion would otherwise stall local
iteration. Rent B300 only when the saved turnaround time justifies the cost,
for example:

- many full-checkpoint sensitivity sweeps;
- large Hessian/calibration searches;
- QAT or distillation;
- regeneration of a broad BF16 logit reference.

B300 results do not replace cn4 acceptance. Its HBM and NVLink can hide the
small-M and PCIe behavior under study, and its kernels are not SM120 kernels.

## Machine roles

| Work | Preferred machine |
|---|---|
| CPU packer/oracle development | local workstation |
| One-tensor and one-expert CUDA kernels | cn4 |
| SM120 microbenchmark/autotuning | cn4 |
| Full checkpoint conversion | upgraded cn4 or rented A100/H100/H200 node |
| BF16 reference generation | rental when local offload is too slow |
| Final quality/performance acceptance | cn4 |
