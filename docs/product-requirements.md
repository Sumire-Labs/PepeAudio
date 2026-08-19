# PepeAudio-rs 製品要求仕様

- 文書状態: MVP実装契約・外部受入待ち
- 最終更新: 2026-08-13
- 対象: PepeAudio-rs の新規 Rust 実装
- 関連文書: [アーキテクチャ](architecture.md)、[ADR 0001](decisions/0001-initial-architecture.md)、[ADR 0002](decisions/0002-horizontal-orbit.md)

## この文書の読み方

要件の確度を次の三種類で表します。

| ラベル | 意味 |
|---|---|
| **確定** | ユーザーが明示した要求。変更には再合意が必要 |
| **暫定** | 現時点の推奨仕様。実装前または検証後に変更し得る |
| **未決定** | 製品判断、音響検証、Discord 制約の確認が必要 |

「確定」は実装済みや動作確認済みという意味ではありません。現在のリポジトリにはdomain/core、Components V2、HeSuVi loader、水平orbit DSP、Player Actor、Discord/API/storage/Web/OAuth/Docker production assemblyがあります。自動テスト、Web build、FFmpeg smoke、PostgreSQL/Valkey live test、production Compose smokeは成功していますが、実Discord Gateway/Voice/DAVE、実HRIRの聴感・負荷、Ubuntu 26.04実host、shard failoverは未検証です。

## 背景

一般的な Discord 音楽 Bot の再生・キュー操作に加え、HeSuVi で使われる HRIR プリセットを読み込み、Bot 側でバイノーラル処理した音声を Discord Voice へ配信したい。操作経路は Discord 内だけに限定せず、複数 guild を一覧できるリアルタイム Web ダッシュボードも提供する。

本プロジェクトは既存実装の互換移植ではなく、Rust、sharding、Web、Docker、音声処理を一貫した境界で設計するスタンドアロンの新規プロジェクトである。

## 製品目標

### G-001: Discord 内で完結する基本音楽プレイヤー

**確定。** URL または添付ファイルから曲をキューへ追加し、再生、一時停止、再開、スキップ、停止、ループ、シャッフル、音量変更を Discord Components V2 から操作できる。

### G-002: HRIR と 360° Audio

**確定。** HeSuVi 互換 HRIR プリセットを読み込み、再生中に選択できる。360° Audio の有効／無効を Discord と Web の両方から操作できる。

**MVP決定:** HeSuViの固定水平7方向をequal-power補間し、60°幅のstereo pairを曲の先頭の正面から60秒で時計回りに一周させる。これはguild共通の水平近似であり、高さを含む連続3D HRTF、head tracking、listener別出力ではない。

### G-003: リアルタイム Web ダッシュボード

**確定。** ユーザーが操作可能な Discord guild を左側に表示し、選択した guild の再生情報、シーク位置、キュー、主要操作、HRIR、音量、360° Audio をリアルタイムに表示・操作できる。

### G-004: 運用可能な Rust サービス

**確定。** Serenity、Poise、Songbird を中心に Rust で実装し、PostgreSQL、Valkey、Discord sharding、Docker を正式にサポートする。Windows 11 でテストでき、Ubuntu Server 26.04 LTS で本番運用できる。

### G-005: 検証可能な品質境界

**暫定。** コンパイル成功と、Discord／音声／Web の実動作確認を分ける。音響効果、Voice 接続、Components V2、shard routing、ブラウザ同期は、それぞれ専用の受け入れ試験で確認する。

### G-006: MIT ライセンスでの公開

**確定。** PepeAudio-rsが独自に作成したコードと文書は、別途ライセンス表記があるものを除き、MIT License（SPDX identifier: `MIT`）で提供する。著作権表示は `Copyright (c) 2026 SumireLabs, s12kuma01` とする。

この決定は、依存パッケージ、vendored source、FFmpeg、フォント、アイコン、HRIRプリセット、利用者が指定するメディアを再ライセンスしない。配布時は各第三者ライセンス、attribution、ソース提供義務、codec構成を個別に確認する。

