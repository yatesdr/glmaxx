# cn4 development container

The SM120 qualification environment is assembled from immutable base-image
digests:

- CUDA 13.3:
  `nvidia/cuda@sha256:ef2203909e80b8b976cfc672f7e2ae2b00bc0e25c404ee86d89e10a3802f1c52`
- Rust 1.92:
  `rust@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2`

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
```

The repository, pinned CUTLASS checkout, Cargo cache, and external evidence
root are mounted at runtime. Raw test output and build products remain under
`~/glmaxx/` on cn4 and never enter Git.

GPU device access remains separately gated. Building this image, fetching
Rust dependencies, running CPU tests, compiling `sm_120f`, and running the
host-only CUTLASS layout probe do not authorize a CUDA device launch.
