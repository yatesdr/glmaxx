#!/usr/bin/env bash
set -euo pipefail

# Supplemental correctness matrix for the already qualified EXL3 source
# projection v1 binary. This script does not rebuild or substitute artifacts:
# it requires the exact Phase-B executable and shared-library identities.

readonly expected_authorization="exl3-multirow-correctness-authorized"
readonly expected_source_commit="ccf0162e236e8a8b5d4d6a308d6491759750e83e"
readonly expected_binary_sha256="ad2fb57c7cb25588f3cea3bc9f421994f4c16e84eea9c42a530b3342dd14187f"
readonly expected_library_sha256="0d95723eb9eb3ed625d6f4933177006faa870eca9624dd3ee1a4fc200813d43d"
readonly expected_review_token="exl3-source-projection-v1-accepted"
readonly expected_review_relative="fable-exl3-source-projection-v1-r2.md"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing GPU access: set GLMAXX_CN4_AUTHORIZATION=${expected_authorization} only after explicit operator authorization" >&2
  exit 64
fi

if [[ -z "${GLMAXX_QUALIFICATION_WORKTREE:-}" ||
      -z "${GLMAXX_PHASE_B_EVIDENCE:-}" ||
      -z "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_QUALIFICATION_WORKTREE, GLMAXX_PHASE_B_EVIDENCE, and GLMAXX_EVIDENCE_DIR are required" >&2
  exit 64
fi

readonly source_dir="$(realpath "${GLMAXX_QUALIFICATION_WORKTREE}")"
readonly phase_b_dir="$(realpath "${GLMAXX_PHASE_B_EVIDENCE}")"
readonly binary="${phase_b_dir}/cargo-target/release/glmaxx"
readonly library="${phase_b_dir}/kernel-build/libglmaxx_sm120.so"
readonly review_artifact="${source_dir}/${expected_review_relative}"

case "${source_dir}" in
  /home/derek/glmaxx/worktrees/*) ;;
  *)
    echo "Qualification worktree must be isolated under /home/derek/glmaxx/worktrees" >&2
    exit 64
    ;;
esac
case "${phase_b_dir}" in
  /home/derek/glmaxx/evidence/*) ;;
  *)
    echo "Phase-B evidence must be isolated under /home/derek/glmaxx/evidence" >&2
    exit 64
    ;;
esac
case "${GLMAXX_EVIDENCE_DIR}" in
  /home/derek/glmaxx/evidence/*) ;;
  *)
    echo "New evidence must be isolated under /home/derek/glmaxx/evidence" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist" >&2
  exit 65
fi

if [[ "$(git -C "${source_dir}" rev-parse HEAD)" != "${expected_source_commit}" ||
      -n "$(git -C "${source_dir}" status --porcelain)" ]]; then
  echo "Qualification source identity is not the clean accepted candidate" >&2
  exit 65
fi
if [[ ! -f "${review_artifact}" ]] ||
   ! grep -Fxq "${expected_review_token}" "${review_artifact}"; then
  echo "Accepted EXL3 review artifact/token is missing" >&2
  exit 65
fi
if [[ "$(sha256sum "${binary}" | awk '{print $1}')" != "${expected_binary_sha256}" ||
      "$(sha256sum "${library}" | awk '{print $1}')" != "${expected_library_sha256}" ]]; then
  echo "Phase-B executable or shared library identity changed" >&2
  exit 65
fi

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no new GPU command was launched" >&2
    exit 75
  fi
}

gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$({ nvidia-smi --query-gpu=compute_cap --format=csv,noheader || true; } | tr -d '\r ' | grep -c '^12\.0$' || true)"
if [[ "${gpu_count}" != "4" || "${sm120_count}" != "4" ]]; then
  echo "Qualification requires exactly four visible SM120 GPUs" >&2
  exit 70
fi
check_idle

mkdir -p "${GLMAXX_EVIDENCE_DIR}"
nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total \
  --format=csv,noheader | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
git -C "${source_dir}" status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
printf '%s\n' "${expected_source_commit}" | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
sha256sum "${binary}" "${library}" "${review_artifact}" "$0" \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"
ldd "${binary}" | tee "${GLMAXX_EVIDENCE_DIR}/linkage.txt"

export LD_LIBRARY_PATH="$(dirname "${library}"):${LD_LIBRARY_PATH:-}"
for rows in 1 2 4 8; do
  for projection in gate up down; do
    check_idle
    output="${GLMAXX_EVIDENCE_DIR}/gpu-exl3-${projection}-m${rows}.json"
    "${binary}" gpu-exl3-smoke "${projection}" "${rows}" | tee "${output}"
    jq -e \
      --arg projection "${projection}" \
      --argjson rows "${rows}" \
      '.schema == "glmaxx.sm120-exl3-source-smoke.v1"
       and .projection == $projection
       and .shape[0] == $rows
       and .failed_elements == 0
       and .repeat_bitwise_deterministic == true
       and .cpu_output_sha256 == .gpu_output_sha256
       and .persistent_reconstructed_weight_bytes == 0
       and .runtime_weight_repack_bytes == 0' \
      "${output}" >/dev/null
  done
done

check_idle
sha256sum "${GLMAXX_EVIDENCE_DIR}"/gpu-exl3-*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/correctness-sha256.txt"
jq -s \
  --arg source_commit "${expected_source_commit}" \
  '{schema:"glmaxx.sm120-exl3-multirow-correctness.v1",
    source_commit:$source_commit,
    rows:([.[].shape[0]]|unique),
    projections:([.[].projection]|unique),
    cases:length,
    failed_elements:([.[].failed_elements]|add),
    all_repeat_bitwise_deterministic:all(.[];.repeat_bitwise_deterministic),
    all_cpu_gpu_hashes_equal:all(.[];.cpu_output_sha256 == .gpu_output_sha256),
    claim:"synthetic multi-row correctness only; not real payload, TP4, or performance"}' \
  "${GLMAXX_EVIDENCE_DIR}"/gpu-exl3-*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/summary.json"

if [[ "$(git -C "${source_dir}" rev-parse HEAD)" != "${expected_source_commit}" ||
      -n "$(git -C "${source_dir}" status --porcelain)" ]]; then
  echo "Qualification source changed during the run" >&2
  exit 70
fi

printf '%s\n' \
  "EXL3_SOURCE_PROJECTION_MULTIROW_CORRECTNESS_PASSED" \
  "Gate, up, and down source-order projections passed at M=1,2,4,8." \
  "This is synthetic correctness evidence, not a real-payload, TP4, or performance result." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