## 想定ユーザー

### Guild 管理者

- Bot の導入と guild 設定を管理する。
- 使用可能な HRIR、既定音量、操作権限、DJ role を設定する。
- Web ダッシュボードから状態とエラーを確認する。

### Voice Channel のリスナー／DJ

- `/play`で曲を追加する。
- `/now`または Web から再生を操作する。
- HRIR、音量、360° Audio を許可された範囲で変更する。

### 運用者

- Bot shard、API、PostgreSQL、Valkey、Caddy を Docker で運用する。
- token、OAuth secret、backup、metrics、ログ、障害復旧を管理する。
- 配布可能な HRIR アセットのライセンスを確認する。

## 機能要件

### Discord コマンド

#### FR-DISCORD-001: `/play`

**確定。** `/play`は URL または Discord 添付ファイルを受け取り、guild の再生キューへ追加する。

暫定コマンド形は、入力型ごとに排他的な subcommand を設ける。

```text
/play url url:<string>
/play file file:<attachment>
```

`url` subcommandでは`url`をrequiredなString option、`file` subcommandでは`file`をrequiredなAttachment optionとして登録する。二つのnullable optionを同じ階層へ置かないため、URLと添付の同時指定をapplication logicでXOR検証する必要はない。subcommand未選択、required option欠落、不正なoption構造は、Discord command schemaとBot側の境界検証で状態変更前に拒否する。

動作要件:

1. guild 内でのみ実行できる。
2. 実行者が Voice Channel に参加していなければ、何も変更せずエラーを返す。
3. subcommand未選択、required option欠落、不正なoption構造では、何も変更せずエラーを返す。
4. Bot が未接続なら、実行者の Voice Channel へ接続する。
5. Bot が同じ guild の別 Voice Channel にいる場合の扱いは操作権限ポリシーに従う。初期案では自動移動せず拒否する。
6. 入力を検証・解決してからキューへ追加する。無効な入力を「追加成功」と表示しない。
7. Player がアイドルなら直ちに先頭曲の読み込みを開始する。
8. 再生中なら末尾へ追加する。
9. 成否は Embed ではなく Components V2 の Text Display／Container 等で返す。
10. URL、添付ファイルの取得にはサイズ、時間、protocol、宛先 IP、redirect、codec の制限を適用する。

**確定。** `/play`は直接音声URL、Discord attachment、YouTube／SoundCloudの
page／playlistを扱う。site resolverはdefault-offで、playlistはqueue空きとoperator上限
（hard maximum 100）までをbounded importする。Spotify track/albumとApple Music catalogは
optionalなmetadata照合であり、再生候補はYouTube優先、SoundCloud fallbackとする。
Spotify playlistはClient Credentialsでは取得せず、Spotify配信音声、cookie、user OAuth、
DRM回避、任意site downloaderは対応範囲外とする。

#### FR-DISCORD-002: `/now`

**確定。** `/now`は現在の Player 状態を、Embed を使わず Components V2 で表示する。

表示対象:

- 曲名
- 作者または配信元が提供する表示情報
- 再生時間と総時間
- Playing / Paused / Loading / Idle / Error 等の状態
- Voice Channel
- キュー件数
- repeat mode
- shuffle 状態
- 現在の音量
- 現在の HRIR プリセット
- 360° Audio の有効／無効

暫定コンポーネント構成:

1. Container
2. Text Display: 曲情報と状態
3. 必要なら Thumbnail: 信頼できる画像 URL のみ
4. Action Row:
   - previous（履歴がある場合）
   - pause / resume
   - skip
   - stop
5. Action Row:
   - repeat
   - shuffle
   - 360° Audio
6. String Select: HRIR preset
7. String Select: volume

要件:

