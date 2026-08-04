#!/usr/bin/env bash
set -euo pipefail

readonly expected_authorization="active-glmaxx-goal-20260803"
readonly expected_review_token="exl3-source-projection-v1-accepted"
readonly expected_review_artifact="fable-exl3-source-projection-v1-r2.md"
readonly expected_review_commit="0edfc8d796aeaeb969668005149bcb6286aa1e85"
readonly expected_cutlass="e05f953a5b3d38adc240df2ff928e0421c2abba3"
readonly expected_container="sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719"
readonly expected_shard_name="model-layer-003.safetensors"
readonly expected_shard_sha256="31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing GPU work: the active-goal authorization marker is required" >&2
  exit 64
fi
if [[ -z "${CUTLASS_DIR:-}" || -z "${GLMAXX_BUILD_DIR:-}" ||
      -z "${GLMAXX_EVIDENCE_DIR:-}" ||
      -z "${GLMAXX_TR3_SHARD:-}" || -z "${GLMAXX_CONTAINER_DIGEST:-}" ]]; then
  echo "CUTLASS_DIR, GLMAXX_BUILD_DIR, GLMAXX_EVIDENCE_DIR, GLMAXX_TR3_SHARD, and GLMAXX_CONTAINER_DIGEST are required" >&2
  exit 64
fi
if [[ "${GLMAXX_CONTAINER_DIGEST}" != "${expected_container}" ]]; then
  echo "Container identity mismatch" >&2
  exit 65
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must be outside the Git repository" >&2
    exit 64
    ;;
esac
case "${GLMAXX_BUILD_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Build directory must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ "${GLMAXX_BUILD_DIR}" == "${GLMAXX_EVIDENCE_DIR}" ||
      -e "${GLMAXX_EVIDENCE_DIR}" || -e "${GLMAXX_BUILD_DIR}" ]]; then
  echo "Distinct build and evidence directories must not exist" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source worktree must be committed and clean" >&2
  exit 65
fi
if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "${expected_cutlass}" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi

review_artifact="${repo_dir}/${expected_review_artifact}"
if [[ ! -f "${review_artifact}" ]] ||
   ! git ls-files --error-unmatch "${expected_review_artifact}" >/dev/null 2>&1 ||
   ! grep -Fxq "${expected_review_token}" "${review_artifact}"; then
  echo "Tracked scalar-v1 acceptance artifact is missing" >&2
  exit 65
fi

require_hash() {
  local expected_sha="$1"
  local input_file="$2"
  local actual_sha
  actual_sha="$(sha256sum "${input_file}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "Pinned input mismatch: ${input_file}" >&2
    exit 70
  fi
}
require_hash \
  "7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3" \
  docs/exl3-trellis-cpu-contract.md
require_hash \
  "20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389" \
  docs/exl3-sm120-source-projection.md
require_hash \
  "241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d" \
  kernels/sm120/exl3_projection_control.cu
require_hash \
  "f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb" \
  crates/glm-format/src/exl3.rs
require_hash \
  "2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31" \
  crates/glm-cuda/src/ffi.rs
require_hash \
  "39be37583a28701eac8cde5c3df52b25397faf6c3f47f0333cc284758d456ff7" \
  crates/glm-cli/src/bin/exl3_real_k3_v1.rs
require_hash \
  "6ab8d2dbe3033e0944c8bd26b3717d5d0fdf7431f52bf55d00d641bbd0984106" \
  crates/glm-cli/Cargo.toml
reviewed_oracle_sha256="$(git show "${expected_review_commit}:crates/glm-format/src/exl3.rs" | sha256sum | awk '{print $1}')"
if [[ "${reviewed_oracle_sha256}" != "c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647" ]]; then
  echo "Reviewed scalar oracle is unavailable or changed" >&2
  exit 70
