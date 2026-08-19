#!/bin/sh
set -eu

runtime_gid=${PEPEAUDIO_RUNTIME_GID:-10001}
consumer_user=${PEPEAUDIO_SECRET_CONSUMER_USER:?set PEPEAUDIO_SECRET_CONSUMER_USER}
consumer_entrypoint=${PEPEAUDIO_SECRET_CONSUMER_ENTRYPOINT:?set PEPEAUDIO_SECRET_CONSUMER_ENTRYPOINT}
drop_user=${PEPEAUDIO_SECRET_DROP_USER:-}
chown_directory=${PEPEAUDIO_SECRET_CHOWN_DIRECTORY:-}

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
if [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' 'The secret-group entrypoint must begin as root.' >&2
    exit 1
fi
if ! id "$consumer_user" >/dev/null 2>&1; then
    printf '%s\n' 'The configured secret consumer user does not exist.' >&2
    exit 1
fi
if [ ! -x "$consumer_entrypoint" ]; then
    printf '%s\n' 'The configured consumer entrypoint is not executable.' >&2
    exit 1
fi

if [ -n "$drop_user" ]; then
    if [ "$drop_user" != "$consumer_user" ]; then
        printf '%s\n' 'The drop user must match the secret consumer user.' >&2
        exit 1
    fi
    if [ -n "$chown_directory" ]; then
        if [ ! -d "$chown_directory" ] || [ -L "$chown_directory" ]; then
            printf '%s\n' 'The configured writable directory is unsafe.' >&2
            exit 1
        fi
        chown -R "$drop_user:$(id -gn "$drop_user")" -- "$chown_directory"
    fi

    # Valkey's upstream entrypoint clears supplementary groups. Drop privilege
    # here instead and carry exactly the secret-reader GID into the process.
    exec setpriv \
        --reuid="$(id -u "$drop_user")" \
        --regid="$(id -g "$drop_user")" \
        --groups="$runtime_gid" \
        -- "$consumer_entrypoint" "$@"
fi

# PostgreSQL's upstream entrypoint uses gosu, which rebuilds supplementary
# groups from /etc/group. Materialize the Docker group_add contract there so it
# remains present after the entrypoint drops from root to postgres.
group_record=$(getent group "$runtime_gid" || true)
if [ -n "$group_record" ]; then
    runtime_group=${group_record%%:*}
else
    runtime_group=pepeaudio-runtime-secrets
    if getent group "$runtime_group" >/dev/null 2>&1; then
        printf '%s\n' 'The runtime secret group name already has another GID.' >&2
        exit 1
    fi
    groupadd --system --gid "$runtime_gid" "$runtime_group"
fi
usermod --append --groups "$runtime_group" "$consumer_user"
case " $(id -G "$consumer_user") " in
    *" $runtime_gid "*)
        ;;
    *)
        printf '%s\n' 'Failed to retain the runtime secret group.' >&2
        exit 1
        ;;
esac

exec "$consumer_entrypoint" "$@"
