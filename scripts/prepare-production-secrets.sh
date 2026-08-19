#!/bin/sh
set -eu

usage() {
    cat <<'EOF'
Usage: sudo sh scripts/prepare-production-secrets.sh [--check]

With no option, set every production Compose secret source to owner root,
PEPEAUDIO_RUNTIME_GID (default 10001), and mode 0440. --check performs the same
validation without changing files. Secret values are never printed.
EOF
}

mode=prepare
case "${1:-}" in
    '')
        ;;
    --check)
        mode=check
        ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
. "$script_dir/lib/production-secrets.sh"

runtime_gid=${PEPEAUDIO_RUNTIME_GID:-10001}
case "$runtime_gid" in
    ''|*[!0-9]*)
        printf '%s\n' 'PEPEAUDIO_RUNTIME_GID must be a positive numeric GID.' >&2
        exit 2
        ;;
esac
if [ "$runtime_gid" -eq 0 ] || [ "$runtime_gid" -gt 2147483647 ]; then
    printf '%s\n' 'PEPEAUDIO_RUNTIME_GID must be between 1 and 2147483647.' >&2
    exit 2
fi

if [ "$(uname -s)" != Linux ]; then
    printf '%s\n' \
        'Production secret ownership preparation is supported only on Linux.' >&2
    exit 1
fi

if [ "$mode" = prepare ] && [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' \
        'Preparing production secrets requires root; run this script with sudo.' >&2
    exit 1
fi

secret_path() {
    production_secret_absolute_path "$repository_root" "$1"
}

assert_materialized_secret() {
    label=$1
    path=$(secret_path "$2")

    if [ -L "$path" ]; then
        printf 'Secret source %s must not be a symbolic link: %s\n' \
            "$label" "$path" >&2
        exit 1
    fi
    if [ ! -f "$path" ]; then
        printf 'Secret source %s is missing or not a regular file: %s\n' \
            "$label" "$path" >&2
        exit 1
    fi
    if [ ! -s "$path" ]; then
        printf 'Secret source %s is empty: %s\n' "$label" "$path" >&2
        exit 1
    fi
}

assert_secret() {
    label=$1
    path=$(secret_path "$2")

    assert_materialized_secret "$label" "$2"

    metadata=$(stat --printf '%u:%g:%a' -- "$path")
    expected="0:$runtime_gid:440"
    if [ "$metadata" != "$expected" ]; then
        printf 'Secret source %s has %s; expected %s: %s\n' \
            "$label" "$metadata" "$expected" "$path" >&2
        exit 1
    fi

    printf 'Verified %s\n' "$label"
}

set_secret_metadata() {
    label=$1
    path=$(secret_path "$2")

    chown "0:$runtime_gid" -- "$path"
    chmod 0440 -- "$path"
}

production_secret_sources assert_materialized_secret "$repository_root"
if [ "$mode" = prepare ]; then
    production_secret_sources set_secret_metadata "$repository_root"
    production_secret_sources assert_secret "$repository_root"
    printf 'Prepared production secrets as root:%s with mode 0440.\n' \
        "$runtime_gid"
else
    production_secret_sources assert_secret "$repository_root"
    printf 'Production secrets satisfy root:%s with mode 0440.\n' \
        "$runtime_gid"
fi
