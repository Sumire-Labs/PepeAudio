#!/bin/sh
set -eu

secret_file=${VALKEY_PASSWORD_FILE:-/run/secrets/valkey_password}
if [ ! -r "$secret_file" ]; then
    echo "Valkey password secret is unreadable" >&2
    exit 1
fi

password=$(sed -e 's/[[:space:]]*$//' "$secret_file")
if [ -z "$password" ]; then
    echo "Valkey password secret is empty" >&2
    exit 1
fi

password_hash=$(printf '%s' "$password" | sha256sum | cut -d ' ' -f 1)
umask 077
printf 'user default on #%s ~* &* +@all\n' "$password_hash" > /run/pepeaudio-users.acl
unset password password_hash

exec valkey-server \
    --aclfile /run/pepeaudio-users.acl \
    --appendonly yes \
    --appendfsync everysec \
    --maxmemory-policy noeviction \
    --protected-mode yes
