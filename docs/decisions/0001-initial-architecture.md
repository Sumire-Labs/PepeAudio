# ADR 0001: 初期アーキテクチャ

- Status: Accepted for MVP foundation
- Date: 2026-08-12
- Decision owners: Project maintainers
- Scope: Bot、Web、リアルタイム同期、データ、sharding、Docker
- Supersedes: なし

## Context

PepeAudio-rs は、次の要求を同時に満たす新規プロジェクトである。

- Rust、Serenity、Poise、SongbirdによるDiscord音楽Bot
- HeSuVi互換HRIRとguild共通360° Audio
- `/play`、`/now`、`/stop`（後続改善として`/leave`も追加）
- Embedを使わないDiscord Components V2 UI
- リアルタイムWebダッシュボード
- PostgreSQLとValkey
- Discord Gateway sharding
- Dockerによる開発・テスト・本番
- Windows 11でのテスト
- Ubuntu Server 26.04 LTSでの本番運用

Web操作が追加されると、HTTP API processが直接Songbird Callを持つ単一process構成は、複数replicaやshardingで破綻する。また、現在位置を毎秒databaseへ保存する構成、失われ得るPub/Subだけで操作を配送する構成、DiscordとWebが別々のPlayer logicを持つ構成は、競合と障害復旧を難しくする。

Web UIは認証後の操作画面が中心であり、SEOやserver-side renderingの優先度は低い。一方で、Rust API、OAuth、リアルタイムevent、Docker運用の境界を単純に保つ必要がある。

repositoryにはdomain、Components V2、HeSuVi loader、参照DSP、Player Actor、Discord/API/storage/Web/DockerのMVP基盤を実装した。自動テストとdevelopment Compose smokeは本ADRの境界がビルド・接続できることを示すが、実音声、production OAuth、分散routing、Discord実機、音響品質、failoverまでの妥当性はまだ証明しない。

また、HeSuVi互換preset単独が表現するのは固定された水平7方向であり、高さ付きの連続3D HRTFではない。本ADRは音声処理をBot側の独立境界へ局所化するところまでを決め、製品上の「360° Audio」の具体的なrouting、移動、crossfade／補間、別dataset採用は決めない。

## Decision drivers

優先順:

1. Discord側とWeb側の操作結果が一つのPlayer状態へ収束すること
2. guildがどのshard processに所属しても操作できること
3. 二重skip、二重queue追加、stale timerによる切断を防げること
4. Pub/Sub切断やAPI replica停止から再同期できること
5. Windows 11とUbuntu 26.04で同じcontainer artifactを検証できること
6. 音声realtime pathをdatabase／network I/Oから分離できること
7. 初期の運用部品数を抑えつつ、将来の水平scaleを妨げないこと
8. 仕様未決定のHRIR／360° Audioを局所化できること

## Decision

このADRがAcceptedになった場合、初期実装は以下を採用する。

### 1. Guild Player Actorを唯一のruntime state ownerにする

各guildについて、Songbird Call、現在曲、queue、position anchor、volume、repeat、shuffle、HRIR、360° Audio、idle timerをBot shard process内の一つのPlayer Actorが所有する。

Discord command、Discord component、Web command、Songbird track event、5分timerは、すべてtyped domain commandとしてActorへ送る。外部taskはPlayer内部fieldを直接変更しない。

HRIR処理へ渡す方向modelもActor command／stateから駆動するが、HeSuViの固定水平7方向を超える連続性はpreset固有能力として仮定しない。具体的な360° Audio MVPはfixtureと測定結果に基づく後続ADRで決める。

### 2. Bot、API、Webをruntime境界として分ける

- Bot: Discord Gateway／Voice／Player／DSP
- Axum API: OAuth2／REST／SSE／認可／command routing
- React + Vite SPA: browser UI
- Caddy: TLS／static files／reverse proxy

APIはSongbird handleとDiscord bot tokenを持たない。Bot参加guildとPlayer snapshotは共有store経由で参照する。

### 3. React 19 + TypeScript + Vite 8を採用する

Webはprivate dashboard中心でSSRを必要としないため、Viteで静的buildする。Node.jsはbuild stageだけで使用し、production runtimeにはCaddyと静的assetだけを残す。

初期案では三ペイン／下部playerの情報設計を候補とした。視覚・component systemの最終決定は [ADR 0003](0003-astryx-web-design-system.md) が置き換える。

### 4. Axum REST + SSEを採用する

- Browser→serverの操作はREST
- Server→browserの状態通知はSSE
- 接続直後に完全snapshot
- 以降はrevision付きevent
- event欠落／lag時はsnapshot再取得
- seek barはposition anchorからbrowserで補間

双方向WebSocketは初期要件に使用しない。

### 5. PostgreSQLをdurable dataの正本にする

PostgreSQLはguild設定、control policy、HRIR metadata、playlist、audit等を所有する。SQLx migrationをone-shot serviceとして実行する。

