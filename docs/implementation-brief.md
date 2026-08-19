# PepeAudio-rs 実装ブリーフ

この文書は、最初の要望をそのまま開発へ渡せる粒度に補った統合プロンプトです。
「確定」はユーザー要求、「暫定」は安全に着手するための推奨初期値、「未決定」は製品判断が必要な項目です。
詳細な受け入れ条件は [製品要求仕様](product-requirements.md)、構成理由は [アーキテクチャ](architecture.md) を正本とします。

## 統合プロンプト

Rust で、HeSuVi 互換 HRIR プリセットを読み込み、バイノーラル再生と水平 360° Audio を提供する Discord 音楽 Bot「PepeAudio-rs」を開発する。
本番は Ubuntu Server 26.04 LTS、開発・テストは Windows 11 とし、Docker Compose を正式なビルド、テスト、配布、運用経路にする。

### 1. 技術基盤

- Rust 1.97.0、Edition 2024 の Cargo workspace とする。
- Discord は released stable の Serenity 0.12.5、Poise 0.6.2、Songbird 0.6.0 を基準にする。
- Songbird 0.6 系の DAVE 対応を維持し、古い Voice 実装へ downgrade しない。
- stable Serenity に不足する Components V2 の型は、最小の wire DTO / REST adapter crate に隔離する。Discord 固有 JSON を domain、audio、storage へ漏らさない。
- Web API は Axum、非同期 runtime は Tokio、永続化は PostgreSQL + SQLx、一時状態・session・command bus は Valkey を使う。
- Web は React + TypeScript + Vite の SPA とし、Caddy から same-origin で静的配信する。
- Browser からの変更操作は認証済み REST、server からの状態通知は SSE とする。
- unsafe は原則禁止し、依存 version と toolchain は lockfile / toolchain file で固定する。

### 2. Discord コマンド

次の guild-only application command を実装する。

```text
/play url url:<string>
/play file file:<attachment>
/now
/stop
/leave
```

`/play` は URL と attachment を同一 option union にせず、`url` と `file` の二つの subcommand にする。
実行者が Voice Channel にいない場合は状態を変更しない。
Bot が未接続なら実行者の VC へ参加し、同じ guild の別 VC にいる場合は初期仕様では自動移動せず拒否する。
入力の取得、形式検証、decoder probe が成功してから enqueue 成功を返す。

URL sourceは、HTTP/HTTPSの直接音声、Discord attachment、YouTube／SoundCloudの
page／playlistを扱う。site resolverはdefault-offで、cookieやuser configを使わず、
metadata/direct audio URLだけをyt-dlpで解決する。実byteは共通のSSRF、DNS pinning、
quota、managed storage境界を必ず通す。Spotify／Apple Musicはoperatorが別途有効にした
catalog metadataだけをYouTube優先／SoundCloud fallbackで照合する。Spotifyの配信音声取得、
Spotify playlistのuser OAuth、DRM回避、任意サイトdownloaderは要件に含めない。

`/now` は Embed と通常の message content を一切使わず、`IS_COMPONENTS_V2` を付けた Components V2 だけで構築する。
曲情報、状態、再生位置、総時間、VC、queue 数、repeat、shuffle、音量、HRIR、360° の状態を表示する。
操作は previous、pause/resume、skip、stop、repeat、shuffle、360° toggle、HRIR select、volume select を基本とする。

- Button は Discord 標準の neutral/secondary style を使う。
- Container の `accent_color` は送らない。
- Discord には slider component がないため、音量は 0、10、20、…、100% の String Select を暫定値とする。連続 slider は Web だけに置く。
- HRIR が 25 option を超える場合は category または page に分ける。
- Components V2 の全 component 数は 40 以下、Action Row は最大 5 button または select 一つ、`custom_id` は message 内で一意にする。
- interaction は 3 秒以内に defer し、その後 original response を Components V2 payload で edit する。
- interaction token の有効期限へ依存した常設更新はしない。永続的な `/now` panel は Bot token で message edit する。
- シーク表示を毎秒 Discord REST で更新しない。再生開始、pause、seek、track change 等の状態境界で更新し、時刻 anchor を表示する。

