#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 ABSOLUTE_CLEAN_SOURCE EXPECTED_COMMIT target-layer3|draft-layer78" >&2
    exit 64
}

if [[ "$#" -ne 3 ]]; then
    usage
fi

readonly source_input="$1"
readonly expected_commit="$2"
readonly qualification_case="$3"
readonly root="/home/derek/glmaxx"
readonly image="sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719"
readonly cutlass="${root}/deps/cutlass"
readonly checkpoint_root="/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw"

case "${source_input}" in
    "${root}"/worktrees/*) ;;
    *)
        echo "source must be an isolated GLMAXX worktree" >&2
        exit 64
        ;;
esac
if [[ ! "${expected_commit}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "expected commit must be a lowercase 40-digit Git object ID" >&2
    exit 64
fi
if [[ -L "${source_input}" || ! -d "${source_input}" ]]; then
    echo "source must be an existing, non-symlink directory" >&2
    exit 65
fi

readonly source="$(cd "${source_input}" && pwd -P)"
case "${source}" in
    "${root}"/worktrees/*) ;;
    *)
        echo "resolved source escaped the isolated GLMAXX worktree root" >&2
        exit 65
        ;;
esac

case "${qualification_case}" in
    target-layer3)
        readonly checkpoint_shard="${checkpoint_root}/model-layer-003.safetensors"
        ;;
    draft-layer78)
        readonly checkpoint_shard="${checkpoint_root}/model-layer-078.safetensors"
        ;;
    *) usage ;;
esac

readonly actual_commit="$(git -C "${source}" rev-parse HEAD)"
[[ "${actual_commit}" == "${expected_commit}" ]]
[[ -z "$(git -C "${source}" status --porcelain)" ]]
[[ -f "${checkpoint_shard}" && ! -L "${checkpoint_shard}" ]]
[[ "$(git -C "${cutlass}" rev-parse HEAD)" == "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]
docker image inspect "${image}" >/dev/null

readonly run="$(${source}/scripts/new-evidence-run.sh \
    "${root}/evidence" "current-real-k3-${qualification_case}-${expected_commit:0:7}")"
readonly build="${root}/build/$(basename "${run}")"
readonly qualification="${run}/qualification"
[[ ! -e "${build}" && ! -e "${qualification}" ]]
"${source}/scripts/begin-evidence-run.sh" "${run}" >/dev/null

terminal="FAILED"
finalize() {
    local exit_code=$?
    trap - EXIT HUP INT TERM
    set +e
    if [[ "$(<"${run}/allocation-state.txt")" == "RUNNING" ]]; then
        date -u +%Y-%m-%dT%H:%M:%SZ >"${run}/command-finish-utc.txt"
        nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
            --format=csv,noheader >"${run}/compute-apps-after.csv"
        "${source}/scripts/finish-evidence-run.sh" "${run}" "${terminal}" >/dev/null
    fi
    printf 'RUN_DIR=%s\n' "${run}"
    if [[ -f "${run}/evidence-sha256.txt" ]]; then
        sha256sum "${run}/evidence-sha256.txt"
    fi
    exit "${exit_code}"
}
trap finalize EXIT HUP INT TERM

cp "$0" "${run}/run.sh"
printf '%s\n' "${actual_commit}" >"${run}/source-commit.txt"
git -C "${source}" status --short --branch >"${run}/source-status.txt"
git -C "${source}" diff --no-ext-diff --binary >"${run}/source.patch"
git -C "${source}" show --format=fuller --stat --summary HEAD \
    >"${run}/source-commit-show.txt"
printf '%s\n' "${image}" >"${run}/container-identity.txt"
printf '%s\n' "${qualification_case}" >"${run}/qualification-case.txt"
printf '%s\n' "${checkpoint_shard}" >"${run}/checkpoint-path.txt"
docker image inspect "${image}" >"${run}/container-image-inspect.json"
docker version >"${run}/docker-version.txt"
hostname >"${run}/hostname.txt"
date -u +%Y-%m-%dT%H:%M:%SZ >"${run}/command-start-utc.txt"
lscpu >"${run}/lscpu.txt"
uptime >"${run}/uptime-before.txt"
ps -eo pid,psr,pcpu,pmem,comm --sort=-pcpu >"${run}/processes-before.txt"
nvidia-smi -q >"${run}/nvidia-smi-before.txt"
nvidia-smi topo -m >"${run}/topology.txt"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader >"${run}/compute-apps-before.csv"

if [[ -s "${run}/compute-apps-before.csv" ]]; then
    echo "cn4 is occupied; no GLMAXX GPU work was launched" >&2
    exit 75
fi

readonly container_name="$(basename "${run}")"
docker_args=(
    run --rm
    --name "${container_name}"
    --pull never
    --network none
    --ipc private
    --gpus all
    --volume "${root}:${root}:ro"
    --volume "${root}/build:${root}/build"
    --volume "${root}/evidence:${root}/evidence"
    --volume "${root}/cache/cargo/registry:/usr/local/cargo/registry:ro"
    --volume "${checkpoint_shard}:${checkpoint_shard}:ro"
    --workdir "${source}"
    --env GIT_CONFIG_COUNT=2
    --env GIT_CONFIG_KEY_0=safe.directory
    --env "GIT_CONFIG_VALUE_0=${source}"
    --env GIT_CONFIG_KEY_1=safe.directory
    --env "GIT_CONFIG_VALUE_1=${cutlass}"
    --env GLMAXX_CN4_AUTHORIZATION=active-glmaxx-goal-20260803
    --env "CUTLASS_DIR=${cutlass}"
    --env "GLMAXX_BUILD_DIR=${build}"
    --env "GLMAXX_EVIDENCE_DIR=${qualification}"
    --env "GLMAXX_TR3_CASE=${qualification_case}"
    --env "GLMAXX_TR3_SHARD=${checkpoint_shard}"
    --env "GLMAXX_CONTAINER_DIGEST=${image}"
    "${image}"
    bash scripts/cn4-exl3-real-k3-v1.sh
)
{
    printf 'docker'
    printf ' %q' "${docker_args[@]}"
    printf '\n'
} >"${run}/command.txt"

docker "${docker_args[@]}" \
    >"${run}/container.stdout.txt" \
    2>"${run}/container.stderr.txt"

(
    cd "${qualification}"
    sha256sum -c artifact-manifest.txt
) >"${run}/inner-manifest-verification.txt"
sha256sum "${qualification}/summary.json" \
    "${qualification}/artifact-manifest.txt" \
    "${qualification}/artifact-manifest.sha256" \
    >"${run}/qualification-sha256.txt"
cat "${qualification}/summary.json"

uptime >"${run}/uptime-after.txt"
ps -eo pid,psr,pcpu,pmem,comm --sort=-pcpu >"${run}/processes-after.txt"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader >"${run}/compute-apps-after.csv"
nvidia-smi --query-gpu=index,uuid,memory.used,utilization.gpu,clocks.current.sm,clocks.current.memory,power.draw \
    --format=csv,noheader >"${run}/gpu-state-after.csv"
[[ -z "$(git -C "${source}" status --porcelain)" ]]
[[ "$(git -C "${source}" rev-parse HEAD)" == "${expected_commit}" ]]

date -u +%Y-%m-%dT%H:%M:%SZ >"${run}/command-finish-utc.txt"
terminal="COMPLETE"
"${source}/scripts/finish-evidence-run.sh" "${run}" "${terminal}" >/dev/null
