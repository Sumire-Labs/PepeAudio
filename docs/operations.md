# PepeAudio-rs 運用・検証 Runbook

この文書は Windows 11 の開発検証、Ubuntu Server 26.04 LTS の本番ホスト準備、Docker Compose、
backup/restore、HRIR 導入、Discord staging 受け入れを扱う。コマンドは repository root で実行する。

> [!CAUTION]
> CI とローカル smoke は、使い捨ての OAuth client secret と Discord に接続しない
> placeholder Bot token を使う。これらは Compose、production API 起動、依存serviceを
> 検査するためだけの値であり、本番稼働の証拠ではない。development header auth、dummy
> credential、in-memory backend を外部公開して受入試験を省略してはいけない。

## 1. 固定 toolchain と検証入口

| 対象 | 基準 |
|---|---|
| Rust | `rust-toolchain.toml` の 1.97.0、rustfmt、Clippy |
| Web | Node.js 24.x（image は 24.19.0）、pnpm 11.3.0 |
| 本番 OS | Ubuntu Server 26.04 LTS |
| Windows container | Docker Desktop の WSL 2 backend、Linux containers |
| Data | PostgreSQL 18、Valkey 9 |

検証スクリプトは secret の値を表示しない。通常実行は unit test と build、および Compose model の検証までで、コンテナは起動しない。

### Windows 11 native

PowerShell 7、Git、Rustup、Node.js 24、Corepack/pnpm、Docker Desktop を用意する。

```powershell
rustup toolchain install 1.97.0 --profile minimal --component rustfmt,clippy
corepack enable
corepack prepare pnpm@11.3.0 --activate
.\scripts\verify.ps1
```

Docker daemon が動作している場合だけ、隔離した Compose project で live test を行う。既存の service/volume は操作せず、project 名に検証 process ID を含める。

```powershell
.\scripts\verify.ps1 -WithDockerIntegration
```

失敗調査用に残す場合だけ `-KeepDockerServices` を追加する。表示された project 名を
確認し、調査後にその project だけを `docker compose --project-name <name> down
--volumes --remove-orphans` で削除する。Docker 未導入端末では
`-SkipDockerConfig` を明示できるが、release 判定には使用しない。

### Ubuntu/Linux native

```sh
sudo apt update
sudo apt install --yes cmake ffmpeg libssl-dev pkg-config
rustup toolchain install 1.97.0 --profile minimal \
  --component rustfmt,clippy
corepack enable
corepack prepare pnpm@11.3.0 --activate
sh scripts/verify.sh
sh scripts/verify.sh --with-docker-integration
```

Linux 側の対応 option は `--keep-docker-services` と `--skip-docker-config`。live test は migration を実行した disposable な接続先だけで行う。
任意の共有 database を `PEPEAUDIO_TEST_DATABASE_URL` に渡して ignored test を直接実行してはいけない。

両 script が順に確認するものは以下。

1. Rust format、workspace全target test、全feature Clippy `-D warnings`
2. installed FFmpeg/ffprobeによるfixture生成・probe・実PCM decode/reap smoke
3. pnpm frozen install、TypeScript check、Vitest、Vite production build
4. first-party MIT metadata、vendored MPL境界、配布Web依存license台帳
5. base、Discord development、production全体のCompose展開と権限分離assertion
6. option指定時のみ PostgreSQL/Valkey health、one-shot migration、storage/authの
   ignored live test、production OAuth APIのhealth/login-start smoke

ライセンス検査だけを再実行する場合は、Web依存をfrozen lockfileからinstallした後に
Windowsでは`.\scripts\verify-licenses.ps1`、Linuxでは
`sh scripts/verify-licenses.sh`を使う。新しい依存やlicenseが検出された場合、互換性を
自動推測して許可せず、配布物を人手で確認してから承認済み台帳を更新する。

## 2. Ubuntu Server 26.04 ホスト準備