- Components V2 の`IS_COMPONENTS_V2` message flagを使用する。
- Embed は一切含めない。
- 任意のカスタム色は使わない。Containerの`accent_color`は省略し、操作ボタンは初期状態でDiscord標準のneutral／secondary styleを使用する。
- ボタンとselect menuのcustom IDには、guild、表示revision、操作種別を安全に関連付ける。ユーザー入力をそのまま埋め込まない。
- 古い`/now`画面からの操作でも、現在状態と権限をサーバー側で再検証する。
- Discord のcomponent数、Action Row、select option数、message size制限を超える場合、HRIR presetをカテゴリ／ページに分ける。

volume selectorの段階値、previousの導入、表示を公開返信にするか実行者だけにするかは**未決定**である。

#### FR-DISCORD-003: `/stop`

**確定。** `/stop`は対象guildの再生を停止する。

**MVP仕様:** 現在曲、通常queue、再生履歴をクリアし、Voice接続は維持してIdleへ遷移する。その後、新しい曲が追加されなければ5分後に退出する。保存playlistは削除しない。同じVoice Channelにいるconfigured DJ roleまたはManage Guild権限保持者だけが実行できる。

#### FR-DISCORD-004: `/leave`

**追加済み。** `/leave`は同じVoice Channelにいるconfigured DJ roleまたはManage Guild権限保持者だけが実行できる。現在曲、通常queue、再生履歴、managed media leaseを解放し、Voice接続とguild Player Actorを即時終了する。既に退出済み、BotのVoice Channelを確定できない、または別Voice Channelからの要求はfail closedとする。

#### FR-DISCORD-005: 共通エラー表示

**暫定。** すべてのユーザー向けエラーは、原因と次の行動が分かる短い Components V2 表示にする。内部URL、filesystem path、token、stack trace、他ユーザーの機密情報を表示しない。

### Player とキュー

#### FR-PLAYER-001: guild ごとの独立 Player

**確定。** Voice接続、現在曲、queue、volume、repeat、shuffle、HRIR、360° Audio、アイドルタイマーはguildごとに独立させる。

#### FR-PLAYER-002: 単一の状態変更経路

**暫定。** Discord command、Discord component、Web API、Songbird event、timeoutは、すべてguild Player Actorへcommandを送り、ActorだけがPlayer状態を変更する。

#### FR-PLAYER-003: 基本操作

**確定。** 少なくとも次を提供する。

- play / resume
- pause
- stop
- skip
- repeat
- shuffle
- volume
- HRIR preset selection
- 360° Audio toggle

previous、queue reorder、remove、seekのDiscord側提供範囲は**暫定**とする。Webダッシュボードではシーク、queue項目の削除、上下への並べ替えを提供する。並べ替えは表示indexではなく安定したtrack IDと移動先track IDを送り、expected revisionが一致した場合だけ適用する。

#### FR-PLAYER-004: 5分アイドル退出

「曲が再生していない状態が5分続いたら自動退出」は**確定要件**である。判定の厳密な意味は次を**暫定仕様**とする。

- `IdleConnected`は、Voiceへ接続中、現在曲なし、queue空の状態。
- `IdleConnected`へ入った時点で、単調増加時計を使った300秒timerを開始する。
- 有効な曲が追加されLoadingまたはPlayingへ移るとtimerをcancelする。
- 300秒継続した場合、Voiceから退出し、最終Disconnected snapshotを公開してidle Actorを終了する。次の操作時に新しいActorを生成する。
- track解決中のLoadingには別のtimeoutを設け、無期限にアイドル退出を妨げない。
- timer callbackはPlayer revisionを検証し、古いtimerが新しい再生を切断しないようにする。
- 手動切断、guild削除、shard停止時はtimerをcancelする。
- `/stop`がqueueをクリアする暫定仕様では、`/stop`後から300秒を数える。

Pausedには現在曲が残っているため、MVPではアイドル扱いにしない。

#### FR-PLAYER-005: 位置とrevision

