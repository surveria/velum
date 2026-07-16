#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fuzzing_dir="$(dirname "${script_dir}")"
revision_file="${fuzzing_dir}/FUZZILLI_REVISION"
checkout_dir="${fuzzing_dir}/fuzzilli"
profile_patch="${fuzzing_dir}/patches/fuzzilli-velum-profile.patch"
repository_url="https://github.com/googleprojectzero/fuzzilli.git"

revision="$(tr -d '[:space:]' < "${revision_file}")"
if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ ]]; then
    printf 'Invalid Fuzzilli revision in %s\n' "${revision_file}" >&2
    exit 1
fi

if [[ ! -d "${checkout_dir}/.git" ]]; then
    git clone --filter=blob:none --no-checkout "${repository_url}" "${checkout_dir}"
    git -C "${checkout_dir}" checkout --detach "${revision}"
fi

actual_revision="$(git -C "${checkout_dir}" rev-parse HEAD)"
if [[ "${actual_revision}" != "${revision}" ]]; then
    printf 'Fuzzilli checkout is at %s, expected %s\n' \
        "${actual_revision}" "${revision}" >&2
    exit 1
fi

if git -C "${checkout_dir}" apply --reverse --check "${profile_patch}" >/dev/null 2>&1; then
    printf 'Velum profile is already applied to Fuzzilli %s\n' "${revision}"
elif git -C "${checkout_dir}" apply --check "${profile_patch}"; then
    git -C "${checkout_dir}" apply "${profile_patch}"
    printf 'Applied the Velum profile to Fuzzilli %s\n' "${revision}"
else
    printf 'The Velum profile patch does not apply cleanly to Fuzzilli %s\n' \
        "${revision}" >&2
    exit 1
fi