`/stop` は暫定的に「現在曲を停止し、通常 queue を空にし、VC 接続は維持する」とする。
その後 5 分間曲が追加されなければ自動退出する。保存 playlist は削除しない。

`/leave` は同じVCにいるconfigured DJ roleまたはManage Guild権限保持者だけに許可し、再生・queue・Player Actorを終了して即時退出する。

### 3. Player の状態と権限

guild ごとに一つの Player Actor を持ち、Discord command、component、Web API、Songbird event、timeout の全変更を Actor command として直列化する。
Voice connection、現在曲、queue、再生位置、音量、HRIR、360°、idle timer の正本は shard owner process 内の Actor とする。
PostgreSQL や Valkey の snapshot から Songbird handle を直接操作しない。

Player snapshot は単調増加 revision を持つ。
Web/Discord command は guild ID、actor、idempotency key、expected revision、deadline を持ち、古い UI、期限切れ、重複配送を安全に拒否する。

暫定権限は次とする。

- enqueue、pause/resume、skip、repeat、shuffle、volume: Bot と同じ VC にいるユーザー。
- stop、disconnect、guild default、HRIR import: DJ role または `Manage Guild` を持つユーザー。
- DJ role 未設定時に stop を同一 VC 全員へ許可するかは未決定。
- すべての component click と Web mutation で、その時点の guild membership、VC、role、permission を再検証する。

再生中でなく、現在曲がなく、queue が空で、VC に接続中の状態を `IdleConnected` とする。
この状態へ入った時点で monotonic clock の 300 秒 timer を開始する。
enqueue で generation を更新して timer を無効化し、callback 側でも generation、revision、現在状態を再確認してから退出する。
Paused は暫定的に idle とみなさない。

### 4. HeSuVi HRIR

HeSuVi WAV の初期対応範囲は次とする。

- 7 channel または 14 channel。
- 44,100 Hz または 48,000 Hz。
- PCM 16-bit または IEEE float 32-bit。
- 通常 WAVE と、reader が対応する `WAVE_FORMAT_EXTENSIBLE`。
- import 後の内部形式は 48 kHz planar `f32`。44.1 kHz は全 channel を同じ resampler で一度だけ変換し、相対 delay を保つ。
- sample が有限、data 長が channel 数で割り切れる、全 channel が同じ frame 数、IR 長と file size が上限内であることを検証する。
- MIME、拡張子、original filename を信頼せず、parser で本体を検証する。

14ch の HeSuVi track order は次の固定規則で、WAVE channel mask から推測しない。

```text
FL = [0, 1]
FR = [8, 7]
FC = [6, 13]
BL = [4, 5]
BR = [12, 11]
SL = [2, 3]
SR = [10, 9]
```

各組は常に `[left_ear, right_ear]` へ正規化する。
特に FR、SR、BR は file 内で right ear が先に来るため、swap を unit test で固定する。
7ch は左右対称の省略形式として、FL=`[f0,f1]`、FR=`[f1,f0]`、SL=`[f2,f3]`、SR=`[f3,f2]`、BL=`[f4,f5]`、BR=`[f5,f4]`、FC=`[f6,f6]` へ展開する。

プリセットごと・耳ごとの個別 normalize はしない。左右レベル差と相対 delay は定位情報なので保持し、必要な保護 gain は全 channel 共通に適用する。
プリセット変更、wet/bypass 変更は事前準備した DSP state 間を暫定 100–250 ms の等電力 crossfade で切り替え、click/pop を防ぐ。

HeSuVi プリセット本体は初期 Docker image へ同梱しない。
管理者が権利を持つ WAV を `/data/hrir` へ mount または管理 UI から import し、source、author、license、attribution、original URL、SHA-256、再配布可否を保存する。

### 5. 360° Audio の正確な意味