**暫定。** Player snapshotは単調増加するrevisionを持つ。位置は毎秒永続化せず、`position_ms_at_anchor`、`anchor_server_time`、状態からクライアントが補間する。seek、pause、resume、track change時にanchorを更新する。

### HRIR と 360° Audio

#### FR-AUDIO-001: HeSuVi 互換preset import

**確定。** HeSuVi HRIR presetを読み込み可能にする。

MVPのloaderはRIFF/WAVE、7chまたは14ch、44.1 kHzまたは48 kHz、PCM16またはIEEE f32を受け付ける。44.1 kHzは全planeを同じ比率で48 kHzへ変換する。拡張子だけで信頼せず、少なくとも次を検証する。

- container／sample形式
- sample rate
- channel数とchannel順
- impulse長
- NaN / Inf / silent data
- peak level
- ファイルサイズ上限
- parserが受け付ける拡張子とmagic bytes

HeSuVi互換presetの意味は、水平面上の固定7方向それぞれについて左右耳へのresponseを得られることとする。presetごとの実ファイル配置とchannel orderはfixtureで確認する。これは任意azimuth／elevationを連続queryできるHRTF surfaceを意味しない。

import時に内部の正規化済み形式へ変換し、再生callback内では任意ファイルをparseしない。

#### FR-AUDIO-002: preset metadata

**暫定。** presetには一意ID、表示名、scope、sample rate、layout、checksum、import version、license metadataを持たせる。バイナリ本体をPostgreSQLやValkeyへ入れない。

#### FR-AUDIO-003: 再生中切り替え

**確定。** `/now`とWebからHRIRを切り替えられる。

**暫定。** 切り替えはaudio frame境界で行い、短いcrossfadeまたはfilter state移行によりclick/popを抑える。切り替え失敗時は現在presetを維持し、再生全体を停止させない。

#### FR-AUDIO-004: 360° Audio

**確定。** DiscordとWebに360° Audio toggleを提供する。

**MVP仕様:** stereoの左右入力を60°幅のpairとして、曲の先頭では正面へ置き、60秒で時計回りに一周させる。HeSuViの隣接する固定方向responseをequal-power補間する。軌道はPCM sample clockで進み、pause中は停止し、seek時は曲位置から復元する。詳細は[ADR 0002](decisions/0002-horizontal-orbit.md)を正本とする。

初期制約:

- guildごとに一つの共有状態
- 全リスナーへ同じステレオ出力
- head trackingなし
- ユーザー別HRIRなし
- ユーザー別音源方向なし
- HeSuViだけを使用する場合は高さ方向なし
- 固定7方向を超える連続性は追加処理による近似であり、presetそのものの能力ではない

手動方向指定、mono／surround固有mapping、高さ付きdatasetは初期MVPに含めない。preset未選択時はspatial toggleを有効化できず、処理をdryで維持する。UIは「水平7方向を補間したguild共通の音場」と明記する。

#### FR-AUDIO-005: 安全な信号レベル

**暫定。** HRIR convolution後に想定外の増幅が起き得るため、gain staging、peak監視、必要ならlimiterを設ける。音量100%が必ずしもunity gainを超えないよう定義する。音質や定位の主張は測定・聴感試験後にのみ公開する。

### Web ダッシュボード

#### FR-WEB-001: Discord OAuth2 login

**暫定。** Discord OAuth2 Authorization Code Grantを使い、初期scopeを`identify guilds`に限定する。Discord access tokenとrefresh tokenをブラウザへ返さない。

#### FR-WEB-002: guild 一覧

**確定。** desktopの主navigation、mobileのdialog navigationへ、ユーザーが操作可能なDiscord guild一覧を表示する。

表示対象は次の積集合とする。

```text
OAuth2で得たユーザー所属guild
∩ Botが参加中のguild
∩ 製品のcontrol policyで許可されたguild
```

guild icon、名前、Bot接続状態、現在再生中かどうかを表示する案とする。

#### FR-WEB-003: now playing

