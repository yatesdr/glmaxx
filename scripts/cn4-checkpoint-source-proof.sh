#!/usr/bin/env bash
set -euo pipefail

readonly expected_repository="brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw"
readonly expected_revision="9297b9f1d53af5c67cffa01e30cc071a1ff7144b"
readonly expected_manifest_sha256="bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

if [[ -z "${GLMAXX_MODEL_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" ||
      -z "${GLMAXX_CONTAINER_DIGEST:-}" ]]; then
  echo "GLMAXX_MODEL_DIR, GLMAXX_EVIDENCE_DIR, and GLMAXX_CONTAINER_DIGEST are required" >&2
  exit 64
fi
if [[ ! "${GLMAXX_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "GLMAXX_CONTAINER_DIGEST must be a sha256:<64 lowercase hex> identity" >&2
  exit 64
fi

model_dir="$(realpath "${GLMAXX_MODEL_DIR}")"
case "${model_dir}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Checkpoint must remain outside the Git repository" >&2
    exit 64
    ;;
esac
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must remain outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist; use a fresh immutable path" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before checkpoint proof" >&2
  exit 65
fi

index="${model_dir}/model.safetensors.index.json"
manifest="${model_dir}/MANIFEST.sha256"
if [[ ! -f "${index}" || ! -f "${manifest}" ]]; then
  echo "Pinned index and source manifest must exist" >&2
  exit 65
fi
if [[ "$(shasum -a 256 "${manifest}" | awk '{print $1}')" != \
      "${expected_manifest_sha256}" ]]; then
  echo "Source manifest identity mismatch" >&2
  exit 65
fi
repository_marker="${model_dir}/glmaxx-source-repository.txt"
revision_marker="${model_dir}/glmaxx-source-revision.txt"
marker_count=0
[[ -e "${repository_marker}" ]] && marker_count=$((marker_count + 1))
[[ -e "${revision_marker}" ]] && marker_count=$((marker_count + 1))
if [[ "${marker_count}" == "1" ]]; then
  echo "Pinned source marker set is incomplete" >&2
  exit 65
fi
marker_posture="exact-content-addressed-manifest"
if [[ "${marker_count}" == "2" ]]; then
  if [[ "$(cat "${repository_marker}")" != "${expected_repository}" ||
        "$(cat "${revision_marker}")" != "${expected_revision}" ]]; then
    echo "Pinned source markers are incorrect" >&2
    exit 65
  fi
  marker_posture="exact-manifest-and-optional-source-markers"
fi
if find "${model_dir}" -maxdepth 1 -name '*.part' -print -quit | grep -q .; then
  echo "Checkpoint still contains an incomplete download" >&2
  exit 75
fi

source_commit_before="$(git rev-parse HEAD)"
mkdir -p "${GLMAXX_EVIDENCE_DIR}"

printf '%s\n' "${source_commit_before}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s\n' "${model_dir}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/checkpoint-path.txt"
printf '%s\n' \
  "schema=glmaxx.pinned-source-binding.v1" \
  "repository=${expected_repository}" \
  "revision=${expected_revision}" \
  "manifest_sha256=${expected_manifest_sha256}" \
  "index_sha256=346227a4ea44b6063017739ee38a830319dc10305ccf714734095e27b28064c2" \
  "identity_basis=${marker_posture}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-binding.txt"
shasum -a 256 \
  "${manifest}" \
  "${index}" \
  crates/glm-format/src/checkpoint.rs \
  crates/glm-format/src/exl3.rs \
  crates/glm-format/src/safetensors.rs \
  crates/glm-cli/src/main.rs \
  scripts/cn4-checkpoint-source-proof.sh \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_EVIDENCE_DIR}/cargo-target"
cargo test --workspace --offline 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"
cargo build --release --offline -p glm-cli --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-release-build.txt"
binary="${CARGO_TARGET_DIR}/release/glmaxx"
shasum -a 256 "${binary}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/binary-sha256.txt"

"${binary}" checkpoint-proof "${index}" \
  > "${GLMAXX_EVIDENCE_DIR}/checkpoint-structure.json"
"${binary}" checkpoint-source-proof "${index}" \
  > "${GLMAXX_EVIDENCE_DIR}/checkpoint-source.json" \
  2> "${GLMAXX_EVIDENCE_DIR}/checkpoint-source-progress.txt"

for projection in gate up down; do
  report="${GLMAXX_EVIDENCE_DIR}/exl3-layer3-expert0-rank0-${projection}.json"
  replay="${GLMAXX_EVIDENCE_DIR}/exl3-layer3-expert0-rank0-${projection}-replay.json"
  "${binary}" exl3-safetensors-proof "${index}" 3 0 0 "${projection}" \
    > "${report}"
  "${binary}" exl3-safetensors-proof "${index}" 3 0 0 "${projection}" \
    > "${replay}"
  if ! cmp -s "${report}" "${replay}"; then
    echo "Real ${projection} projection proof is not byte-deterministic" >&2
    exit 70
  fi
done

if ! grep -Fq '"verdict": "PINNED_CHECKPOINT_STRUCTURE_PASS"' \
    "${GLMAXX_EVIDENCE_DIR}/checkpoint-structure.json" ||
   ! grep -Fq '"verdict": "PINNED_CHECKPOINT_SOURCE_PASS"' \
    "${GLMAXX_EVIDENCE_DIR}/checkpoint-source.json" ||
   ! grep -Fq "\"identity_basis\": \"${marker_posture}\"" \
    "${GLMAXX_EVIDENCE_DIR}/checkpoint-source.json" ||
   ! grep -Fq '"verified_file_count": 92' \
    "${GLMAXX_EVIDENCE_DIR}/checkpoint-source.json"; then
  echo "Pinned checkpoint proof did not emit all required pass records" >&2
  exit 70
fi

shasum -a 256 \
  "${GLMAXX_EVIDENCE_DIR}/checkpoint-structure.json" \
  "${GLMAXX_EVIDENCE_DIR}/checkpoint-source.json" \
  "${GLMAXX_EVIDENCE_DIR}/source-binding.txt" \
  "${GLMAXX_EVIDENCE_DIR}"/exl3-layer3-expert0-rank0-*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/proof-sha256.txt"

if [[ "$(git rev-parse HEAD)" != "${source_commit_before}" ||
      -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during checkpoint proof" >&2
  exit 70
fi

printf '%s\n' \
  "PINNED_EXL3_CPU_PROOF_PASS" \
  "All 92 manifest files were SHA-256 verified and all 81 shards were structurally validated." \
  "Actual gate, up, and down layer-3/expert-0/rank-0 source payloads reconstructed deterministically without conversion." \
  "No CUDA feature or device access was used." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
