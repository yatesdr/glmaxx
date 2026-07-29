#!/usr/bin/env bash
set -euo pipefail

repository="brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw"
revision="9297b9f1d53af5c67cffa01e30cc071a1ff7144b"
manifest_sha256="bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63"
gitattributes_manifest_sha256="34448b82c17d60fec9b65b1f093c115ddbaadc04beb1b0140b6bfed2e012a930"
gitattributes_revision_sha256="5bb36c320417db43af1dc6af8bd0fcc154bb7276eddaf96b12c395bdafed634d"
readme_manifest_sha256="ed5aca8ce3dc5f8de626c87e488444343e43b1dcbdeb0e643dc72fea63ab06e8"
readme_revision_sha256="e60e023082ee175a11f51e79e8dd88f5e4ed9975fc904e64cdeabbbcf8abe225"
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

expected_file_sha256() {
  local file="$1"
  local manifest_expected
  manifest_expected="$(awk -v file="${file}" '$2 == file {print $1}' "${manifest}")"
  if [[ ! "${manifest_expected}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "No unique pinned manifest digest for ${file}" >&2
    return 65
  fi
  case "${file}" in
    .gitattributes)
      if [[ "${manifest_expected}" != "${gitattributes_manifest_sha256}" ]]; then
        echo "Pinned .gitattributes manifest tuple changed" >&2
        return 65
      fi
      printf '%s\n' "${gitattributes_revision_sha256}"
      ;;
    README.md)
      if [[ "${manifest_expected}" != "${readme_manifest_sha256}" ]]; then
        echo "Pinned README.md manifest tuple changed" >&2
        return 65
      fi
      printf '%s\n' "${readme_revision_sha256}"
      ;;
    *)
      printf '%s\n' "${manifest_expected}"
      ;;
  esac
}

download_file() {
  local file="$1"
  local expected
  local actual
  expected="$(expected_file_sha256 "${file}")"
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

verified_files=0
while read -r manifest_expected file; do
  if [[ ! "${manifest_expected}" =~ ^[0-9a-f]{64}$ || -z "${file}" ]]; then
    echo "Malformed pinned manifest record" >&2
    exit 65
  fi
  expected="$(expected_file_sha256 "${file}")"
  actual="$(sha256sum "${destination}/${file}" | awk '{print $1}')"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "Final checkpoint verification failed: ${file}" >&2
    exit 65
  fi
  verified_files=$((verified_files + 1))
done < "${manifest}"
if [[ "${verified_files}" != "92" ]]; then
  echo "Final checkpoint inventory is not exactly 92 files" >&2
  exit 65
fi

printf '%s\n' \
  "${repository}" > "${destination}/glmaxx-source-repository.txt"
printf '%s\n' \
  "${revision}" > "${destination}/glmaxx-source-revision.txt"
printf '%s\n' \
  "PINNED_CHECKPOINT_DOWNLOAD_PASS" \
  "repository=${repository}" \
  "revision=${revision}" \
  "manifest_sha256=${manifest_sha256}" \
  "publisher_manifest_exceptions=2" \
  "verified_files=${verified_files}" \
  "destination=${destination}"
