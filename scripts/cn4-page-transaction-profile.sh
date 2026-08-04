#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 ABSOLUTE_CLEAN_SOURCE EXPECTED_COMMIT" >&2
    exit 64
}

if [[ "$#" -ne 2 ]]; then
    usage
fi

readonly source_input="$1"
readonly expected_commit="$2"
readonly root="/home/derek/glmaxx"
readonly image="sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842"

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

readonly actual_commit="$(git -C "${source}" rev-parse HEAD)"
[[ "${actual_commit}" == "${expected_commit}" ]]
[[ -z "$(git -C "${source}" status --porcelain)" ]]

readonly run="$(${source}/scripts/new-evidence-run.sh \
    "${root}/evidence" "page-transaction-profile-${expected_commit:0:7}")"
readonly build="${root}/build/$(basename "${run}")"
readonly result="${run}/result"
readonly profile="${result}/profile"
[[ ! -e "${build}" ]]
mkdir -p "${build}" "${profile}"
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
printf '%s\n' \
    "docker run --rm --network none --cpuset-cpus 0-15 --env NVIDIA_VISIBLE_DEVICES=void ${image} cargo build --locked --offline --release -p glm-cli --bin glmaxx; glmaxx page-transaction-profile RESULT/profile ${expected_commit} 10 100" \
    >"${run}/command.txt"
printf '%s\n' "${actual_commit}" >"${run}/source-commit.txt"
git -C "${source}" status --short --branch >"${run}/source-status.txt"
git -C "${source}" diff --no-ext-diff --binary >"${run}/source.patch"
git -C "${source}" show --format=fuller --stat --summary HEAD \
    >"${run}/source-commit-show.txt"
printf '%s\n' "${image}" >"${run}/container-identity.txt"
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

docker run --rm --network none --cpuset-cpus 0-15 \
    --env NVIDIA_VISIBLE_DEVICES=void \
    --volume "${source}:${source}:ro" \
    --volume "${build}:${build}" \
    --volume "${root}/cache/cargo/registry:/usr/local/cargo/registry:ro" \
    --volume "${result}:${result}" \
    --workdir "${source}" \
    --env CARGO_TARGET_DIR="${build}/cargo-target" \
    --env GLMAXX_RESULT_DIR="${result}" \
    --env GLMAXX_EXPECTED_COMMIT="${expected_commit}" \
    "${image}" bash -ceu '
        rustc --version --verbose >"${GLMAXX_RESULT_DIR}/rustc.txt"
        cargo --version --verbose >"${GLMAXX_RESULT_DIR}/cargo.txt"
        cargo build --locked --offline --release -p glm-cli --bin glmaxx \
            >"${GLMAXX_RESULT_DIR}/build.stdout.txt" \
            2>"${GLMAXX_RESULT_DIR}/build.stderr.txt"
        binary="${CARGO_TARGET_DIR}/release/glmaxx"
        sha256sum "${binary}" >"${GLMAXX_RESULT_DIR}/binary-sha256.txt"
        "${binary}" page-transaction-profile \
            "${GLMAXX_RESULT_DIR}/profile" "${GLMAXX_EXPECTED_COMMIT}" 10 100 \
            >"${GLMAXX_RESULT_DIR}/profile.stdout.txt" \
            2>"${GLMAXX_RESULT_DIR}/profile.stderr.txt"
    ' >"${run}/container.stdout.txt" 2>"${run}/container.stderr.txt"

uptime >"${run}/uptime-after.txt"
ps -eo pid,psr,pcpu,pmem,comm --sort=-pcpu >"${run}/processes-after.txt"
sha256sum "${profile}/page-transaction-profile.json" \
    >"${run}/result-sha256.txt"
jq -n \
    --arg schema "glmaxx.cn4-page-transaction-profile.v1" \
    --arg source_commit "${expected_commit}" \
    --arg raw_sha256 "$(sha256sum "${profile}/page-transaction-profile.json" | cut -d' ' -f1)" \
    --slurpfile profile "${profile}/page-transaction-profile.json" \
    '{
        schema: $schema,
        source_commit: $source_commit,
        raw_sha256: $raw_sha256,
        gpu_launched: false,
        claim: "synthetic CPU page-transaction overhead only",
        profile: $profile[0]
    }' >"${run}/summary.json"

[[ -z "$(git -C "${source}" status --porcelain)" ]]
date -u +%Y-%m-%dT%H:%M:%SZ >"${run}/command-finish-utc.txt"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader >"${run}/compute-apps-after.csv"
terminal="COMPLETE"
"${source}/scripts/finish-evidence-run.sh" "${run}" "${terminal}" >/dev/null
jq -c '{schema,source_commit,raw_sha256,gpu_launched,claim,cells:(.profile.cells|length)}' \
    "${run}/summary.json"
