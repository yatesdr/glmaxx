#!/usr/bin/env bash
set -euo pipefail

readonly expected_authorization="hybrid-source-hash-authorized"
readonly expected_model_dir="/home/claude/LLM/GLM-5.2-hybrid"
readonly expected_file_count="194"
readonly expected_file_bytes="366021385004"
readonly expected_shard_count="184"
readonly expected_shard_bytes="365987273208"
readonly expected_index_total_size="365968736768"
readonly expected_tensor_count="148289"
readonly expected_config_sha256="254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef"
readonly expected_index_sha256="6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234"
readonly expected_tier_sha256="ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing audit: set GLMAXX_CN4_AUTHORIZATION=${expected_authorization}" >&2
  exit 64
fi
if [[ -z "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_EVIDENCE_DIR is required" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

model_dir="$(realpath "${expected_model_dir}")"
if [[ "${model_dir}" != "${expected_model_dir}" ]]; then
  echo "Hybrid checkpoint path changed" >&2
  exit 65
fi
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*|"${model_dir}"|"${model_dir}"/*)
    echo "Evidence must remain outside the repository and checkpoint" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before the audit" >&2
  exit 65
fi
if [[ -e "${model_dir}/MANIFEST.sha256" ]]; then
  echo "Unexpected source manifest appeared; re-review the identity basis" >&2
  exit 65
fi
if find "${model_dir}" -maxdepth 1 \( -type l -o -type p -o -type s -o -type b -o -type c \) \
    -print -quit | grep -q .; then
  echo "Checkpoint root contains an unsupported non-regular entry" >&2
  exit 65
fi
if find "${model_dir}" -maxdepth 1 -type f -name '*.part' -print -quit | grep -q .; then
  echo "Checkpoint contains an incomplete download" >&2
  exit 75
fi
if find "${model_dir}" -maxdepth 1 -type f -printf '%f\n' \
    | grep -Ev '^[A-Za-z0-9._-]+$' | grep -q .; then
  echo "Checkpoint root contains a noncanonical filename" >&2
  exit 65
fi

hash_file() {
  sha256sum "$1" | awk '{print $1}'
}

if [[ "$(hash_file "${model_dir}/config.json")" != "${expected_config_sha256}" ||
      "$(hash_file "${model_dir}/model.safetensors.index.json")" != "${expected_index_sha256}" ||
      "$(hash_file "${model_dir}/mxfp8_tier_nokvb.json")" != "${expected_tier_sha256}" ]]; then
  echo "Hybrid runtime metadata identity mismatch" >&2
  exit 65
fi

read -r file_count file_bytes shard_count shard_bytes < <(
  find "${model_dir}" -maxdepth 1 -type f -printf '%f\t%s\n' \
    | awk '$1 ~ /\.safetensors$/ {sn += 1; sb += $2} {n += 1; b += $2}
           END {printf "%d %.0f %d %.0f\n", n, b, sn, sb}'
)
if [[ "${file_count}" != "${expected_file_count}" ||
      "${file_bytes}" != "${expected_file_bytes}" ||
      "${shard_count}" != "${expected_shard_count}" ||
      "${shard_bytes}" != "${expected_shard_bytes}" ]]; then
  echo "Hybrid checkpoint file inventory mismatch" >&2
  exit 65
fi

read -r index_total_size tensor_count index_shard_count < <(
  python3 -c '
import glob, json, os, sys
root = sys.argv[1]
with open(os.path.join(root, "model.safetensors.index.json"), "rb") as stream:
    index = json.load(stream)
weight_map = index["weight_map"]
declared = set(weight_map.values())
actual = {os.path.basename(path) for path in glob.glob(os.path.join(root, "*.safetensors"))}
if declared != actual:
    raise SystemExit("index shard set differs from the checkpoint root")
print(index["metadata"]["total_size"], len(weight_map), len(declared))
' "${model_dir}"
)
if [[ "${index_total_size}" != "${expected_index_total_size}" ||
      "${tensor_count}" != "${expected_tensor_count}" ||
      "${index_shard_count}" != "${expected_shard_count}" ]]; then
  echo "Hybrid safetensors index inventory mismatch" >&2
  exit 65
fi

active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
if [[ -n "${active_pids//[[:space:]]/}" ]]; then
  echo "cn4 has active GPU compute processes; audit did not start" >&2
  exit 75
fi

source_commit_before="$(git rev-parse HEAD)"
mkdir -p "${GLMAXX_EVIDENCE_DIR}"

date -u +%Y-%m-%dT%H:%M:%SZ | tee "${GLMAXX_EVIDENCE_DIR}/start-utc.txt"
printf '%s\n' "${source_commit_before}" | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
printf '%s\n' "${model_dir}" | tee "${GLMAXX_EVIDENCE_DIR}/checkpoint-path.txt"
printf '%s\n' \
  "schema=glmaxx.hybrid-source-hash.v1" \
  "config_sha256=${expected_config_sha256}" \
  "index_sha256=${expected_index_sha256}" \
  "tier_sha256=${expected_tier_sha256}" \
  "regular_file_count=${file_count}" \
  "regular_file_bytes=${file_bytes}" \
  "shard_count=${shard_count}" \
  "shard_file_bytes=${shard_bytes}" \
  "index_total_size=${index_total_size}" \
  "tensor_count=${tensor_count}" \
  "identity_basis=complete-local-read-only-content-hash" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-binding.txt"
nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total \
  --format=csv,noheader | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_gpu_memory \
  --format=csv,noheader > "${GLMAXX_EVIDENCE_DIR}/compute-apps-before.csv" 2>/dev/null || true
findmnt -T "${model_dir}" -o SOURCE,FSTYPE,OPTIONS,TARGET \
  | tee "${GLMAXX_EVIDENCE_DIR}/checkpoint-filesystem.txt"
sha256sum scripts/cn4-hybrid-source-hash.sh \
  | tee "${GLMAXX_EVIDENCE_DIR}/audit-script-sha256.txt"

fingerprint() {
  find "${model_dir}" -maxdepth 1 -type f \
    -printf '%f\t%D\t%i\t%s\t%T@\n' | LC_ALL=C sort
}
fingerprint > "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-before.txt"

mapfile -d '' source_files < <(
  find "${model_dir}" -maxdepth 1 -type f -printf '%f\0' | LC_ALL=C sort -z
)
audit_start_ns="$(date +%s%N)"
(
  cd "${model_dir}"
  nice -n 19 ionice -c 3 sha256sum -- "${source_files[@]}"
) | tee "${GLMAXX_EVIDENCE_DIR}/source-file-sha256.txt"
audit_end_ns="$(date +%s%N)"

fingerprint > "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-after.txt"
if ! cmp -s "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-before.txt" \
    "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-after.txt"; then
  echo "Checkpoint metadata changed during hashing" >&2
  exit 70
fi
if [[ "$(wc -l < "${GLMAXX_EVIDENCE_DIR}/source-file-sha256.txt" | tr -d ' ')" != \
      "${expected_file_count}" ]]; then
  echo "Content-hash result count mismatch" >&2
  exit 70
fi

elapsed_ns="$((audit_end_ns - audit_start_ns))"
printf '%s\n' \
  "start_nanoseconds=${audit_start_ns}" \
  "end_nanoseconds=${audit_end_ns}" \
  "elapsed_nanoseconds=${elapsed_ns}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/timing.txt"
sha256sum "${GLMAXX_EVIDENCE_DIR}/source-file-sha256.txt" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-manifest-sha256.txt"

active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
if [[ -n "${active_pids//[[:space:]]/}" ]]; then
  echo "A GPU workload appeared during hashing; result is not publishable" >&2
  exit 75
fi
if [[ "$(git rev-parse HEAD)" != "${source_commit_before}" ||
      -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during hashing" >&2
  exit 70
fi

date -u +%Y-%m-%dT%H:%M:%SZ | tee "${GLMAXX_EVIDENCE_DIR}/end-utc.txt"
sha256sum \
  "${GLMAXX_EVIDENCE_DIR}/audit-script-sha256.txt" \
  "${GLMAXX_EVIDENCE_DIR}/checkpoint-filesystem.txt" \
  "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-before.txt" \
  "${GLMAXX_EVIDENCE_DIR}/file-fingerprints-after.txt" \
  "${GLMAXX_EVIDENCE_DIR}/source-binding.txt" \
  "${GLMAXX_EVIDENCE_DIR}/source-file-sha256.txt" \
  "${GLMAXX_EVIDENCE_DIR}/source-manifest-sha256.txt" \
  "${GLMAXX_EVIDENCE_DIR}/timing.txt" \
  | tee "${GLMAXX_EVIDENCE_DIR}/evidence-sha256.txt"
printf '%s\n' \
  "HYBRID_SOURCE_CONTENT_HASH_PASS" \
  "All 194 top-level regular files, including all 184 safetensors shards, were hashed read-only." \
  "The checkpoint and source fingerprints remained unchanged; no CUDA context or kernel was created." \
  "This is local content identity evidence, not publisher provenance or checkpoint admission." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
