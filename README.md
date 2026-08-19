# PepeAudio

PepeAudioは、HeSuViのHRIRを使って音楽を立体的に楽しむDiscord Botです。
Rustで動き、DiscordのコマンドとWebダッシュボードのどちらからでも操作できます。

いまは最初の実機テストに入る段階です。自動テストとLinuxコンテナでの結合テストは通していますが、Discord Voice、DAVE、実HRIRの聴感、Ubuntu Server 26.04での長時間運転はまだ確認中です。

## できること

- YouTube／SoundCloudの曲・プレイリスト、URL、添付ファイルを`/play`でキューへ追加
- Spotify／Apple Musicの曲情報をYouTube／SoundCloudの安全な一致候補へ変換（任意設定）
- pause、seek、skip、repeat、shuffle、音量調整
- HeSuVi互換の7ch／14ch HRIRプリセットを切り替え
- 水平方向を60秒で一周する360° Audio
- 再生状態をDiscord Components V2とWebへリアルタイム反映
- 5分間何も再生しなければVoice Channelから自動退出
- DiscordのステータスにBot本体のメモリ使用量を表示
- PostgreSQLへの設定保存、Valkeyを使ったsession・command配送
- Discord Gateway shardingとDocker Compose運用

Discordの返答にはEmbedを使いません。Web UIはMeta AstryxのNeutral Themeで作っています。

## 試す前に

次のものが必要です。

- Rust 1.97
- Node.js 24とpnpm 11.3
- FFmpeg／ffprobe
- YouTubeを試す場合は、同梱済みyt-dlp／Denoを含むbot image
- Docker EngineまたはDocker Desktop
- テスト用Discord ApplicationとGuild
- Discord Bot token
- 利用条件を確認したHeSuVi互換HRIR WAV

HRIRデータは同梱していません。対応形式やDiscord実機テストの手順は[運用ガイド](docs/operations.md)を参照してください。

## 開発

Rust workspaceを確認します。

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

ダッシュボードを確認します。

```text
cd web
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
```

WindowsとLinux向けの一括検証は`.\scripts\verify.ps1`と`sh scripts/verify.sh`にまとめています。

## コンテナとリリース

`v0.1.0`のようなタグをpushすると、GitHub Actionsがテストをやり直し、GitHub Releaseと次のGHCR imageを公開します。

```text
ghcr.io/<owner>/pepeaudio-bot:0.1.0
ghcr.io/<owner>/pepeaudio-api:0.1.0
ghcr.io/<owner>/pepeaudio-migrate:0.1.0
ghcr.io/<owner>/pepeaudio-caddy:0.1.0
```

各imageは`linux/amd64`と`linux/arm64`向けです。Releaseにはdigest一覧が付き、imageにはSBOMとbuild provenanceを付与します。初回公開後は、GitHub Packages側で各packageを`Public`へ変更するまで匿名の`docker pull`はできません。

Composeでは次の環境変数で公開imageへ差し替えられます。

- `PEPEAUDIO_BOT_IMAGE`
- `PEPEAUDIO_API_IMAGE`
- `PEPEAUDIO_MIGRATE_IMAGE`
- `PEPEAUDIO_CADDY_IMAGE`

タグの作り方、公開範囲、attestationの確認方法は[運用ガイド](docs/operations.md)に記載しています。

## 知っておきたいこと

360° AudioはHeSuViの水平7方向を補間する機能です。高さ方向、頭部追跡、ユーザー別HRTFには対応していません。同じVoice Channelの全員が同じステレオ出力を聴きます。

サイト連携は初期状態では無効です。YouTube／SoundCloudはoperatorが明示的に有効化すると使えます。Spotifyはtrackとalbum、Apple Musicはcatalog URLを任意設定で照合できますが、Spotify playlistはClient Credentialsでは取得できないため未対応です。cookie、ユーザーOAuth、DRM回避は行いません。

## ドキュメント

- [運用・検証ガイド](docs/operations.md)
- [製品要求](docs/product-requirements.md)
- [アーキテクチャ](docs/architecture.md)
- [水平360° Audioの設計](docs/decisions/0002-horizontal-orbit.md)
- [Astryx採用方針](docs/decisions/0003-astryx-web-design-system.md)
- [第三者ソフトウェアとライセンス](docs/third-party.md)

## ライセンス

PepeAudioの独自コードと文書は[MIT License](LICENSE)です。依存ライブラリ、FFmpeg、HRIR、再生するメディアには、それぞれのライセンスと利用条件が適用されます。