**確定。** 選択guildについて次を表示する。

- 曲情報
- artwork（安全なURLまたはproxy済みassetのみ）
- status
- seek bar
- elapsed / duration
- play / pause
- previous / skip
- repeat
- shuffle
- stop
- volume bar
- HRIR selector
- 360° Audio toggle
- queue（項目削除と上下への並べ替えを含む）
- Voice Channel

#### FR-WEB-004: リアルタイム反映

**確定。** Discord側の操作、track終了、timeout、Web側の操作を、ページ再読み込みなしで反映する。

**暫定。** 操作はREST、通知はSSEとする。接続直後に完全snapshotを送り、その後はrevision付き差分eventを送る。event欠落を検出した場合は完全snapshotを再取得する。

#### FR-WEB-005: Astryx design system

**確定。** Meta Astryxの公開component、Neutral Theme、StyleX tokenをUIの正本とする。React 19、Astryx、StyleXは互換性を検証したexact versionで固定し、更新時にcomponent API、アクセシビリティ、CSS、license noticeをまとめて再検証する。

desktopではguild navigation、再生workspace、queue／360° Audio inspector、下部固定playerを持つ。狭い画面ではnavigationをdialogへ、inspectorをmain flowへ移す。生の色値や旧Spotify風presentation fieldをdomain modelへ持ち込まず、Astryx componentで表現できないPepeAudio固有レイアウトだけを責務別StyleX moduleへ置く。

**必須制約:** Spotifyを含む他サービスのロゴ、商標、固有アイコン、文章、正確な配色・寸法・アニメーション、画面そのものを複製しない。非公式StyleX Vite pluginやAstryx componentの内部DOM／class名へ依存しない。

#### FR-WEB-006: responsive / accessibility

**確定。** desktopを第一対象とし、狭い画面ではguild navigationをdialogへ移し、queue／360° Audio inspectorをmain flowへ配置する。keyboard操作、focus表示と復帰、landmark／accessible name、色だけに依存しない状態表示、`prefers-reduced-motion`を最低要件とする。

### Sharding と複数instance

#### FR-SCALE-001: Discord sharding

**確定。** Discord Gateway shardingをサポートする。Discordが返す推奨shard数と公式routing式を使用する。

#### FR-SCALE-002: shard ownerへのcommand配送

**暫定。** Web操作はguild IDからshardを決め、`cmd:shard:{shard_id}` Valkey Streamへ投入する。担当Bot processだけがconsumeし、成功後にackする。

#### FR-SCALE-003: 冪等性

**暫定。** 各操作に一意command IDを付ける。再配送で二重skip、二重queue追加、二重stopが起きないよう、短期dedupe recordを持つ。

#### FR-SCALE-004: API horizontal scaling

**暫定。** Axum APIはsessionと共有状態をValkey/PostgreSQLへ置き、複数replicaで動かせる。SSEは任意replicaへ接続でき、sticky sessionを必須にしない。

#### FR-SCALE-005: Voice failoverの限界

**確定させるべき制約。** Voice connectionと進行中audio pipelineはBot process内にある。process停止やreshard時に、Voiceを完全無停止で他processへ移送できることは初期目標にしない。queue復元と再接続は可能でも、一時的な再生中断を許容する。

## 非機能要件

### NFR-001: 対象プラットフォーム

- **確定:** 本番はUbuntu Server 26系列。
- **暫定:** 正式表記をUbuntu Server 26.04 LTS Resoluteとする。
- **確定:** テスト・開発はWindows 11をサポートする。
- **暫定:** Docker DesktopのWSL 2 Linux containerを標準の統合テスト経路にする。

### NFR-002: Docker 完全サポート

**確定。** Bot、API、Web、migration、PostgreSQL、ValkeyをDockerで構築・起動・health checkできる。

最低条件:

- multi-stage build
- version pinningとlockfile
- non-root runtime
- healthcheck
- graceful shutdown
- Compose secrets
- named volume
- migration one-shot service
- 開発用と本番用の設定差分
- Linux amd64を必須build target
- backup／restore手順