fi
oracle_delta_sha256="$(git diff "${expected_review_commit}" HEAD -- crates/glm-format/src/exl3.rs | sha256sum | awk '{print $1}')"
if [[ "${oracle_delta_sha256}" != "84a28dad0bc626f494251a08df4e26d8c485134fd06df868d74c043004a086ed" ]]; then
  echo "Post-review scalar-oracle delta is not the pinned warp-proof-only addition" >&2
  exit 70
fi

shard="$(realpath "${GLMAXX_TR3_SHARD}")"
if [[ ! -f "${shard}" || "$(basename "${shard}")" != "${expected_shard_name}" ]]; then
  echo "GLMAXX_TR3_SHARD must name the pinned layer-003 safetensors file" >&2
  exit 65
fi
shard_sha256="$(sha256sum "${shard}" | awk '{print $1}')"
if [[ "${shard_sha256}" != "${expected_shard_sha256}" ]]; then
  echo "TR3 layer-003 shard identity mismatch" >&2
  exit 70
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}" "${GLMAXX_BUILD_DIR}"
readonly source_commit="$(git rev-parse HEAD)"
readonly review_sha256="$(sha256sum "${review_artifact}" | awk '{print $1}')"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    echo "cn4 is occupied; no new GPU work was launched" >&2
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total,memory.used,utilization.gpu \
  --format=csv,noheader | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory-before.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader | tr -d '\r ' | grep -c '^12\.0$' || true)"
printf 'visible_devices=%s sm120_devices=%s\n' "${gpu_count}" "${sm120_count}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-counts.txt"
if [[ "${gpu_count}" != 4 || "${sm120_count}" != 4 ]]; then
  echo "Qualification requires exactly four visible SM120 GPUs" >&2
  exit 70
fi
check_idle

printf '%s\n' "${source_commit}" > "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch > "${GLMAXX_EVIDENCE_DIR}/source-status-before.txt"
git diff --binary HEAD > "${GLMAXX_EVIDENCE_DIR}/source-diff.patch"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" > "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s\n' "${GLMAXX_BUILD_DIR}" > "${GLMAXX_EVIDENCE_DIR}/build-dir.txt"
printf '%s  %s\n' "${shard_sha256}" "${shard}" > "${GLMAXX_EVIDENCE_DIR}/checkpoint-shard-sha256.txt"
printf '%s  %s\n' "${review_sha256}" "${expected_review_artifact}" > "${GLMAXX_EVIDENCE_DIR}/review-artifact-sha256.txt"
printf '%s\n' "${reviewed_oracle_sha256}" > "${GLMAXX_EVIDENCE_DIR}/reviewed-oracle-sha256.txt"
printf '%s\n' "${oracle_delta_sha256}" > "${GLMAXX_EVIDENCE_DIR}/oracle-delta-sha256.txt"
rustc --version --verbose > "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose > "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version > "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD > "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
sha256sum Cargo.lock crates/glm-cli/Cargo.toml \
  crates/glm-cli/src/bin/exl3_real_k3_v1.rs crates/glm-format/src/exl3.rs \
  crates/glm-cuda/src/abi.rs crates/glm-cuda/src/ffi.rs crates/glm-cuda/src/lib.rs \
  kernels/include/glmaxx_kernel.h kernels/sm120/exl3_projection_control.cu \
  scripts/cn4-exl3-real-k3-v1.sh "${review_artifact}" \
  > "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_BUILD_DIR}/cargo-target"
cargo test --workspace --offline 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"
kernel_build="${GLMAXX_BUILD_DIR}/kernel"
cmake -S kernels -B "${kernel_build}" -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${kernel_build}" --target glmaxx_sm120 --verbose 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"
cuobjdump --list-elf "${kernel_build}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"
if ! grep -Fq 'exl3_projection_control.sm_120.cubin' "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"; then
  echo "Scalar EXL3 SM120 cubin is missing" >&2
  exit 70
fi
export GLMAXX_KERNEL_LIB_DIR="${kernel_build}"
export LD_LIBRARY_PATH="${kernel_build}:${LD_LIBRARY_PATH:-}"
cargo test --offline -p glm-cli --features cuda-ffi --bin glmaxx-exl3-real-k3-v1 \
  2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cargo-harness-test.txt"
