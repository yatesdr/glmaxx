# cn4 development container

The SM120 qualification environment is assembled from immutable base-image
digests:

- CUDA 13.3:
  `nvidia/cuda@sha256:ef2203909e80b8b976cfc672f7e2ae2b00bc0e25c404ee86d89e10a3802f1c52`
- Rust 1.92:
  `rust@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2`
- Nsight Systems:
  `nsight-systems-2026.1.3=2026.1.3.425-261338342291v0`

The full Nsight Systems package is intentional. CUDA's target-side collector
can retain a `.qdstrm` without containing the host-side `QdstrmImporter`,
which makes a seemingly successful capture unusable. The image therefore
pins both the collector and importer from the same package.

Build it from the repository root:

```bash
docker build \
  --file containers/cn4-dev.Dockerfile \
  --tag glmaxx-dev:cuda13.3-rust1.92 \
  containers
```

Record the local image identity in every evidence directory:

```bash
docker image inspect glmaxx-dev:cuda13.3-rust1.92 \
  --format '{{.Id}}'
nsys --version
test -x /opt/nvidia/nsight-systems/2026.1.3/host-linux-x64/QdstrmImporter
```

The repository, pinned CUTLASS checkout, Cargo cache, and external evidence
root are mounted at runtime. Raw test output and build products remain under
`~/glmaxx/` on cn4 and never enter Git.

GPU device access remains separately gated. Building this image, fetching
Rust dependencies, running CPU tests, compiling `sm_120f`, and running the
host-only CUTLASS layout probe do not authorize a CUDA device launch.

The preparation-only evidence gate is:

```bash
export CUTLASS_DIR=/cutlass
export GLMAXX_EVIDENCE_DIR=/evidence/prepare-<UTC timestamp>
export GLMAXX_CONTAINER_DIGEST=sha256:<local image identity>
./scripts/cn4-phase-b-prepare.sh
```

Run it in the pinned container with the repository mounted read-only at
`/workspace`, CUTLASS read-only at `/cutlass`, and an external evidence root
at `/evidence`. The script may record `nvidia-smi` inventory when devices are
visible, but it never launches a CUDA kernel. It deliberately stops at the
independent-review boundary enforced by `cn4-phase-b.sh`.