### NFR-003: 性能目標

次は**暫定目標**であり、測定環境を記録して評価する。

- 正常な単一host環境で、操作受付からUI状態反映までp95 2秒以内
- seek barはイベント間を滑らかに補間し、状態変化後2秒以内に再anchor
- 5分退出は300秒から±5秒以内
- audio処理はSongbirdへ必要frameを期限内に供給し、underrunをmetrics化
- HRIR切り替えでpanic、NaN、極端なlevel jumpを起こさない

### NFR-004: 可用性と劣化動作

**暫定。** PostgreSQL／Valkeyが一時停止しても、既に再生中のguild Playerを可能な限り継続する。Web操作は誤成功を返さず503またはdegradedを表示する。復旧後にBotが最新snapshotを再公開する。

### NFR-005: 観測性

**暫定。** structured log、metrics、traceを提供する。

主要観測項目:

- shard ready / reconnect
- active Voice Call
- command latency / reject / duplicate
- Stream pending count / oldest pending age
- track resolver／decoder failure
- audio underrun／DSP time
- SSE connection／resync
- PostgreSQL pool wait
- Valkey reconnect
- Discord REST rate limit
- idle disconnect

user ID、guild ID、URLをPrometheus labelへ直接入れず、高cardinalityと情報漏えいを避ける。

### NFR-006: Security

**暫定だが実装必須。** 次を最低条件とする。

- OAuth `state`
- Secure / HttpOnly / SameSite session cookie
- CSRF tokenとOrigin／Fetch Metadata検証
- すべての変更操作で認可を再確認
- Bot tokenをBot serviceだけへ付与
- secretをsource、image、logへ入れない
- URLのSSRF対策
- upload size／duration／format／path検証
- downloaderをshell文字列として実行しない
- PostgreSQL runtime roleの最小権限
- Valkey ACL、外部port非公開
- Caddy以外のserviceをpublic networkへ公開しない
- dependency audit、SBOM、container scan

### NFR-007: Privacy

**未決定。** 次の保持期間と公開policyを決める。

- Discord user表示情報
- guild表示情報
- audit event
- playlist
- upload media
- HRIR upload
- OAuth refresh token
- IP addressを含むaccess log

## 状態モデル

暫定Player state:

```text
Disconnected
  └─ join → IdleConnected

IdleConnected
  ├─ enqueue → Loading
  └─ 300s timeout → Disconnected

Loading
  ├─ ready → Playing
  ├─ failure + next item → Loading
  └─ failure + empty queue → IdleConnected

Playing
  ├─ pause → Paused
  ├─ seek → Playing(new anchor)
  ├─ track end + next → Loading
  ├─ track end + empty → IdleConnected
  └─ stop → IdleConnected

Paused
  ├─ resume → Playing
  ├─ seek → Paused(new anchor)
  └─ stop → IdleConnected
```

Errorは独立した永久状態にせず、操作結果／診断情報として持ち、回復可能ならLoadingまたはIdleConnectedへ遷移する案とする。

## 操作権限の暫定案

| 操作 | 暫定許可 |
|---|---|
| guild設定 | owner / Administrator / Manage Guild |
| HRIR import・削除 | owner / Administrator / Manage Guild |
| play・queue追加 | 同じVoice Channelのメンバー |
| pause・resume・skip・seek | 同じVoice Channel、DJ role、またはManage Guild |
| stop・disconnect | DJ roleまたはManage Guild |
| volume・HRIR・360° | guild設定で選択 |

MVPの既定policyは同じVoice Channelのメンバーを許可する。guild設定で`dj_only`または`manage_guild`へ制限でき、DiscordとWebは同じ保存policyを評価する。`/stop`はpolicyにかかわらず同じVoice ChannelのDJ roleまたはManage Guildを要求する。Botの現在Voice Channelを確定できないcontrolはfail closedとする。