cargo build --release --offline -p glm-cli --features cuda-ffi \
  --bin glmaxx-exl3-real-k3-v1 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-build.txt"
runner="${CARGO_TARGET_DIR}/release/glmaxx-exl3-real-k3-v1"
sha256sum "${runner}" "${kernel_build}/libglmaxx_sm120.so" \
  > "${GLMAXX_EVIDENCE_DIR}/build-artifact-sha256.txt"

check_idle
if CUDA_VISIBLE_DEVICES=0 "${runner}" "${shard}" "${shard_sha256}" 3 6 0 gate \
    > "${GLMAXX_EVIDENCE_DIR}/k4-negative-stdout.txt" \
    2> "${GLMAXX_EVIDENCE_DIR}/k4-negative-stderr.txt"; then
  echo "K=4 negative control unexpectedly passed the K=3-only qualifier" >&2
  exit 70
fi
if ! grep -Fq 'trellis' "${GLMAXX_EVIDENCE_DIR}/k4-negative-stderr.txt"; then
  echo "K=4 negative control did not fail at its trellis contract" >&2
  exit 70
fi

for rank in 0 3; do
  for projection in gate up down; do
    check_idle
    result="${GLMAXX_EVIDENCE_DIR}/layer3-expert0-rank${rank}-${projection}.json"
    CUDA_VISIBLE_DEVICES=0 "${runner}" "${shard}" "${shard_sha256}" \
      3 0 "${rank}" "${projection}" | tee "${result}"
    grep -Fq '"verdict": "passed"' "${result}"
    grep -Fq '"failed_elements": 0' "${result}"
    grep -Fq '"repeat_bitwise_deterministic": true' "${result}"
    if [[ "$(grep -Fc '"rows":' "${result}")" != 4 ]]; then
      echo "Expected all four M=1/2/4/8 result cases" >&2
      exit 70
    fi
  done
done

check_idle
nvidia-smi --query-gpu=index,uuid,memory.used,utilization.gpu,clocks.current.sm,clocks.current.memory,power.draw \
  --format=csv,noheader > "${GLMAXX_EVIDENCE_DIR}/gpu-state-after.csv"
git status --short --branch > "${GLMAXX_EVIDENCE_DIR}/source-status-after.txt"
if [[ "$(git rev-parse HEAD)" != "${source_commit}" || -n "$(git status --porcelain)" ||
      "$(sha256sum "${review_artifact}" | awk '{print $1}')" != "${review_sha256}" ]]; then
  echo "Source or review provenance changed during qualification" >&2
  exit 70
fi

printf '{"schema":"glmaxx.sm120-exl3-real-k3-run.v1","verdict":"passed","source_commit":"%s","checkpoint_shard_sha256":"%s","gpu_device":"physical_gpu_0","projection_reports":6,"shape_cases":24,"k4_negative_control":"passed_fail_closed","performance_status":"scalar_control_only"}\n' \
  "${source_commit}" "${shard_sha256}" > "${GLMAXX_EVIDENCE_DIR}/summary.json"
printf '%s\n' \
  "EXL3_REAL_TR3_K3_SCALAR_V1_PASSED" \
  "Six real K=3 projections across ranks 0 and 3 passed M=1/2/4/8 correctness and deterministic replay." \
  "K=4 remains fail-closed; scalar timings are controls, not optimized-route acceptance." \
  > "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
(
  cd "${GLMAXX_EVIDENCE_DIR}"
  find . -maxdepth 1 -type f ! -name 'artifact-manifest.txt' ! -name 'artifact-manifest.sha256' \
    -print0 | sort -z | xargs -0 sha256sum > artifact-manifest.txt
  sha256sum artifact-manifest.txt > artifact-manifest.sha256
)
cat "${GLMAXX_EVIDENCE_DIR}/summary.json"
cat "${GLMAXX_EVIDENCE_DIR}/artifact-manifest.sha256"
