#!/usr/bin/env bash
set -euo pipefail

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "phase-c-authorized" ]]; then
  echo "Refusing GPU timing: set GLMAXX_CN4_AUTHORIZATION=phase-c-authorized only after explicit operator authorization" >&2
  exit 64
fi

if [[ "${GLMAXX_REVIEW_GATE:-}" != "manifest-abi-v0.2.2-accepted" ||
      -z "${GLMAXX_REVIEW_ARTIFACT:-}" ]]; then
  echo "Refusing timing: the review token and committed review artifact are required" >&2
  exit 64
fi

if [[ -z "${GLMAXX_PHASE_B_EVIDENCE:-}" ||
      -z "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_PHASE_B_EVIDENCE and GLMAXX_EVIDENCE_DIR are required" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

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
if [[ ! -d "${GLMAXX_PHASE_B_EVIDENCE}" ]]; then
  echo "Phase-B evidence directory does not exist" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before timing" >&2
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
if ! git ls-files --error-unmatch "${review_relative}" >/dev/null 2>&1 ||
   ! grep -Fxq "manifest-abi-v0.2.2-accepted" "${review_artifact}"; then
  echo "Review artifact must be tracked and contain the exact acceptance token" >&2
  exit 65
fi

source_commit="$(git rev-parse HEAD)"
phase_b_commit="$(tr -d '[:space:]' < "${GLMAXX_PHASE_B_EVIDENCE}/source-commit.txt")"
if [[ "${phase_b_commit}" != "${source_commit}" ]]; then
  echo "Phase-B correctness was produced by a different source commit" >&2
  exit 65
fi

eager_summary="${GLMAXX_PHASE_B_EVIDENCE}/correctness/summary.json"
graph_summary="${GLMAXX_PHASE_B_EVIDENCE}/graph-correctness/summary.json"
for summary in "${eager_summary}" "${graph_summary}"; do
  if [[ ! -f "${summary}" ]]; then
    echo "Required Phase-B summary is missing: ${summary}" >&2
    exit 65
  fi
done
for expected in \
  '"positive_cases": 135' \
  '"negative_route_cases": 9' \
  '"failed_elements": 0' \
  '"eager_deterministic_cases": 2'; do
  if ! grep -Fq "${expected}" "${eager_summary}"; then
    echo "Phase-B eager summary does not satisfy: ${expected}" >&2
    exit 65
  fi
done
for expected in \
  '"graph_cases": 2' \
  '"graph_repeat_count": 20' \
  '"failed_elements": 0' \
  '"bitwise_deterministic_cases": 2'; do
  if ! grep -Fq "${expected}" "${graph_summary}"; then
    echo "Phase-B graph summary does not satisfy: ${expected}" >&2
    exit 65
  fi
done

kernel_dir="${GLMAXX_PHASE_B_EVIDENCE}/build"
runner="${GLMAXX_PHASE_B_EVIDENCE}/cargo-target/release/glmaxx"
if [[ ! -f "${kernel_dir}/libglmaxx_sm120.so" || ! -x "${runner}" ]]; then
  echo "Phase-B native library or Rust runner is missing" >&2
  exit 65
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no timing work was launched" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

check_idle
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.draw,power.limit,temperature.gpu \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-state-before.csv"
printf '%s\n' "${source_commit}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
shasum -a 256 \
  "${review_artifact}" \
  "${eager_summary}" \
  "${graph_summary}" \
  "${kernel_dir}/libglmaxx_sm120.so" \
  "${runner}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

benchmark_dir="${GLMAXX_EVIDENCE_DIR}/direct-baseline"
mkdir "${benchmark_dir}"
export LD_LIBRARY_PATH="${kernel_dir}:${LD_LIBRARY_PATH:-}"
check_idle
"${runner}" gpu-bench "${benchmark_dir}" 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-bench-summary.json"

shasum -a 256 "${benchmark_dir}"/*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/benchmark-sha256.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.draw,power.limit,temperature.gpu \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-state-after.csv"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during baseline timing" >&2
  exit 70
fi

printf '%s\n' \
  "PROVISIONAL_CONTROL_ONLY" \
  "The direct CUDA-core baseline was timed only after eager and graph correctness passed." \
  "No tensor-core performance claim is made by this result." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