## 段階的ロードマップ

### Phase 0: 要件固定とfixture

- 未決定事項のうちMVPを妨げる項目を決定
- HeSuVi互換fixtureと期待channel mappingを確定
- Discord test guild／OAuth application／domain方針を準備
- threat modelとライセンス台帳を作成

完了条件:

- MVPの操作権限、idle、stop、360°の意味が承認されている
- 再配布可能なテストHRIRが用意されている

### Phase 1: Runtime foundation

- Rust workspace
- config／secret loader
- PostgreSQL migration
- Valkey接続
- Dockerfile／Compose
- tracing／health endpoint

完了条件:

- Windows 11 + WSL2とUbuntu 26.04で同じCompose stackが起動
- migration、healthcheck、graceful shutdownが確認済み

### Phase 2: Discord MVP

- Serenity／Poise／Songbird
- guild Player Actor
- queue
- `/play`、`/now`、`/stop`、`/leave`
- Components V2のみ
- 5分idle退出

完了条件:

- test guildでURL／fileの再生、操作、退出を実機確認
- Embedが送信されないことをAPI payloadで確認

### Phase 3: HRIR / 360° Audio

- preset import、検証、内部形式
- convolution pipeline
- runtime switching
- 360° Audio MVP
- signal／impulse／聴感試験

完了条件:

- fixtureのchannel mappingが自動testで一致
- bypassとのlevel差、NaN、click、underrunを確認
- Discord実機で左右・代表方向を確認

### Phase 4: Web dashboard

- Discord OAuth2
- REST／OpenAPI
- SSE
- React／Vite UI
- guild一覧、player、queue、volume、HRIR、360°

完了条件:

- Discord操作とWeb表示が双方向に同期
- 再接続、event欠落、複数tabでsnapshotへ収束
- Playwrightで主要フローを確認

### Phase 5: Sharding / hardening

- shard range起動
- Valkey Streams command routing
- idempotency／pending recovery
- rate limit／SSRF／upload hardening
- metrics、backup、restore、負荷試験

完了条件:

- 複数Bot processでguild操作が正しいownerだけへ届く
- consumer crash後に未ack commandを回収
- 二重skip等が起きない
- PostgreSQL restore drillが成功

### Phase 6: Release candidate

- Ubuntu 26.04 soak test
- Windows 11再現手順
- 運用runbook
- privacy／license／third-party notices
- upgrade／rollback手順

## MVP 受け入れ条件

### Discord

- [ ] `/play url url:<string>`で有効な音声をqueueへ追加し、実行者のVCで再生できる。
- [ ] `/play file file:<attachment>`で許可された添付音声を再生できる。
- [ ] subcommand未選択、required option欠落、不正なoption構造を状態変更前に拒否する。
- [ ] `/now`が現在情報と操作componentをComponents V2だけで表示する。
- [ ] 送信payloadにEmbedが含まれない。
- [ ] pause / resume / skip / stop / repeat / shuffle / volumeがPlayer Actorを通る。
- [ ] 権限のないユーザー操作を状態変更前に拒否する。
- [ ] queueが空のIdleConnected状態を300秒継続するとVCから退出する。
- [ ] 299秒以内に曲を追加した場合、古いtimerが新しい再生を切断しない。

### HRIR / 360° Audio

- [ ] 合意済みHeSuVi fixtureをimportできる。
- [ ] 不正channel数、巨大file、NaN／Inf、不正headerを拒否する。
- [ ] preset切り替えが再生をpanicさせない。
- [ ] HRIR bypassと有効時の出力を録音し、自動signal testと聴感testを分けて記録する。
- [ ] 360° toggleがguild snapshotと実際のDSP pathの両方へ反映される。
- [ ] 全リスナー共通出力であることをUI／文書に表示する。

### Web

