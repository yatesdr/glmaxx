#!/usr/bin/env bash
set -euo pipefail

repository="brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw"
revision="9297b9f1d53af5c67cffa01e30cc071a1ff7144b"
manifest_sha256="bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63"
destination="${1:-/nvme-kv/glmaxx-models/GLM-5.2-EXL3-TR3-3.0bpw-9297b9f}"
base_url="https://huggingface.co/${repository}/resolve/${revision}"

mkdir -p "${destination}"

manifest="${destination}/MANIFEST.sha256"
if [[ -e "${manifest}" ]]; then
  actual_manifest_sha256="$(sha256sum "${manifest}" | awk '{print $1}')"
  if [[ "${actual_manifest_sha256}" != "${manifest_sha256}" ]]; then
    echo "Existing manifest has the wrong digest: ${manifest}" >&2
    exit 65
  fi
else
  curl --fail --location --silent --show-error \
    --retry 20 --retry-all-errors --connect-timeout 30 \
    --output "${manifest}.part" "${base_url}/MANIFEST.sha256"
  actual_manifest_sha256="$(sha256sum "${manifest}.part" | awk '{print $1}')"
  if [[ "${actual_manifest_sha256}" != "${manifest_sha256}" ]]; then
    echo "Downloaded manifest digest mismatch" >&2
    exit 65
  fi
  mv "${manifest}.part" "${manifest}"
fi

download_file() {
  local file="$1"
  local expected
  local actual
  expected="$(awk -v file="${file}" '$2 == file {print $1}' "${manifest}")"
  if [[ ! "${expected}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "No unique pinned digest for ${file}" >&2
    return 65
  fi
  if [[ -e "${destination}/${file}" ]]; then
    actual="$(sha256sum "${destination}/${file}" | awk '{print $1}')"
    if [[ "${actual}" != "${expected}" ]]; then
      echo "Existing file has the wrong digest: ${destination}/${file}" >&2
      return 65
    fi
    echo "verified existing ${file}"
    return 0
  fi
  echo "downloading ${file}"
  curl --fail --location --silent --show-error \
    --retry 20 --retry-all-errors --connect-timeout 30 \
    --speed-limit 1048576 --speed-time 120 \
    --continue-at - --output "${destination}/${file}.part" \
    "${base_url}/${file}"
  actual="$(sha256sum "${destination}/${file}.part" | awk '{print $1}')"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "Downloaded file digest mismatch: ${file}" >&2
    return 65
  fi
  mv "${destination}/${file}.part" "${destination}/${file}"
  echo "verified ${file}"
}

for file in \
  .gitattributes \
  LICENSE \
  README.md \
  calibration_manifest.json \
  config.json \
  generation_config.json \
  chat_template.jinja \
  tokenizer.json \
  tokenizer_config.json \
  tier_bitmap.json \
  model.safetensors.index.json \
  model-embed.safetensors \
  model-head.safetensors
do
  download_file "${file}"
done

for layer in {0..78}; do
  printf -v file 'model-layer-%03d.safetensors' "${layer}"
  download_file "${file}"
done

(
  cd "${destination}"
  sha256sum --check --strict MANIFEST.sha256
)

printf '%s\n' \
  "${repository}" > "${destination}/glmaxx-source-repository.txt"
printf '%s\n' \
  "${revision}" > "${destination}/glmaxx-source-revision.txt"
printf '%s\n' \
  "PINNED_CHECKPOINT_DOWNLOAD_PASS" \
  "repository=${repository}" \
  "revision=${revision}" \
  "manifest_sha256=${manifest_sha256}" \
  "destination=${destination}"
