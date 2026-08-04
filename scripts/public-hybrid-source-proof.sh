#!/usr/bin/env bash
set -euo pipefail

readonly expected_repository="madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid"
readonly expected_revision="68babde27a97a4c980c2494e830dd424975cd5a3"
readonly expected_manifest_sha256="a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7"
readonly expected_file_count="194"
readonly expected_file_bytes="366021385004"
readonly expected_lfs_count="186"
readonly expected_plain_count="8"
readonly expected_shard_count="184"
readonly expected_shard_bytes="365987273208"

if [[ -z "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_EVIDENCE_DIR is required" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must remain outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before public-source proof" >&2
  exit 65
fi
for command in curl python3 sha256sum; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "Required command is unavailable: ${command}" >&2
    exit 69
  fi
done

manifest="manifests/glm52-hybrid-source-v1.sha256"
if [[ "$(sha256sum "${manifest}" | awk '{print $1}')" != \
      "${expected_manifest_sha256}" ]]; then
  echo "Checked-in hybrid source manifest identity mismatch" >&2
  exit 65
fi

proof_tmp="$(mktemp -d /tmp/glmaxx-public-hybrid-proof.XXXXXX)"
cleanup() {
  case "${proof_tmp}" in
    /tmp/glmaxx-public-hybrid-proof.*) rm -rf -- "${proof_tmp}" ;;
    *) echo "Refusing unexpected temporary cleanup target" >&2 ;;
  esac
}
trap cleanup EXIT

source_commit_before="$(git rev-parse HEAD)"
mkdir -p "${GLMAXX_EVIDENCE_DIR}"
date -u +%Y-%m-%dT%H:%M:%SZ | tee "${GLMAXX_EVIDENCE_DIR}/start-utc.txt"
printf '%s\n' "${source_commit_before}" | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
curl --version | tee "${GLMAXX_EVIDENCE_DIR}/curl-version.txt"
python3 --version 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/python-version.txt"
sha256sum "${manifest}" scripts/public-hybrid-source-proof.sh \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

api_url="https://huggingface.co/api/models/${expected_repository}/revision/${expected_revision}?blobs=true"
curl -fsSL --retry 3 "${api_url}" -o "${proof_tmp}/model-api.json"
cp "${proof_tmp}/model-api.json" "${GLMAXX_EVIDENCE_DIR}/model-api.json"

python3 - \
  "${manifest}" \
  "${proof_tmp}/model-api.json" \
  "${GLMAXX_EVIDENCE_DIR}" \
  "${expected_revision}" \
  "${expected_file_count}" \
  "${expected_file_bytes}" \
  "${expected_lfs_count}" \
  "${expected_plain_count}" \
  "${expected_shard_count}" \
  "${expected_shard_bytes}" <<'PY'
import json
import pathlib
import re
import sys

(
    manifest_path,
    api_path,
    evidence_path,
    expected_revision,
    expected_file_count,
    expected_file_bytes,
    expected_lfs_count,
    expected_plain_count,
    expected_shard_count,
    expected_shard_bytes,
) = sys.argv[1:]

manifest_bytes = pathlib.Path(manifest_path).read_bytes()
if not manifest_bytes.endswith(b"\n"):
    raise SystemExit("source manifest lacks its final newline")
rows = manifest_bytes[:-1].split(b"\n")
manifest = {}
for row in rows:
    if not re.fullmatch(rb"[0-9a-f]{64}  [A-Za-z0-9._-]+", row):
        raise SystemExit("source manifest has noncanonical syntax")
    name = row[66:].decode("ascii")
    digest = row[:64].decode("ascii")
    if name in manifest:
        raise SystemExit("source manifest has a duplicate filename")
    manifest[name] = digest

api = json.loads(pathlib.Path(api_path).read_bytes())
if api.get("sha") != expected_revision:
    raise SystemExit("public API revision mismatch")
siblings = api.get("siblings")
if not isinstance(siblings, list):
    raise SystemExit("public API has no sibling inventory")
public = {entry.get("rfilename"): entry for entry in siblings}
if None in public or len(public) != len(siblings) or set(public) != set(manifest):
    raise SystemExit("public and checked-in filename sets differ")

lfs = []
plain = []
total_bytes = 0
shard_bytes = 0
shard_count = 0
shard_pattern = re.compile(r"model-[0-9]{5}-of-00184[.]safetensors")
for name in sorted(public):
    entry = public[name]
    size = entry.get("size")
    if not isinstance(size, int) or size < 0:
        raise SystemExit(f"invalid public size for {name}")
    total_bytes += size
    if shard_pattern.fullmatch(name):
        shard_count += 1
        shard_bytes += size
    lfs_record = entry.get("lfs")
    if lfs_record is None:
        plain.append((name, manifest[name], size))
        continue
    if (
        lfs_record.get("sha256") != manifest[name]
        or lfs_record.get("size") != size
    ):
        raise SystemExit(f"publisher LFS identity mismatch for {name}")
    lfs.append((name, manifest[name], size))