HeSuVi が持つのは FC 0°、FL/FR ±30°、SL/SR ±90°、BL/BR ±150° の水平 7 方位であり、仰角データも密な球面 HRTF もない。
したがって、HeSuVi backend の機能名と説明は「水平 360° 近似」とし、高さ付き 3D、head tracking、ユーザーごとの定位を主張しない。

初期比較対象は次の二モードとする。

1. HeSuVi faithful: 通常 stereo を正面 FL/FR の仮想 speaker として binaural 化する。
2. Horizontal stereo-pair orbit: stereo pair の中心角を水平に動かし、各 source を隣接二方位の畳み込み結果間で等電力 crossfade する。

orbit の推奨初期値は stereo width 60°、正面 0°開始、時計回り 60 秒/周、pause 中は角度も停止、track change で 0°へ戻す。
これは未決定の製品挙動なので、mono orbit、自動/手動、速度、往復、角度保持を聴感・負荷試験後に確定する。
上下を含む真の 3D が必要になった場合は、同じ `Spatializer` interface の別実装として SOFA/Steam Audio backend を追加し、HeSuVi WAV を SOFA と偽って扱わない。

Discord Voice へ送れるのは guild/VC ごとに一つの完成済み 48 kHz stereo stream である。
音量、HRIR、360°、角度は全リスナー共通であり、UI に「ヘッドホン推奨」「OS/ヘッドセット側の空間音響を無効化」「speaker では定位を保証しない」を表示する。

### 6. Audio pipeline

次の境界で実装する。

```text
resolver/downloader
→ decoder
→ 48 kHz resampler/channel mapper
→ guild DSP worker
→ HRIR convolution/spatial interpolation
→ guild volume/safety limiter
→ bounded SPSC PCM ring
→ Songbird RawAdapter
→ Opus/Discord
```

network I/O、WAV parse、database/cache access、FFT 準備を Songbird mixer callback 内で行わない。
DSP worker は 20 ms stereo frame を先行生成し、Songbird 側の read は block させない。
一時 underrun では EOF を返さず無音 frame を返して metric を増やす。
seek は decoder、resampler、convolver history、ring、position anchor を同一 generation で reset する。
HRIR 有効時は Opus passthrough を期待せず、decode → DSP → Opus re-encode の CPU を見積もる。

正確性は synthetic impulse、naive convolution との golden test、左右 swap、44.1→48 kHz の相対 onset、preset switch、seek/reset、NaN/Inf、Windows/Linux 差で検証する。
音質は unit test だけで宣言せず、HeSuVi/FFmpeg offline oracle と実 Discord VC のヘッドホン聴感試験を分けて記録する。

### 7. Web dashboard

画面は操作可能 guild 一覧、選択 guild の now-playing と queue、固定 player bar を持つ。desktop では guild navigation を左側へ配置し、狭い画面では dialog へ移す。
Meta Astryx の公開 component、Neutral Theme、StyleX token を視覚・interaction の正本とする。他サービスのロゴ、名称、asset、固有 icon、配色、寸法、animation、画面構成を参照・複製しない。

Discord OAuth2 Authorization Code Grant の `identify guilds` で login する。
表示する guild は OAuth 所属 guild、Bot 参加 guild、control policy 許可 guild の積集合とする。
OAuth token は browser へ返さず、opaque session ID を Secure/HttpOnly/SameSite cookie へ保存する。

Web は現在曲、artwork、状態、補間型 seek bar、previous/skip、play/pause、repeat、shuffle、stop、volume slider、HRIR select、360° toggle、queue、VC を表示する。
mutation は CSRF と Origin/Fetch Metadata を検証した REST、更新は revision 付き SSE とする。
SSE reconnect 後は full snapshot を取得し、event gap や stale revision を検出したら必ず snapshot へ収束させる。

### 8. Sharding と data ownership

Discord 公式式を使う。

```text
shard_id = (guild_id >> 22) % shard_total
```

一つの Bot process は固定された重複なし shard range を所有する。
Web command は guild の shard を計算し、Valkey Stream `cmd:shard:{id}` へ idempotency key、expected revision、deadline 付きで投入する。
owner は consumer group で読み、Actor 適用と dedupe を行ってから `XACK` する。crash 時は pending を `XAUTOCLAIM` し、poison message を隔離する。

