ARG RUST_IMAGE=rust@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2
ARG CUDA_IMAGE=nvidia/cuda@sha256:ef2203909e80b8b976cfc672f7e2ae2b00bc0e25c404ee86d89e10a3802f1c52

FROM ${RUST_IMAGE} AS rust-toolchain

FROM ${CUDA_IMAGE}

ARG NSIGHT_SYSTEMS_PACKAGE_VERSION=2026.1.3.425-261338342291v0

LABEL org.opencontainers.image.title="glmaxx cn4 development toolchain"
LABEL org.opencontainers.image.description="Pinned Rust 1.92 and CUDA 13.3 toolchain for the SM120 qualification gates"

ENV NSIGHT_SYSTEMS_ROOT=/opt/nvidia/nsight-systems/2026.1.3 \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/opt/nvidia/nsight-systems/2026.1.3/bin:/usr/local/cargo/bin:${PATH}

COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup
COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        git \
        libdigest-sha-perl \
        ninja-build \
        nsight-systems-2026.1.3=${NSIGHT_SYSTEMS_PACKAGE_VERSION} \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

CMD ["/bin/bash"]