expected_plain = {
    ".gitattributes",
    "README.md",
    "chat_template.jinja",
    "config.json",
    "docker-compose.yml",
    "generation_config.json",
    "mxfp8_tier_nokvb.json",
    "tokenizer_config.json",
}
checks = {
    "file_count": (len(public), int(expected_file_count)),
    "file_bytes": (total_bytes, int(expected_file_bytes)),
    "lfs_count": (len(lfs), int(expected_lfs_count)),
    "plain_count": (len(plain), int(expected_plain_count)),
    "shard_count": (shard_count, int(expected_shard_count)),
    "shard_bytes": (shard_bytes, int(expected_shard_bytes)),
}
for name, (observed, expected) in checks.items():
    if observed != expected:
        raise SystemExit(f"{name} mismatch: {observed} != {expected}")
if {row[0] for row in plain} != expected_plain:
    raise SystemExit("unexpected Git-backed publisher file set")

evidence = pathlib.Path(evidence_path)
with (evidence / "publisher-lfs-identities.tsv").open("w", encoding="ascii") as out:
    for name, digest, size in lfs:
        out.write(f"{name}\t{digest}\t{size}\n")
with (evidence / "publisher-plain-identities.tsv").open("w", encoding="ascii") as out:
    for name, digest, size in plain:
        out.write(f"{name}\t{digest}\t{size}\n")
summary = {
    "schema": "glmaxx.public-hybrid-source-proof.v1",
    "revision": expected_revision,
    "file_count": len(public),
    "file_bytes": total_bytes,
    "lfs_count": len(lfs),
    "plain_count": len(plain),
    "shard_count": shard_count,
    "shard_bytes": shard_bytes,
    "lfs_sha256_and_size_match": True,
}
(evidence / "publisher-summary.json").write_text(
    json.dumps(summary, sort_keys=True, indent=2) + "\n",
    encoding="ascii",
)
PY

plain_result="${GLMAXX_EVIDENCE_DIR}/publisher-plain-body-sha256.txt"
: > "${plain_result}"
while IFS=$'\t' read -r name expected_sha256 expected_bytes; do
  output="${proof_tmp}/${name}"
  file_url="https://huggingface.co/${expected_repository}/resolve/${expected_revision}/${name}"
  curl -fsSL --retry 3 "${file_url}" -o "${output}"
  observed_sha256="$(sha256sum "${output}" | awk '{print $1}')"
  observed_bytes="$(wc -c < "${output}" | tr -d ' ')"
  if [[ "${observed_sha256}" != "${expected_sha256}" ||
        "${observed_bytes}" != "${expected_bytes}" ]]; then
    echo "Publisher body identity mismatch: ${name}" >&2
    exit 70
  fi
  printf '%s\t%s\t%s\n' "${name}" "${observed_sha256}" "${observed_bytes}" \
    | tee -a "${plain_result}"
done < "${GLMAXX_EVIDENCE_DIR}/publisher-plain-identities.tsv"

if [[ "$(git rev-parse HEAD)" != "${source_commit_before}" ||
      -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during public-source proof" >&2
  exit 70
fi

date -u +%Y-%m-%dT%H:%M:%SZ | tee "${GLMAXX_EVIDENCE_DIR}/end-utc.txt"
sha256sum \
  "${GLMAXX_EVIDENCE_DIR}/model-api.json" \
  "${GLMAXX_EVIDENCE_DIR}/publisher-lfs-identities.tsv" \
  "${GLMAXX_EVIDENCE_DIR}/publisher-plain-identities.tsv" \
  "${GLMAXX_EVIDENCE_DIR}/publisher-plain-body-sha256.txt" \
  "${GLMAXX_EVIDENCE_DIR}/publisher-summary.json" \
  | tee "${GLMAXX_EVIDENCE_DIR}/evidence-sha256.txt"
printf '%s\n' \
  "HYBRID_IMMUTABLE_PUBLISHER_SOURCE_PASS" \
  "All 186 LFS SHA-256/size identities and all eight Git-backed file bodies match the 194-row GLMAXX manifest." \
  "The public API and every body were resolved at the exact immutable revision." \
  "This authenticates source bytes; it does not accept checkpoint semantics, conversion, CUDA, quality, or performance." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