進行中Player stateはPostgreSQLの正本にしない。HRIR／mediaバイナリもPostgreSQLへ格納しない。

### 6. Valkeyをsession、cache、command、snapshotへ使う

- OAuth state／opaque session
- 短期Discord guild cache
- shard lease／readiness
- shard別command Streams
- command result／dedupe
- versioned Player snapshot
- Pub/Sub event
- rate limit

失われて困るWeb commandは`cmd:shard:{shard_id}` Streamへ入れ、`XREADGROUP`、`XACK`、`XAUTOCLAIM`、idempotency keyで処理する。Pub/Subはsnapshot更新後の通知にだけ使う。

command busを同じValkeyへ置く初期構成では、AOF、`noeviction`、Stream trim、memory monitoringを必要とする。

### 7. Discord公式shard routingを使う

```text
shard_id = (guild_id >> 22) % num_shards
```

Bot processは明示されたdisjoint shard rangeを担当する。Web APIはguild IDからshardを計算し、そのshardのStreamへcommandを配送する。MVPは同じrangeを同時起動しないstop-before-startを運用契約とし、owner epoch／fencing leaseはrolling overlapを導入する将来段階で追加する。

### 8. Caddyをpublic edgeにする

Caddyだけが80/443を公開し、自動HTTPS、Vite asset、SPA fallback、Axum reverse proxy、security headerを担当する。PostgreSQL、Valkey、Axumはhost public portへ公開しない。

### 9. Docker Composeを正式な単一host deploymentにする

Bot、API、Caddy、migration、PostgreSQL、Valkeyをmulti-stage OCI image／Composeで実行する。Windows 11ではDocker Desktop WSL 2、Ubuntu Server 26.04 LTSではDocker Engine + Compose pluginを使用する。

Docker full supportは、MVPでKubernetesを必須にすることを意味しない。multi-host orchestratorは別ADRで決める。

## Detailed consequences

### Positive

- DiscordとWebが同じPlayer logicを使用する。
- shard owner以外がVoiceを誤操作しない。
- APIをstatelessに近づけ、複数replicaへscaleできる。
- SSE切断やPub/Sub event欠落からsnapshotへ回復できる。
- Stream pending messageとidempotencyによりcrash／retryを扱える。
- Vite static buildによりproduction Node serverが不要になる。
- 音声realtime pathとDB／network I/Oを分離できる。
- HRIR実装をaudio crate境界へ閉じ込められる。
- WindowsとLinuxで同じcontainer構成を検証できる。

### Negative

- 単一process Botより部品とdistributed stateが増える。
- Valkeyがcacheだけでなくcommand transportにもなり、persistence／memory管理が必要になる。
- Streamは自動でexactly-onceにならず、dedupe実装が必要になる。
- SSE eventとsnapshotのrevision protocolを設計・testする必要がある。
- Bot process再起動時、Discord Voice再生を完全無停止で移送できない。
- Caddy、API、Botの複数imageを維持する必要がある。
- imported HRIRをmulti-hostで共有する場合、object storageが必要になる。

### Risks

- Actor内で長いresolve／decodeをawaitするとguild操作全体が停止する。
- stale shard ownerがsnapshotを上書きするとUIが逆戻りする。
- Valkey Pub/Subを正本と誤認するとevent lossで不整合になる。
- `tower-sessions`とRedis store crateのversion不一致を無視すると依存解決またはtrait互換で問題になる。
- URL downloaderがapplicationの事前検査と別にDNS解決するとSSRF防御を迂回し得る。
- HRIR convolutionがgain、CPU、latency、underrunを悪化させ得る。

各riskへの対策は[アーキテクチャ文書](../architecture.md)で定義する。

## Alternatives considered

### A. 一つのBot binaryにWeb serverも同居

却下理由:

- API replicaのscaleとGateway shard ownershipが結び付く。
- Web deployがVoice connection再起動を招く。
- process境界がなく、Bot tokenとOAuth secretの権限分離が弱い。

小規模prototypeでは単純だが、明示されたsharding／Web要件に合わない。

### B. Next.js full-stack

却下理由:

- Rust APIと責務が重複する。
- production Node serverが追加される。
- self-hosted multiple instanceでcache coordinationが必要になる。
- static exportではCookie、Proxy、Server Actions等のdynamic機能を使えず、利点が小さい。
- 認証後dashboard中心でSSR／SEOの価値が低い。

公開marketing siteやSSRが将来必要なら、dashboardと別serviceとして再検討できる。

### C. WebSocketですべての操作と通知を行う

却下理由:

- 操作は低頻度で、RESTのstatus、CSRF、idempotency semanticsが適している。
- custom reconnect、heartbeat、ack、backpressureが増える。
- 現在要件はserver→browser通知が中心。

将来、連続双方向interactionやpresenceが必要になった場合は再検討する。

### D. PostgreSQL LISTEN/NOTIFYをcommand busにする

却下理由:

