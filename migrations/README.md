# PostgreSQL migration contract

`pepeaudio-migrate` embeds every SQLx migration at build time. Run that
binary as a one-shot deployment step before Bot/API readiness. Normal Bot and
API replicas must not apply DDL during startup.

The migration identity must own the schema. The runtime identity should receive
only `CONNECT`, `USAGE`, and the table/sequence privileges required by
repository operations. Do not grant `CREATE`, schema ownership, superuser, or
replication privileges to runtime services.

Accepted migration URL sources are exactly one of:

- `PEPEAUDIO_DATABASE_URL`
- `PEPEAUDIO_DATABASE_URL_FILE`
- `DATABASE_URL`
- `DATABASE_URL_FILE`

Direct and file-backed variants are mutually exclusive. Error output names the
variable but never prints the URL, file path, or credential.

The schema stores guild settings, HRIR metadata, playlist headers, and ordered
playlist tracks. HRIR WAV data, uploaded media, live queues, active playback
positions, web sessions, shard commands, and snapshots are deliberately stored
outside PostgreSQL.

Before production migration:

1. Back up PostgreSQL and verify the restore path.
2. Apply the one-shot migration with the schema-owner identity.
3. Start runtime identities only after migration success.
4. Check application readiness with runtime credentials.

The initial migration is forward-only. A rollback that would discard user data
must be an explicit operator procedure, not an automatic container restart.

For Valkey command delivery, configure persistence and `noeviction`; cache
eviction policy is not acceptable for Streams. Stream trimming and retention
must be selected from measured command volume and the maximum recovery window.