Docker は [公式 Ubuntu repository 手順](https://docs.docker.com/engine/install/ubuntu/) を使う。convenience script は本番 provisioning に使わない。

```sh
sudo apt update
sudo apt install --yes ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
  -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc

. /etc/os-release
arch=$(dpkg --print-architecture)
codename=${UBUNTU_CODENAME:-$VERSION_CODENAME}
printf '%s\n' \
  'Types: deb' \
  'URIs: https://download.docker.com/linux/ubuntu' \
  "Suites: $codename" \
  'Components: stable' \
  "Architectures: $arch" \
  'Signed-By: /etc/apt/keyrings/docker.asc' |
  sudo tee /etc/apt/sources.list.d/docker.sources >/dev/null

sudo apt update
sudo apt install --yes docker-ce docker-ce-cli containerd.io \
  docker-buildx-plugin docker-compose-plugin
sudo systemctl enable --now docker
sudo docker run --rm hello-world
```

`docker` group は root 相当の権限を持つ。運用 user を追加する場合はその前提で監査し、そうでなければ `sudo docker ...` を一貫して使う。
release を `/opt/pepeaudio/releases/<version>` へ配置し、owner と writable user を限定する。`secrets/` と backup は repository 外の暗号化された保管にも複製する。

公開 port は原則 Caddy の 80/TCP、443/TCP、443/UDP と管理用 SSH だけにする。PostgreSQL、Valkey、API の host port は公開しない。
Docker が UFW/firewalld の規則を迂回し得るため、公開 mapping と `DOCKER-USER` chain を導入時に監査する。

### GitHub Release と GHCR image

`.github/workflows/release.yml`は`vMAJOR.MINOR.PATCH`形式のtag pushだけを公開起点にする。
prereleaseは`v0.2.0-rc.1`のように付ける。tagのversionは全Cargo workspace packageと
`web/package.json`に一致しなければならない。

公開前にlocalで同じ契約を確認する。

```sh
node scripts/verify-release-tag.mjs v0.1.0
sh scripts/verify.sh
```

versionを更新し、検証が完了したcommitへ署名付きtagを作る。次の操作はreleaseを実際に
開始するため、review済みのcommitだけで行う。

```sh
git tag -s v0.1.0 -m "PepeAudio v0.1.0"
git push origin v0.1.0
```

workflowはRust/Web test、license inventory、dependency advisoryを再検査してから、
`linux/amd64`と`linux/arm64`向けに次のimageをpublishする。

```text
ghcr.io/<owner>/pepeaudio-bot:0.1.0
ghcr.io/<owner>/pepeaudio-api:0.1.0
ghcr.io/<owner>/pepeaudio-migrate:0.1.0
ghcr.io/<owner>/pepeaudio-caddy:0.1.0
```

同じ内容へGit tag付きtag、完全version、major.minor、commit SHAでも参照できる。安定版だけが
`latest`を更新し、0.xでは誤解を避けるためmajor-only tagを作らない。GitHub Releaseは
4 imageのpublishとattestationがすべて成功した後に作成され、digest固定参照を
`container-images.txt`として添付する。

GHCRの新規packageは既定でprivateである。匿名の`docker pull`を許可する場合、最初の
publish後にGitHubの各Package settingsから4 packageを個別に`Public`へ変更する。この変更は
不可逆なので、source label、license、秘密情報がimage layerにないことを先に確認する。

```sh
docker pull ghcr.io/<owner>/pepeaudio-bot:0.1.0
docker pull ghcr.io/<owner>/pepeaudio-api:0.1.0
docker pull ghcr.io/<owner>/pepeaudio-migrate:0.1.0
docker pull ghcr.io/<owner>/pepeaudio-caddy:0.1.0
```

privateのまま使う場合はread権限を持つtokenで`docker login ghcr.io`してからpullする。
Composeには次のimage変数を渡す。`docker compose pull`の後、deploy時に`--build`を付けない。

```sh
export PEPEAUDIO_BOT_IMAGE=ghcr.io/<owner>/pepeaudio-bot:0.1.0
export PEPEAUDIO_API_IMAGE=ghcr.io/<owner>/pepeaudio-api:0.1.0
export PEPEAUDIO_MIGRATE_IMAGE=ghcr.io/<owner>/pepeaudio-migrate:0.1.0
export PEPEAUDIO_CADDY_IMAGE=ghcr.io/<owner>/pepeaudio-caddy:0.1.0
```

GitHub CLIで署名元をrepositoryまで絞ってprovenanceを検証できる。private packageの検証前は
registryへloginする。

```sh
gh attestation verify \
  oci://ghcr.io/<owner>/pepeaudio-bot:0.1.0 \
  --repo <owner>/<repository>
```

複数imageのpublishはregistry上で一つのatomic transactionにはならない。途中失敗時は
GitHub Releaseを作らずworkflowが停止するが、先に完了したimageが残る場合がある。同じtagを
移動せずworkflowを再実行し、4つのdigestとRelease添付を照合する。repository側ではtag保護と
immutable releaseを有効にする。

## 3. Secret の準備

Compose secret は service ごとに `/run/secrets/...` へ mount されるが、host 上の source file自体を暗号化する仕組みではない。
詳細は [Docker Compose secrets](https://docs.docker.com/compose/how-tos/use-secrets/) を参照する。

最低限、次の file を `secrets/` に用意する。

| file | 用途 |
|---|---|
| `postgres_superuser_password.txt` | 初期化、backup/restore |
| `postgres_migrator_password.txt` | schema owner/migration |
| `postgres_runtime_password.txt` | runtime最小権限 role |
| `database_migrator_url.txt` | one-shot migration接続 |
| `database_runtime_url.txt` | API/Bot runtime接続 |
| `valkey_password.txt` / `valkey_url.txt` | Valkey ACL/接続 |
| `discord_token.txt` | Botだけへ付与 |
| `discord_client_secret.txt` | confidential OAuthを行うAPIだけへ付与 |
| `component_signing_key.txt` | component `custom_id`署名 |

Web sessionはrandomなopaque cookieのhashだけをValkeyへ保存し、署名鍵fileは使わない。
開発用 infrastructure secret とcomponent signing keyは、既存値を上書きしないhelperで
生成できる。Discord Bot tokenとOAuth client secretは生成しない。

```powershell
.\scripts\init-dev-secrets.ps1
```

```sh
umask 077
sh scripts/init-dev-secrets.sh
chmod 600 secrets/*.txt
```

本番では secret manager/offline generator から同名fileをmaterializeする。repository外の
pathを使う場合は各Compose secretの`PEPEAUDIO_*_SOURCE`へhost pathを設定する。この環境
変数にsecret値そのものを入れない。tokenをshell引数、`.env`、image layer、CI artifact、
logへ出さない。

本番の正本はUbuntu Server 26.04上のrootful Docker Engineで、Rust runtime imageは
`10001:10001`として動作する。Composeのlocal file secretはhost sourceをbind mountするため、
source fileを`root:${PEPEAUDIO_RUNTIME_GID:-10001}`、mode `0440`にする。`0644`へ緩和しては
ならない。全secretをmaterializeし、repository外pathを使う場合はComposeと同じ
`PEPEAUDIO_*_SOURCE`を設定してから、次を実行する。

```sh
export PEPEAUDIO_RUNTIME_GID=10001
sudo sh scripts/prepare-production-secrets.sh
sudo sh scripts/prepare-production-secrets.sh --check

# productionで使う全imageを用意した後、全consumerの実UID/GIDとreadを検査する
sh scripts/smoke-production-secret-read.sh
```

準備scriptはsecret値を表示せず、symlink、欠落、空file、owner/group/modeの不一致を
fail closedにする。外部source pathの環境変数を`sudo`が破棄する構成では、必要な変数だけを
明示的に`sudo`へ渡す。Composeはsecret consumerだけにsupplementary GIDを付与する。
PostgreSQLは`gosu`後にもgroup membershipを再構成し、Valkeyはrootでsourceを読むのではなく
UID `999`へ降格してGIDだけを保持する。container smokeはPostgreSQL、Valkey、migration、API、
Botそれぞれについて実UID、期待GID、実read、write不可を検査し、同じhost sourceを追加group
なしのUID `65534`と非member GIDへ直接bindした場合はreadできないことも検査する。rootless Docker、
`userns-remap`、Docker Desktop等はownership mappingが異なるためこの契約の保証外であり、
別のownership設計と同等のcontainer smokeなしに本番へ流用しない。Windows 11での結果も
Ubuntu Server 26.04のfile ownershipを証明しない。

直接値と対応するcontainer内の `*_FILE` を同時に設定せず、credential rotation は staging
で再認証とrollbackを確認してから行う。CIのrandom credentialやnumeric dummy Discord
IDは本番へ流用しない。APIへBot token/component signing keyを、BotへOAuth secretを
mountしてはいけない。この境界は`scripts/assert-compose-model.mjs`でも検査する。

### YouTube、SoundCloud、Spotify、Apple Music

`pepeaudio-bot` imageだけがPython、yt-dlp 2026.06.09、Deno 2.8.1を含む。APIと
migration imageには入れない。起動時にyt-dlp/Denoの実行可能性と最低versionを確認し、
不一致なら最初の`/play`まで遅延せずBot起動を失敗させる。artifact、SHA-256、license
noticeは[第三者ソフトウェア台帳](third-party.md)を正本にする。

site extractorは利用規約、著作権、地域法、ホスティング事業者のpolicyをoperatorが確認して
から明示的に有効化する。公開launch前にはstaging専用Botで対象serviceのpublic URLを使い、
取得権限、rate limit、削除依頼、log/retention、egress監視を含むpolicy reviewを記録する。
cookie、browser profile、netrc、user config、plugin、remote component、DRM回避は使わない。

YouTube／SoundCloudだけを使う場合は`.env`で次を設定する。

```text
PEPEAUDIO_ENABLE_SITE_EXTRACTORS=true
PEPEAUDIO_MAX_SITE_MEDIA_BYTES=104857600
PEPEAUDIO_MAX_PLAYLIST_ITEMS=25
```

Spotify／Apple Music照合はさらに独立したdefault-off switchとprovider credentialが必要。
default Composeはcredential fileを要求もmountもしない。使うproviderの明示overlayだけを
追加し、secret値は`.env`ではなくmode `0440`のsource fileへ置く。

`PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING=true`は技術的な有効化フラグであり、
providerが公開利用を許諾したことを意味しない。Spotify／Apple Music由来のmetadataを
別serviceの音源照合に使う構成は、providerの利用条件、attribution／branding要件、
対象アカウントの権限を公開前に別途確認し、承認記録を残す。それまでは本機能を
stagingのcontrolled evaluationに限定し、productionでは`false`を維持する。

```sh
# Spotify track/album metadata -> YouTube first, SoundCloud fallback
export PEPEAUDIO_SPOTIFY_CLIENT_ID='<client-id>'
export PEPEAUDIO_SPOTIFY_CLIENT_SECRET_SOURCE=/srv/pepeaudio/secrets/spotify-client-secret
docker compose \
  -f compose.yaml -f compose.discord.yaml \
  -f compose.catalog.spotify.yaml -f compose.production.yaml \
  --profile production config --quiet

# Apple Music catalog metadata, including catalog playlists
export PEPEAUDIO_APPLE_MUSIC_TEAM_ID='<team-id>'
export PEPEAUDIO_APPLE_MUSIC_KEY_ID='<key-id>'
export PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_SOURCE=/srv/pepeaudio/secrets/AuthKey.p8
docker compose \
  -f compose.yaml -f compose.discord.yaml \
  -f compose.catalog.apple.yaml -f compose.production.yaml \
  --profile production config --quiet
```

両providerを使う場合は二つのcatalog overlayを同時に指定する。Spotify Client Credentialsは
2026年時点のAPI境界上、任意のpublic playlist itemを取得できないため、Spotifyはtrackと
albumだけを対応範囲とする。Spotify playlistは専用errorで拒否し、ユーザーOAuthやrefresh
tokenを追加して回避しない。Apple MusicもDRM音源を取得せずcatalog metadataだけを使う。

site／catalog playlistはqueue空き、operator上限、hard maximum 100の最小値までを処理し、
最大4件を並列取得する。候補audioのbyteは必ずSSRF/DNS pinningとmanaged quotaを通る。
batchは5分の絶対deadlineを持ち、成功分を一括enqueueする前にfatal errorがあれば全成功objectを
破棄する。download commit後からffprobe完了／batch cleanup登録までの間にtaskがcancelされた
場合も、そのobjectはhard quotaの使用量として残り、追加downloadを上限で拒否する。objectが
5分の最小保持期間を越えた後、容量不足時のserialized
on-demand cleanupまたは15分周期のjanitorが回収する。通常のobject TTLは7日、staging
partial TTLは1時間である。signed CDN URLはsnapshot/logへ出さない。

Ubuntuでは各yt-dlp／Deno／ffprobe processを新しいprocess groupで起動し、timeout時はgroup
全体へ`SIGKILL`を送り、pipe drainも2秒で打ち切る。これによりtoolが生成した同一groupの
descendantがstdout/stderrを保持してもpermitを無期限に占有しない。Windowsのtest runtimeは
job objectをまだ使わないため、親processのkillとbounded pipe drainまでは保証するが、別process
groupへ離脱したdescendantの完全回収は保証しない。production契約はUbuntu containerである。

通常CIはfake fixtureだけをrelease gateにする。外部serviceの変化を確認するlive smokeは、
operatorが対象URLへの権利と各serviceの利用条件を確認した場合だけ、manual workflowまたは次の
ignored testを明示的に実行する。定期的な自動accessは行わない。結果はproviderの一時障害・
bot判定でも失敗し得るため、Rust品質gateと混同しない。

```sh
PEPEAUDIO_YTDLP_PATH=/usr/local/bin/yt-dlp \
PEPEAUDIO_DENO_PATH=/usr/local/bin/deno \
PEPEAUDIO_DENO_DIR=/tmp/pepeaudio-deno \
PEPEAUDIO_YOUTUBE_SMOKE_URL='https://www.youtube.com/watch?v=<authorized-id>' \
PEPEAUDIO_SOUNDCLOUD_SMOKE_URL='https://soundcloud.com/<artist>/<authorized-track>' \
cargo test -p pepeaudio-media --test ytdlp_live -- --ignored --nocapture
```

Discord guild一覧はlogin時のOAuth projectionであり、refresh tokenは保存しない。production
overlayはabsolute/idle session期限を30分にし、APIも30分をhard maximumとして拒否する。
これによりguild退出後に古いmembershipが残る時間を30分以内へ限定する。SSEはさらに短い
bounded connectionで定期的にsession認可を再確認する。期限を延ばす変更は、Discord
membership refreshを別途実装・検証し、このhard maximumを意図的に見直してから行う。
旧releaseが作成した長いsession recordも、更新後の最初の認可時に現行policyへclampし、
既に30分を超えていればsessionとcurrent-user pointerを失効させる。

認証入口はclient IPや`X-Forwarded-For`を信頼せず、同じValkey keyspace全体で未消費の
OAuth stateを最大4096件に制限する。予約時に期限切れmemberを原子的に除去し、上限時は
`429 Retry-After: 60`で新しいstate cookieを発行しない。これにより複数API replicaでも
pending stateのmemory使用量は有界になる。Caddyで追加のIP別rate limitを導入する場合は、
public listenerへ直接到達できるproxyだけが転送headerを設定する構成と、信頼するproxy
hopを明示的に固定してから行う。任意のclient-supplied forwarded headerを正本にしない。

SSEはAPI replicaごとに全体1024接続、1ユーザー8接続までとし、response bodyの終了・破棄で
admission leaseを解放する。上限時は`429 Retry-After: 5`を返す。この上限はprocess-localな
防御であり、複数replica全体のsocket上限を保証しないため、Caddy／host側にも接続数とfile
descriptorの監視・上限を持たせる。Cookie認証を含むAPI／auth responseは
`Cache-Control: private, no-store`とし、共有cacheへsession依存responseを保存させない。

### Web command admission

認証済みWeb player commandは、sessionから得た`actor_user_id`と対象guildを主体にし、
1分の固定窓で1ユーザー・1guildあたり20件、guild全体で60件まで受け付ける。
client IP、`X-Forwarded-For`等の転送headerはこの判定に使用しない。productionではValkeyの
`TIME`を時刻正本とし、全API replicaが同じcounterを共有する。development in-memory
backendも同じ閾値とHTTP契約を再現するが、counterはprocess-localであり複数process間では
共有されない。

stream容量、既存Pending、対象key型、両counterの確認と、counter更新、Pending result作成、
streamへの`XADD`は一つのLua scriptで処理する。拒否時はcounter、Pending result、streamを
一切変更しない。同じ`command_id`のPending再送もquotaを消費せず、streamへ重複追加しない。
HTTP retryは同じidempotency keyでも新しい`command_id`を持つ別のadmission attemptであり、
二重適用はowner-side idempotencyで防ぐ一方、quotaには計上する。
上限時は`429`、code `player_command_rate_limited`、Valkey時刻から算出した正確な
`Retry-After: 1..60`を返す。errorとlogへactor ID、guild ID、Valkey keyを含めない。

counter keyは固定窓の終端で失効する。guild quotaに到達してから新しいactor keyを作らないため、
1guild・1窓のrate keyはguild用1件とactor用最大60件で有界になる。固定窓境界をまたぐ
最大burstを含め、1guildが任意の24時間に新規受付できるcommandは最大86,460件である。
terminal completion時に24時間retentionが更新されるため、result／dedupeの容量見積りには
この定常上限に加え、過去から遅延している対象shardのstream backlog（shard全体で最大100,000
entry）も含める。shard mapping変更時は旧streamを別途棚卸しする。監視では
`player_command_rate_limited`の発生率、Valkey memory、`cmd-result:*`／`processed:*`のkey数、
shard stream長を確認する。通常操作で制限が継続する場合、先にWeb clientの重複送信、
stale revision retry、複数tabを調査し、memory見積りなしに閾値や24時間retentionを
引き上げない。

## 4. Compose 展開、migration、停止

Compose 2.24.4以上を使用する。production overlayは`!override`でdevelopment設定を
除去するため、必ずbase、Discord、productionの順で指定する。単独では使用しない。
展開内容とsecret pathを起動前に確認する。`docker compose config` の出力を共有する
場合は、将来environment secretが増えても漏えいしないよう`--quiet`を使う。

```sh
sh scripts/verify-compose.sh
sudo sh scripts/prepare-production-secrets.sh --check

export PEPEAUDIO_DOMAIN='audio.example.com'
export PEPEAUDIO_DISCORD_CLIENT_ID='<oauth-client-id>'
export PEPEAUDIO_VALKEY_KEYSPACE='pepeaudio-production'
docker compose -f compose.yaml -f compose.discord.yaml \
  -f compose.production.yaml --profile production config --quiet
```

`PEPEAUDIO_DOMAIN`はDNSでこのhostへ向け、Developer Portalへ
`https://<domain>/auth/callback`を完全一致で登録する。APIとBotの
`PEPEAUDIO_VALKEY_KEYSPACE`、`PEPEAUDIO_SHARD_TOTAL`は同じ値にする。Bot processを
1つだけ起動する場合、`PEPEAUDIO_SHARD_END_EXCLUSIVE`を未設定にすると検証済みの
`PEPEAUDIO_SHARD_TOTAL`が終端として使われる。Bot processを複数起動する場合は終端を
Compose overrideまたはorchestratorで明示し、各`SHARD_START..SHARD_END_EXCLUSIVE`を
重複させず、和集合を
`0..SHARD_TOTAL`にし、`PEPEAUDIO_INSTANCE_ID`をprocessごとに安定かつ一意にする。managed
mediaは共有upload volume内の`<INSTANCE_ID>/staging`と`<INSTANCE_ID>/objects`へ隔離され、
process-local leaseとjanitorが他instanceのfileを操作しない。MVPには同一shardのowner epoch
fencingがないため、同じrange／instance IDの旧・新Botを同時に動かすrolling overlapは禁止する。
該当rangeはstop-before-startで更新する。

`PEPEAUDIO_MAX_MANAGED_MEDIA_BYTES`（既定10 GiB）はBot instanceごとのhard上限であり、
`PEPEAUDIO_MAX_UPLOAD_BYTES`以上にする。既存object、crash後のstaging partial、同時downloadの
予約を合算する。Botは起動時にmanaged directoryを全件安全検査し、未知name、link／reparse、
metadata検査失敗、または`max_entries_per_scan`（既定4096）を超える状態では会計を推測せず
起動を失敗させる。容量不足時はURL解析・DNS・HTTPより前に要求を拒否し、内部logにはpathを
含めず`used_bytes`、`reserved_bytes`、`maximum_bytes`とentry数だけを記録する。容量を増やす前に
host volumeの実空き容量とbackup領域を確認する。
current／queue／repeat history／audio pipelineのobjectはopaque leaseが保護するため、最大queue
滞在時間をretentionへ加算しない。leaseがなくなったobjectは5分後から容量回収対象、通常TTLは
7日である。定期Janitorは15分間隔で、容量予約失敗時には直列化されたadmission cleanupを1回
実行してから予約を一度だけ再試行する。それでも不足する場合はnetworkへ接続せずgenericな
容量超過として拒否する。

初回またはschema更新時は、runtime起動より先に一度だけmigrationを実行する。

```sh
compose() {
  docker compose -f compose.yaml -f compose.discord.yaml \
    -f compose.production.yaml --profile production "$@"
}
compose pull postgres valkey
compose build migrate api bot caddy
sh scripts/smoke-production-secret-read.sh
compose up --detach --wait postgres valkey
compose up --detach --wait api caddy
compose up --detach bot
compose ps
```

migrationはreplica起動ごとではなく、backup後に専用roleのone-shot `migrate` serviceで適用する。
`api`の`service_completed_successfully`依存により、`compose up --detach --wait api caddy`が
migration完了を待つため、同じrolloutで別途`compose run migrate`を重ねない。非0で終了したら
API/Bot rolloutを止める。まずAPI/Caddyの`ready`とOAuth login開始を確認し、
最後にBotを起動する。BotはDiscord Gatewayへ接続するため、CI placeholder tokenで
起動しない。production Caddyだけが80/TCP、443/TCP、443/UDPを公開し、development
profileの`127.0.0.1:8080`はproduction modelから除去される。

Bot processはDiscord Gateway、shard command worker、guild presence actor、managed media
janitor、Discord status updaterを一つのsupervisorで監視する。worker、presence、janitor、
status updaterが正常・error・panicの
いずれで予期せず終了してもGatewayを停止してprocessを非0終了させ、
`restart: unless-stopped`へ回復を委ねる。
Discordのcustom statusにはBot process自身のresident memoryを60秒ごとに表示する。
LinuxではRSS、Windowsでは対応するworking set相当の値であり、別process／containerの
PostgreSQLとValkeyは含まない。複数Bot processでshardを分担する場合は、各guildに接続する
shard process自身の値となる。tick遅延時は古い更新をまとめて送らず、最新値を一度だけ送る。
BotのCompose healthcheckは、この契約下のPID 1が生存していることだけを検査するlivenessで
あり、Discord Ready、Voice接続、音声品質を証明するreadinessではない。これらはstagingの
実Discord受入で別に確認する。

SIGTERM受信後はDiscord Gatewayの停止と全cleanup phaseで一つの38秒deadlineを共有する。
command ingressを閉じた後、独立したpresence lease、media janitor、guild playerを並列に停止し、
最後のsnapshotとsettings writeも並列にflushする。Composeの45秒猶予との差7秒はscheduler遅延、
log flush、Tokio runtime teardown用である。依存先が応答せずdeadlineへ達した場合、残存workerを
cancelして非0終了するため、正常なgraceful shutdownとして扱わない。

API processもValkey snapshot subscription taskをHTTP serverと同時に監視する。taskが
予期せずreturnまたはpanicした場合はgraceful HTTP shutdown後に非0終了し、古いSSE状態を
配信し続けない。通常のValkey切断はtask内部で再接続し、その間`/health/ready`を失敗させる。

image registryを使う場合はdigestを固定し、次の変数でComposeのimage参照を上書きする。
`PEPEAUDIO_MIGRATE_IMAGE`、`PEPEAUDIO_API_IMAGE`、`PEPEAUDIO_BOT_IMAGE`、
`PEPEAUDIO_CADDY_IMAGE`。`build`を残しているため、ローカルでは`:local` imageも作成できる。

停止は次を使い分ける。

```sh
# serviceを停止するがvolumeは保持
compose stop
# container/networkを削除するがnamed volumeは保持
compose down --remove-orphans
```

本番で `down --volumes`、`docker system prune --volumes` を実行してはいけない。upgrade時は新image digest、migration結果、health/readiness、rollback可能なbackupを記録する。
Bot shard rangeは重複なし、和集合が `0..SHARD_TOTAL` になることを確認する。

## 5. PostgreSQL backup / restore drill

backup先を先に作り、信頼できる自組織のdumpだけをrestoreする。PostgreSQLのcustom archiveは `pg_dump` と `pg_restore` の組で扱える。
詳細は [PostgreSQL 18 pg_dump](https://www.postgresql.org/docs/18/app-pgdump.html) を参照する。

```sh
backup_dir=/srv/backups/pepeaudio
stamp=$(date -u +%Y%m%dT%H%M%SZ)
sudo install -d -m 0700 "$backup_dir"

sudo docker compose exec -T postgres sh -euc '
  export PGPASSWORD="$(cat /run/secrets/postgres_superuser_password)"
  pg_dump -U postgres -d pepeaudio -Fc -f /tmp/pepeaudio.dump
'
sudo docker compose cp postgres:/tmp/pepeaudio.dump \
  "$backup_dir/postgres-$stamp.dump"
sudo docker compose exec -T postgres rm -f /tmp/pepeaudio.dump
sudo sha256sum "$backup_dir/postgres-$stamp.dump" |
  sudo tee "$backup_dir/postgres-$stamp.dump.sha256" >/dev/null
```

production databaseを上書きせず、固定名の隔離databaseへ復元して検査する。

```sh
dump=/srv/backups/pepeaudio/postgres-YYYYMMDDTHHMMSSZ.dump
sudo sha256sum --check "$dump.sha256"
drill_id=$(date -u +%Y%m%d%H%M%S)
restore_db="pepeaudio_restore_$drill_id"
restore_dump="/tmp/restore-$drill_id.dump"
sudo docker compose cp "$dump" "postgres:$restore_dump"
sudo docker compose exec -T \
  -e RESTORE_DB="$restore_db" -e RESTORE_DUMP="$restore_dump" \
  postgres sh -euc '
  export PGPASSWORD="$(cat /run/secrets/postgres_superuser_password)"
  case "$RESTORE_DB" in pepeaudio_restore_[0-9]*) ;; *) exit 2 ;; esac
  created=0
  cleanup() {
    if [ "$created" -eq 1 ]; then
      dropdb -U postgres --if-exists "$RESTORE_DB"
    fi
    rm -f "$RESTORE_DUMP"
  }
  trap cleanup EXIT
  trap "exit 129" HUP
  trap "exit 130" INT
  trap "exit 143" TERM
  test -z "$(psql -U postgres -d postgres -Atc \
    "SELECT 1 FROM pg_database WHERE datname = '\''$RESTORE_DB'\''")"
  createdb -U postgres --template=template0 "$RESTORE_DB"
  created=1
  pg_restore -U postgres -d "$RESTORE_DB" \
    --exit-on-error --no-owner --no-privileges "$RESTORE_DUMP"
  psql -U postgres -d "$RESTORE_DB" \
    -v ON_ERROR_STOP=1 -c "SELECT count(*) FROM guild_settings"
'
```

restore日時、checksum、row/table確認、所要時間を記録する。RPO/RTO、off-host保管、retention、暗号鍵管理はproduction rollout前に決定する。
`uploads` と `hrir_data` はdatabaseとは別にversion付きでbackup/restoreする。PostgreSQLにHRIR WAVを入れない。
scratch volumeへ展開してfile数、checksum、loader検査を行い、production volumeへ直接restoreしない。
実volume名は先に `docker volume inspect pepeaudio_uploads pepeaudio_hrir_data` で確認する。

ValkeyはAOF + `noeviction` だが永続設定の正本ではない。停止整合点を取ったvolume backupと、session/command喪失時の再ログイン・再配送手順を別途試験する。
稼働中volumeを `tar` で直接読まず、Valkeyの停止または正式なsnapshot手順を使う。
shard command Streamは100,000件を原子的なbackpressure上限とし、到達時はAPIが操作を
受理しない。古い未処理commandを自動trimしない。ACKと削除も同一Lua操作なので、通常処理済み
entryは残留しない。backlog件数と最古pending ageを監視し、上限到達前にBot workerを復旧する。

## 6. HRIR catalog runbook

Botは起動時に`PEPEAUDIO_HRIR_DIRECTORY`直下の`.wav`だけを列挙し、symlink、unsafeな
path、上限超過、無効な7/14ch HeSuVi WAV、危険なDSP係数が一つでもあればcatalog構築を
失敗させる。filename stemが安定したpreset ID/表示名になる。catalogはprocess lifetime中
immutableなので、追加・削除後はBotをrestartする。動的upload/import APIは提供しない。
productionは入力FFT履歴をsourceごとに共有するuniform-partitioned convolutionを使用し、
Windows 11 reference hostのrelease throughput測定に基づきIRを最大9,600 frames（48 kHzで
200 ms）へ制限する。10秒・960-frame blockの単一orbit測定は4,800 tapsで434.5x、9,600 taps
で248.9x、19,200 tapsで123.7x realtimeだった。preset crossfadeは一時的に二つのrendererを
処理するため、この単一guild測定だけで同時wet guild数を保証しない。44.1 kHz sourceは48 kHz
への160/147 resample後にも9,600-frame上限を再検査し、source frame数だけで上限をすり抜けない。

候補fileは本番volumeへ入れる前に個別検証する。

```sh
docker build -f deploy/rust/Dockerfile \
  --build-arg PEPEAUDIO_BINARY=pepeaudio-hrir-check \
  -t pepeaudio-hrir-check:local .
docker run --rm --read-only --cap-drop ALL \
  --security-opt no-new-privileges \
  --mount type=bind,src=/srv/pepeaudio/hrir-staging,dst=/input,readonly \
  pepeaudio-hrir-check:local /input/preset.wav
```

受け入れ条件は以下。

1. 配布・利用license、attribution、原典を人手で承認する。
2. quarantineでsize上限とSHA-256を確定し、安全で一意なfilename/preset IDを割り当てる。
3. 7chまたは14ch、44.1/48 kHz、PCM16またはf32、有限sample、frame上限をloaderで検査。
4. WAVは`hrir_data`へ置き、Botにはread-only mountする。手動SQL insertは行わない。
5. channel mapping fixture、impulse response、NaN、peak/gain、切替crossfadeを検査する。
6. stagingで全listener共通の水平7方向処理として聴感確認し、head trackingや高さ付き
   3Dと表現しない。OS/ヘッドセット側の空間音響を無効にして比較する。
7. asset checksumとdeployment inventoryを揃えてからactiveにし、不一致時は旧presetを維持。

## 7. Discord staging 受け入れ

productionとは別のDiscord application、test guild、text/voice channelを使う。Bot
tokenをAPI/Webへ渡さず、OAuth secretをBotへ渡さない。Gateway shard総数は
[Discord Get Gateway Botとsharding規則](https://docs.discord.com/developers/events/gateway)
に基づき、同時Identify上限も守る。enqueue直前のrole／`Manage Guild`再確認を
現在のmember cacheから行うため、Developer PortalのBot設定でprivileged
**Server Members Intent**を有効にする。無効またはmember factsが欠落した場合、
操作は古いinteraction payloadへfallbackせずfail closedになる。

- command登録: `/play url`、`/play file`、`/now`、`/stop`、`/leave`
- 権限: VC未参加、別VC、DJ/Manage Guild不足を拒否し、内部情報を表示しない
- UI: `/now`がEmbed/content/accent colorなし、Components V2だけ、default色
- Player: enqueue、pause/resume、skip、stop、repeat、shuffle、volume、seek/queue
- Defaults: volume、HRIR、360°を変更後、5分idle終了またはBot再起動を挟んでも
  PostgreSQLから同じ値でPlayerを再生成する。DJ role／control policyを同時更新しても失わない
- Idle: current trackなし・queue空で300秒後、許容±5秒で退出。Pausedは暫定で除外
- Audio: join/leave、track end/error、再接続、DAVE handshake、全listener同一出力
- HRIR: preset切替、360° toggle、無効化fallback、NaN/level jump/underrunなし
- Web: OAuth、guild認可、初回snapshot、SSE更新、revision gap時resync、再接続
- Shard:公式式でrouting、range重複なし、command dedupe、owner停止/復帰、queue復元
- Failure: URL/添付不正、期限切れ、PostgreSQL/Valkey停止、rate limit、SIGTERM

runtimeのPostgreSQL poolは、connection取得5秒、statement 10秒、lock 5秒、
idle transaction 15秒でfail closedになる。stagingでは意図的なtable lockを使い、
1ギルドの設定読み書きが他guildのWeb command workerを無期限に止めないことも確認する。

各項目はclient/version、timestamp、guild/shard、期待値、結果、log/録音/画面証跡を残す。
音声品質、定位、latency、CPU、shard failoverはbuild成功から推測しない。

## 8. 現在の既知 fail-closed / 未検証境界

| 境界 | 現在の挙動 | rollout条件 |
|---|---|---|
| API production auth | 実装・container smokeあり | 実Discord OAuth callback、再認証、失効試験 |
| Bot assembly | production wiringのsource検証 | 実Discord stagingでGateway/Voice受入 |
| Songbird playback | decoder/pipelineの自動試験 | track end/error、DAVE、音切れの実機試験 |
| HRIR/360° | loader/DSP/pipelineの決定的試験 | 聴感、level、underrun、切替の実機試験 |
| URL/file media | SSRF/probe/quotaの自動試験 | public network、期限切れ添付、長時間soak |
| realtime Web | OAuth/Valkey/SSEの自動試験 | browser再接続、複数guild、shard停止E2E |
| HRIR catalog | 起動時read-only catalog実装 | asset license承認、運用restart手順の試験 |
| multi-process shard | routing/dedupe/presence primitive | lease/reclaim/dedupeのcrash/soak試験 |
| FFmpeg image配布 | executable存在とdecode smoke | build configuration、codec license、SBOM/CVE監査 |
| partitioned HRIR | 9,600-tap単一orbit測定・起動時上限 | target hostの多guild load／crossfade／underrun |
| shard ownership | range別routing・revision watermark | 同一range overlap禁止、将来owner epoch fencing |
| Web command結果 | 202後にrevisionを最大5秒確認 | owner rejection理由を返すresult channel |
| Ubuntu/Docker/Discord | CIで代替不可 | 26.04 hostと実Discord stagingの記録 |

2026-08-12 に Windows 11 + Docker Desktop で `development-api` profileの
PostgreSQL、Valkey、migration、API、Caddyがhealthyとなり、live/ready health、
SPA 200 + CSP、REST mutationのrevision/volume反映を確認した。SSEのunit/integration
testは成功したが、手動PowerShell streaming観測はclient timeoutのため完了していない。
これはdevelopment smokeであり、production OAuthやUbuntu/Discord/音響の証拠ではない。

2026-08-13 に同じWindows 11 hostでproduction Compose modelの権限分離assertion、
`migrate`/`api` Linux image build、隔離したPostgreSQL/Valkeyへのmigration、production APIの
live/ready、未認証session拒否、OAuth開始redirect/Secure cookieを確認した。PostgreSQL
repository、Valkey snapshot/stream/idempotency/presence、Valkey OAuth state/sessionのignored
live testも成功した。使い捨てprojectとvolumeは検証後に削除した。これはCaddyの実TLS、
実Discord OAuth callback、Bot Gateway/Voice、Ubuntu 26.04 host、音響品質の証拠ではない。

Web/Caddy imageの検証では、SPA shellの`Cache-Control: no-cache`、hash付き
`/assets/*`の一年immutable cache、CSP、MIT／第三者notice、production source mapが
存在しないことを`scripts/smoke-caddy-static.sh`で確認する。asset filenameはcontent hashを
含むため長期cacheできるが、`index.html`は新しいhashを参照できるよう毎回再検証させる。

上表の項目を環境変数やdummy adapterで握り潰さない。release判定は
「source/unit」「container integration」「Discord/音響E2E」を別々に記録する。
