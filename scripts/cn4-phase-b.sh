#!/usr/bin/env bash
set -euo pipefail

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "phase-b-authorized" ]]; then
  echo "Refusing GPU access: set GLMAXX_CN4_AUTHORIZATION=phase-b-authorized only after explicit operator authorization" >&2
  exit 64
fi

if [[ "${GLMAXX_REVIEW_GATE:-}" != "manifest-abi-v0.2.2-accepted" ||
      -z "${GLMAXX_REVIEW_ARTIFACT:-}" ]]; then
  echo "Refusing M2: the review token and committed review artifact are required" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

if [[ -z "${CUTLASS_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" || \
      -z "${GLMAXX_CONTAINER_DIGEST:-}" ]]; then
  echo "CUTLASS_DIR, GLMAXX_EVIDENCE_DIR, and GLMAXX_CONTAINER_DIGEST are required" >&2
  exit 64
fi

if [[ ! "${GLMAXX_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "GLMAXX_CONTAINER_DIGEST must be a sha256:<64 lowercase hex> identity" >&2
  exit 64
fi

case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must be outside the Git repository" >&2
    exit 64
    ;;
esac

if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist; use a fresh immutable path" >&2
  exit 65
fi

if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi

source_status="$(git status --porcelain)"
if [[ -n "${source_status}" ]]; then
  echo "Source tree must be committed before qualification" >&2
  exit 65
fi

if [[ ! -f "${GLMAXX_REVIEW_ARTIFACT}" ]]; then
  echo "Review artifact does not exist or is not a regular file" >&2
  exit 65
fi
review_artifact="$(realpath "${GLMAXX_REVIEW_ARTIFACT}")"
case "${review_artifact}" in
  "${repo_dir}"/*) ;;
  *)
    echo "Review artifact must be inside the source repository" >&2
    exit 65
    ;;
esac
review_relative="${review_artifact#"${repo_dir}/"}"
if ! git ls-files --error-unmatch "${review_relative}" >/dev/null 2>&1; then
  echo "Review artifact must be tracked by Git" >&2
  exit 65
fi
if ! grep -Fxq "manifest-abi-v0.2.2-accepted" "${review_artifact}"; then
  echo "Review artifact does not contain the exact acceptance token" >&2
  exit 65
fi
review_sha_before="$(shasum -a 256 "${review_artifact}" | awk '{print $1}')"

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no GPU work was launched" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,driver_version,memory.total \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-before.csv"

check_idle

git rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
cmake --version | tee "${GLMAXX_EVIDENCE_DIR}/cmake.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s  %s\n' "${review_sha_before}" "${review_relative}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/review-artifact-sha256.txt"
shasum -a 256 spec/engine-v0.md spec/format-v0.md \
  manifests/glm52-operation-v1.json benchmarks/sm120-fc1-matrix-v1.json \
  fixtures/sm120-fc1-matrix-proof-v1.json "${review_artifact}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_EVIDENCE_DIR}/cargo-target"
cargo test --workspace --offline 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"

build_dir="${GLMAXX_EVIDENCE_DIR}/build"
cmake -S kernels -B "${build_dir}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${build_dir}" --verbose 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"

"${build_dir}/glmaxx_cutlass_layout_probe" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-layout-probe.txt"
"${build_dir}/glmaxx_cutlass_activation_layout_probe" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-activation-layout-probe.txt"
cuobjdump --dump-resource-usage "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-resources.txt"
cuobjdump --dump-sass "${build_dir}/libglmaxx_sm120.so" \
  > "${GLMAXX_EVIDENCE_DIR}/glmaxx-library-sass.txt"
owned_omma_count="$(
  grep -c 'OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X' \
    "${GLMAXX_EVIDENCE_DIR}/glmaxx-library-sass.txt" || true
)"
printf '%s\n' "${owned_omma_count}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/glmaxx-owned-omma-count.txt"
if [[ "${owned_omma_count}" != "64" ]]; then
  echo "GLMAXX-owned dense control did not retain exactly 64 expected SM120 NVFP4 OMMA instructions" >&2
  exit 70
fi
nm -D --defined-only "${build_dir}/libglmaxx_sm120.so" \
  | grep 'glmaxx_nvfp4_dense_control_launch' \
  | tee "${GLMAXX_EVIDENCE_DIR}/glmaxx-dense-control-symbol.txt"

check_idle

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"

check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  -- gpu-smoke 1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-smoke-m1.json"

correctness_dir="${GLMAXX_EVIDENCE_DIR}/correctness"
if [[ -e "${correctness_dir}" ]]; then
  echo "Correctness evidence directory already exists; refusing to overwrite it" >&2
  exit 65
fi
mkdir "${correctness_dir}"
check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  -- gpu-matrix "${correctness_dir}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-matrix-summary.json"

shasum -a 256 "${correctness_dir}"/*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/correctness-sha256.txt"

graph_dir="${GLMAXX_EVIDENCE_DIR}/graph-correctness"
if [[ -e "${graph_dir}" ]]; then
  echo "Graph evidence directory already exists; refusing to overwrite it" >&2
  exit 65
fi
mkdir "${graph_dir}"
check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  -- gpu-graph "${graph_dir}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-graph-summary.json"

shasum -a 256 "${graph_dir}"/*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/graph-correctness-sha256.txt"

dense_control_dir="${GLMAXX_EVIDENCE_DIR}/dense-control-correctness"
if [[ -e "${dense_control_dir}" ]]; then
  echo "Dense-control evidence directory already exists; refusing to overwrite it" >&2
  exit 65
fi
mkdir "${dense_control_dir}"
check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  -- gpu-dense-control "${dense_control_dir}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-dense-control-summary.json"

shasum -a 256 "${dense_control_dir}"/*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/dense-control-correctness-sha256.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-after.csv"

review_sha_after="$(shasum -a 256 "${review_artifact}" | awk '{print $1}')"
if [[ "${review_sha_after}" != "${review_sha_before}" ]]; then
  echo "Review artifact changed during qualification" >&2
  exit 70
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during qualification" >&2
  exit 70
fi

echo "Eager, CUDA-graph, and SM120 CUTLASS dense-control correctness gates finished. Do not benchmark unless the eager summary reports 135 positive cases, 9 negative rejections, 2 deterministic cases, and zero failures; the graph summary reports 2 bitwise-deterministic cases over 20 replays with zero failures; and the dense-control summary reports 2 bitwise-deterministic cases over 20 repeats with zero failures."