PostgreSQL は guild 設定、playlist、HRIR metadata、audit 等の永続データの正本とする。
Valkey は opaque Web session、短期 cache、shard command Stream、短命な result、versioned player snapshot、Pub/Sub 通知に使う。
Pub/Sub だけを失われて困る操作や状態の正本にしない。
Voice handle と live DSP state は owner process 外へ移送しない。process failure 時の無停止 failover は MVP 非目標とし、再接続と queue 復元を目指す。

### 9. Security と resource limit

- URL は http/https のみ。全 A/AAAA、redirect ごとに loopback、private、link-local、multicast、unspecified、metadata address を拒否する。
- downloader を shell 文字列で起動しない。必要なら隔離した egress proxy/network と resource limit を使う。
- attachment の署名 URL は期限切れ前に取得し、Discord interaction が示す upload limit とアプリ側上限の小さい方を使う。
- upload は magic/decoder で検証し、random object key、容量/時間/channel/sample-rate/IR-length quota、TTL cleanup を適用する。
- Bot token は Bot service だけ、OAuth secret は API だけへ渡し、secret を image、source、log、metrics label に含めない。
- PostgreSQL role と Valkey ACL は service ごとに最小権限とし、両 port を public に publish しない。

### 10. Docker と運用

最終 Compose は Caddy、Web static asset、API、Bot shard process、one-shot migration、PostgreSQL、Valkey を healthcheck 付きで起動できること。
multi-stage build、non-root、read-only root filesystem、tmpfs、`no-new-privileges`、named volume、graceful SIGTERM、secret injection、backup/restore、SBOM、image scan を備える。
Caddy だけが 80/443 を公開し、app/data network は internal にする。
Windows 11 は Docker Desktop WSL 2 の Linux container、Ubuntu 26.04 は Docker Engine + Compose plugin を正式検証経路とする。
FFmpeg を同梱する場合は build configuration と license を記録し、GPL/nonfree 構成を無自覚に再配布しない。

### 11. 実装順序と完了の定義

1. Domain/Core、Components V2 DTO、HeSuVi loader と fixture。
2. PostgreSQL migration、Valkey、設定、Compose の runtime foundation。
3. Discord join、queue、direct URL/attachment、YouTube／SoundCloud resolver、`/play`、`/now`、`/stop`、`/leave`、5分退出。
4. 48 kHz DSP worker、固定正面 HeSuVi、hot switch、signal test。
5. 水平 orbit、負荷試験、実 VC 聴感試験。
6. OAuth、Axum REST/SSE、React dashboard。
7. 複数 shard/process、pending reclaim、dedupe、障害復旧、観測性。
8. Windows 11 / Ubuntu 26.04 の Compose E2E、backup/restore、soak、release audit。

各 phase は format、unit/integration test、Clippy、dependency/license/security scan を通す。
build 成功だけで Discord Voice、Components V2 表示、HRIR 音質、リアルタイム同期、failover を完了扱いにしない。

## 次に確定する製品判断

以下は実装を止めず推奨値で進められるが、MVP UI/DSP を固定する前にユーザー確認が必要です。

1. 360° を stereo-pair 自動 orbit とするか、固定 surround bed または mono orbit とするか。
2. `/stop` を queue clear とするか、queue を残すか。
3. Paused が何分続いたら退出するか。無期限保持を許すか。
4. DJ role の設定方法と、未設定時に誰が stop/設定変更できるか。
5. public launchでsite resolverを有効化する時期と、利用規約／法務／abuse対応の承認者。
6. HRIR は volume mount のみか、管理 Web upload も MVP に含めるか。
7. 想定する同時再生 guild 数、本番 CPU architecture/core 数、memory、CPU quota。
8. public multi-tenant Bot か、個人・限定 guild 用か。privacy、保持期間、abuse 対応が変わる。
9. 既存の別実装から挙動や HRIR asset を移行するか。asset は権利確認なしにコピーしない。