- durable command queue、pending ownership、claimを別実装する必要がある。
- PostgreSQLへrealtime fan-out負荷とPlayer ephemeral stateを集中させる。
- Valkeyが既に必須要件である。

durable business outboxにはPostgreSQLを併用できるが、Player command transportにはValkey Streamsを使う。

### E. Valkey Pub/Subだけでcommandを配送

却下理由:

- Valkey公式上at-most-onceで、subscriber切断中のmessageは失われる。
- stop／skip／queue追加の誤消失を検知できない。

Pub/Subは失ってもsnapshot再取得で回復できる表示通知に限定する。

### F. queueと再生位置をPostgreSQLへ毎秒保存

却下理由:

- 書き込み量と競合が不要に増える。
- DBの時刻と実際のaudio clockが正本として競合する。
- seek barはanchor補間で十分。

必要ならtrack change／pause／seek等の境界でcheckpointを保存する。

### G. NATS JetStream等を初期から追加

保留理由:

- command transportとして明確だが、Valkey必須要件に加えて運用componentが一つ増える。
- 初期規模ではshard別Valkey Streamsで要件を満たせる。

Valkeyのsession／cache負荷とcommand保証を分離する必要が出た場合の有力候補とする。

### H. Leptos／Yew等のRust frontend

却下理由:

- Rust統一の利点はあるが、dashboard UI、headless component、testing、designer toolingではReact ecosystemの方が採用しやすい。
- audio／Bot側のRust境界とfrontend言語統一は独立した判断である。

## Validation status

`Accepted for MVP foundation`の実装・検証状況を示す。自動検証済み項目と、外部環境での受入を必要とする項目を分ける。

- [x] 初期domain／shard modelとComponents V2 wire adapterをunit testした。
- [ ] 製品要求仕様のMVP未決定事項をownerが確認した。
- [x] Axum + SSE + Valkey Pub/Subでsnapshot／resyncと再接続を自動検証した。
- [x] shard別Streamでcommand routing、ack、pending reclaim、dedupeを自動検証した。
- [x] Songbird Callをguild Player Actorから操作するproduction adapterとfake-port testを実装した。
- [x] HRIR rendererの準備をPCM worker外で行い、partitioned convolutionの数値oracle／allocation／throughput境界を自動検証した。
- [ ] Windows 11 WSL 2とUbuntu 26.04でmulti-stage imagesをbuildできた。
- [ ] Discord Components V2だけで`/now`の必要UIが収まることをstagingで確認した。
- [x] HeSuVi fixtureの水平7方向mappingを確認し、360° Audio MVPの処理と呼称をADR 0002で承認した。
- [x] opaque Valkey sessionを使う専用OAuth adapterを実装し、live Valkey testとproduction API smokeを通した。

## Revisit triggers

次の場合は新しいADRで見直す。

- multi-host productionが必須になった。
- Valkey command backlog／memoryが運用限界に達した。
- strictなcommand durability保証が必要になった。
- APIとBotを同一failure domainに戻す合理的理由が生じた。
- WebにSSR、public SEO、server componentが必要になった。
- 双方向の高頻度browser protocolが必要になった。
- guildごとの単一Voice outputでは製品要求を満たせなくなった。
- Discord Voiceのprocess移送を無停止にする要件が追加された。
- HRIR storageを複数hostで共有する必要が生じた。

## Primary sources

- [Discord Gateway sharding](https://docs.discord.com/developers/events/gateway)
- [Discord Components reference](https://docs.discord.com/developers/components/reference)
- [Discord OAuth2](https://docs.discord.com/developers/topics/oauth2)
- [Songbird](https://docs.rs/songbird/latest/songbird/)
- [HeSuVi: impulse responseの記録方法と7 channel / 14 IR構造](https://sourceforge.net/p/hesuvi/wiki/How-To%20Record%20Impulse%20Responses%20Digitally/)
- [Axum SSE](https://docs.rs/axum/latest/axum/response/sse/)
- [HTML Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [Valkey Streams](https://valkey.io/topics/streams-intro/)
- [Valkey XREADGROUP](https://valkey.io/commands/xreadgroup/)
- [Valkey XAUTOCLAIM](https://valkey.io/commands/xautoclaim/)
- [Valkey Pub/Sub delivery semantics](https://valkey.io/topics/pubsub/)
- [SQLx migrations](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html)
- [Vite static deployment](https://vite.dev/guide/static-deploy.html)
- [Next.js self-hosting considerations](https://nextjs.org/docs/app/guides/self-hosting)
- [Next.js static export limitations](https://nextjs.org/docs/app/guides/static-exports)
- [Docker Engine on Ubuntu](https://docs.docker.com/engine/install/ubuntu/)
- [Docker Desktop WSL 2](https://docs.docker.com/desktop/features/wsl/)
- [Docker Compose startup order](https://docs.docker.com/compose/how-tos/startup-order/)
- [Caddy automatic HTTPS](https://caddyserver.com/docs/quick-starts/https)
