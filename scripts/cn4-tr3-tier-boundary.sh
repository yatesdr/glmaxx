#!/usr/bin/env bash
set -euo pipefail

# Read-only metadata proof for the TR3 target/draft EXL3-width boundary. The
# script reads the index, tier map, and safetensors headers only; it never
# reads tensor payloads or creates a CUDA context.

readonly expected_index_sha256="f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21"
readonly expected_tier_sha256="a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d"

if [[ -z "${GLMAXX_TR3_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_TR3_DIR and GLMAXX_EVIDENCE_DIR are required" >&2
  exit 64
fi

readonly source_dir="$(realpath "${GLMAXX_TR3_DIR}")"
readonly evidence_dir="${GLMAXX_EVIDENCE_DIR}"
readonly index_path="${source_dir}/model.safetensors.index.json"
readonly tier_path="${source_dir}/tier_bitmap.json"

case "${evidence_dir}" in
  /home/derek/glmaxx/evidence/*) ;;
  *)
    echo "Evidence must be isolated under /home/derek/glmaxx/evidence" >&2
    exit 64
    ;;
esac
if [[ -e "${evidence_dir}" ]]; then
  echo "Evidence directory must not already exist" >&2
  exit 65
fi
for path in "${source_dir}" "${index_path}" "${tier_path}"; do
  if [[ -L "${path}" ]]; then
    echo "Source paths must not be symbolic links: ${path}" >&2
    exit 65
  fi
done
if [[ ! -d "${source_dir}" || ! -f "${index_path}" || ! -f "${tier_path}" ]]; then
  echo "TR3 source directory, index, or tier map is missing" >&2
  exit 65
fi
if [[ "$(sha256sum "${index_path}" | awk '{print $1}')" != "${expected_index_sha256}" ||
      "$(sha256sum "${tier_path}" | awk '{print $1}')" != "${expected_tier_sha256}" ]]; then
  echo "TR3 index or tier-map identity changed" >&2
  exit 65
fi

mkdir -p "${evidence_dir}"
date -u +%Y-%m-%dT%H:%M:%SZ > "${evidence_dir}/start-utc.txt"
printf 'GLMAXX_TR3_DIR=%q GLMAXX_EVIDENCE_DIR=%q %q\n' \
  "${source_dir}" "${evidence_dir}" "$0" > "${evidence_dir}/command.txt"
printf '%s\n' "$(git rev-parse HEAD)" > "${evidence_dir}/source-commit.txt"
git status --short --branch > "${evidence_dir}/source-status-before.txt"
sha256sum "${index_path}" "${tier_path}" "$0" > "${evidence_dir}/input-sha256.txt"
{
  jq --version
  dd --version | sed -n '1p'
  od --version | sed -n '1p'
  sha256sum --version | sed -n '1p'
} > "${evidence_dir}/tool-versions.txt"
stat --printf='%n bytes=%s mode=%a type=%F\n' \
  "${index_path}" "${tier_path}" > "${evidence_dir}/source-stat.txt"
nvidia-smi --query-gpu=index,name,uuid,compute_cap,memory.used,utilization.gpu \
  --format=csv,noheader > "${evidence_dir}/gpu-before.csv"
nvidia-smi --query-compute-apps=pid,process_name,used_memory \
  --format=csv,noheader > "${evidence_dir}/compute-before.csv"

jq -c '
  . as $root
  | (keys | map(tonumber) | sort) as $layers
  | [range(3; 78)
      | . as $layer
      | ($root[($layer | tostring)].k) as $k
      | {
          layer: $layer,
          experts: ($k | length),
          k3: ([$k[] | select(. == 3)] | length),
          k4: ([$k[] | select(. == 4)] | length),
          other: ([$k[] | select(. != 3 and . != 4)] | length)
        }] as $target
  | ($root["78"] | {
      layer: 78,
      has_k: has("k"),
      keep_nvfp4_count: (.keep_nvfp4 | length),
      tail_tr3_count: (.tail_tr3 | length),
      tail_tr3_exact: (.tail_tr3 == [range(0; 256)])
    }) as $draft
  | {
      tier_sha256: "a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d",
      layer_count: ($layers | length),
      layer_minimum: ($layers | min),
      layer_maximum: ($layers | max),
      exact_layers_3_through_78: ($layers == [range(3; 79)]),
      target: $target,
      draft: $draft
    }
' "${tier_path}" > "${evidence_dir}/tier-summary.json"

jq -e '
  .layer_count == 76
  and .exact_layers_3_through_78
  and (.target | length) == 75
  and all(.target[]; .experts == 256 and .k3 == 192 and .k4 == 64 and .other == 0)
  and (.draft == {
    layer: 78,
    has_k: false,
    keep_nvfp4_count: 0,
    tail_tr3_count: 256,
    tail_tr3_exact: true
  })
' "${evidence_dir}/tier-summary.json" >/dev/null

: > "${evidence_dir}/tensor-widths.ndjson"
for layer in $(seq 3 78); do
  shard="${source_dir}/model-layer-$(printf '%03d' "${layer}").safetensors"
  if [[ -L "${shard}" || ! -f "${shard}" ]]; then
    echo "Missing regular shard for layer ${layer}" >&2
    exit 65
  fi
  header_bytes="$(od -An -tu8 -N8 "${shard}" | tr -d '[:space:]')"
  if [[ ! "${header_bytes}" =~ ^[0-9]+$ ||
        "${header_bytes}" -eq 0 ||
        "${header_bytes}" -gt 268435456 ||
        $((header_bytes % 8)) -ne 0 ]]; then
    echo "Invalid safetensors header length for layer ${layer}" >&2
    exit 65
  fi
  dd if="${shard}" bs=8 skip=1 count=$((header_bytes / 8)) status=none \
    | jq -c --argjson layer "${layer}" --argjson header_bytes "${header_bytes}" '
        [. as $header
          | to_entries[]
          | select(.key != "__metadata__")
          | select(.key | startswith("model.layers.\($layer).mlp.experts."))
          | select(.key | endswith(".trellis"))] as $trellis
        | def count_width($items; $width):
            [$items[] | select(.value.dtype == "I16" and .value.shape[2] == $width)] | length;
          def role($name):
            [$trellis[] | select(.key | contains(".\($name)."))];
          def rank_items($rank):
            [$trellis[] | select(.key | contains(".rank\($rank)."))];
          {
            layer: $layer,
            shard_header_bytes: $header_bytes,
            trellis_tensors: ($trellis | length),
            k3: count_width($trellis; 48),
            k4: count_width($trellis; 64),
            other: (($trellis | length) - count_width($trellis; 48) - count_width($trellis; 64)),
            projection_counts: {
              gate: {k3: count_width(role("gate_proj"); 48), k4: count_width(role("gate_proj"); 64)},
              up: {k3: count_width(role("up_proj"); 48), k4: count_width(role("up_proj"); 64)},
              down: {k3: count_width(role("down_proj"); 48), k4: count_width(role("down_proj"); 64)}
            },
            rank_counts: [range(0; 4)
              | . as $rank
              | {rank: $rank, k3: count_width(rank_items($rank); 48), k4: count_width(rank_items($rank); 64)}]
          }
      ' \
    >> "${evidence_dir}/tensor-widths.ndjson"
done

jq -s -e '
  length == 76
  and (map(.layer) == [range(3; 79)])
  and all(.[];
    if .layer < 78 then
      .trellis_tensors == 3072 and .k3 == 2304 and .k4 == 768 and .other == 0
      and all(.projection_counts[]; .k3 == 768 and .k4 == 256)
      and all(.rank_counts[]; .k3 == 576 and .k4 == 192)
    else
      .trellis_tensors == 3072 and .k3 == 3072 and .k4 == 0 and .other == 0
      and all(.projection_counts[]; .k3 == 1024 and .k4 == 0)
      and all(.rank_counts[]; .k3 == 768 and .k4 == 0)
    end)
' "${evidence_dir}/tensor-widths.ndjson" >/dev/null

jq -n \
  --slurpfile tier "${evidence_dir}/tier-summary.json" \
  --slurpfile widths "${evidence_dir}/tensor-widths.ndjson" '
    {
      schema: "glmaxx.tr3-tier-boundary-metadata-proof.v1",
      tier: $tier[0],
      tensor_widths: {
        layers: ($widths | length),
        target_layers: ([$widths[] | select(.layer < 78)] | length),
        draft_layers: ([$widths[] | select(.layer == 78)] | length),
        target_k3_trellis_tensors: ([$widths[] | select(.layer < 78) | .k3] | add),
        target_k4_trellis_tensors: ([$widths[] | select(.layer < 78) | .k4] | add),
        draft_k3_trellis_tensors: ([$widths[] | select(.layer == 78) | .k3] | add),
        draft_k4_trellis_tensors: ([$widths[] | select(.layer == 78) | .k4] | add)
      },
      target_sparse_layers: 75,
      recurrent_draft_layer: 78,
      claim: "pinned raw index/tier-map identities and safetensors-header metadata only; no publisher authentication, tensor payload, or CUDA access",
      verdict: "TR3_TARGET_DRAFT_TIER_BOUNDARY_PASS"
    }
  ' > "${evidence_dir}/summary.json"

git status --short --branch > "${evidence_dir}/source-status-after.txt"
date -u +%Y-%m-%dT%H:%M:%SZ > "${evidence_dir}/end-utc.txt"
nvidia-smi --query-gpu=index,name,uuid,compute_cap,memory.used,utilization.gpu \
  --format=csv,noheader > "${evidence_dir}/gpu-after.csv"
nvidia-smi --query-compute-apps=pid,process_name,used_memory \
  --format=csv,noheader > "${evidence_dir}/compute-after.csv"

(
  cd "${evidence_dir}"
  find . -type f ! -name evidence-sha256.txt -print0 \
    | sort -z \
    | xargs -0 sha256sum > evidence-sha256.txt
  sha256sum -c evidence-sha256.txt >/dev/null
)
cat "${evidence_dir}/summary.json"