- [ ] Discord OAuth2 `identify guilds`でloginできる。
- [ ] 操作可能guildだけを表示する。
- [ ] 現在曲、status、seek、queue、volume、HRIR、360°を表示する。
- [ ] Web操作がDiscord側表示へ、Discord操作がWebへ反映される。
- [ ] SSE切断後に自動再接続し、完全snapshotへ収束する。
- [ ] 古いrevisionのqueue編集を無言で上書きしない。
- [ ] Astryx doctor、TypeScript、component test、production buildが成功する。
- [ ] 320／360／1024／1440pxで横overflowがなく、keyboard focusが失われない。
- [ ] Spotifyのasset／ロゴ／固有UIを含まない。

### Sharding / Docker

- [ ] 公式式でguildからshardを計算する。
- [ ] Web commandが担当shardのStreamへ入り、担当Botだけが処理する。
- [ ] command再配送が二重操作にならない。
- [ ] Windows 11 WSL2とUbuntu 26.04でCompose起動が成功する。
- [ ] PostgreSQL／Valkeyをhost public portへ公開しない。
- [ ] secretがimage history、repository、通常logに含まれない。
- [ ] Bot／APIがSIGTERMでgraceful shutdownする。

## 非目標

初期リリースでは次を目標にしない。

- Discordユーザーごとの別々のHRIR出力
- head tracking
- リスナーごとの音源方向
- HeSuVi preset単独による連続azimuth／elevationの真の3D定位
- Webブラウザへの音声ストリーミング
- Discord Voice sessionの完全無停止process移送
- Spotify音声の取得やSpotify UIの複製
- DAW相当の編集・mixing
- 任意plugin codeのupload／実行
- DRM回避
- 対応を検証していないメディアサイトの包括的サポート
- KubernetesをMVP必須要件にすること
- mobile native application
- build成功だけを根拠にした音質・低遅延・可用性の宣伝

## 未決定事項

優先度順:

1. 配布可能な実HeSuVi fixtureと音響受入datasetをどれにするか。
2. HRIR storageのquota、保持期間、ライセンス表示をproductionでどう運用するか。
3. public launchでdefault-offのsite extractorおよびcross-service matchingを有効化する
   時期、providerの利用条件・attribution／branding要件とpolicy/legal承認記録。
4. Discord componentのvolume段階値とHRIR pagination。
5. `/now`を公開返信にするか実行者限定にするか。
6. queueをprocess再起動後に復元するか。
7. Webからfile uploadを許可するか。
8. 単一host Composeからmulti-hostへ拡張する時期とowner fencing方式。
9. production domain、OAuth redirect URI、30分以下で採用するabsolute/idle session期限。
10. audit、playlist、upload、OAuth tokenの保持期間。
11. backup RPO／RTO。

## 一次資料

- [Discord Message Components reference](https://docs.discord.com/developers/components/reference)
- [Discord application commands](https://docs.discord.com/developers/interactions/application-commands)
- [Discord Gateway sharding](https://docs.discord.com/developers/events/gateway)
- [Discord OAuth2](https://docs.discord.com/developers/topics/oauth2)
- [Discord user guilds API](https://docs.discord.com/developers/resources/user)
- [Discord rate limits](https://docs.discord.com/developers/topics/rate-limits)
- [Songbird](https://docs.rs/songbird/latest/songbird/)
- [HeSuVi: impulse responseの記録方法と7 channel / 14 IR構造](https://sourceforge.net/p/hesuvi/wiki/How-To%20Record%20Impulse%20Responses%20Digitally/)
- [Axum SSE](https://docs.rs/axum/latest/axum/response/sse/)
- [Valkey Streams](https://valkey.io/topics/streams-intro/)
- [Valkey Pub/Sub](https://valkey.io/topics/pubsub/)
- [Docker Engine on Ubuntu](https://docs.docker.com/engine/install/ubuntu/)
- [Docker Desktop WSL 2](https://docs.docker.com/desktop/features/wsl/)
- [OWASP SSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html)
- [OWASP File Upload](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html)
- [OWASP CSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)
