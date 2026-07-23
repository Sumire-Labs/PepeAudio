# PepeAudio

高音質・高速・大規模を目標とする、オープンソースの Discord 音楽 BOT（V2）。
自前の HeSuVi プリセット **「Aura」** による疑似バイノーラル/サラウンドの聴感、
**embed を使わない ComponentsV2 UI**、そして Apple Music 風の **WebGUI** を特徴とする。

> **ステータス: 設計中（Design phase）** — 現在はコード実装前。
> 設計の全体は [`docs/blueprint/`](docs/blueprint/README.md) を参照。

## 特徴（設計目標）

- **ネイティブ音声パイプライン**（Lavalink 非依存）: yt-dlp + FFmpeg。Aura(HeSuVi) を畳み込み、高速スタートで差別化。
- **多プラットフォーム**: YouTube / SoundCloud / 直 URL / 添付ファイル（直接再生）、Spotify / Apple Music（メタデータ解決 → 再生可能ソースへ照合）。
- **キュー内蔵 / プレイリスト**。
- **ComponentsV2 UI**（embed 不使用・デフォルト色）。
- **WebGUI**（Next.js + Turbopack + Tailwind）でサーバー一覧・プレイヤー操作を Discord 同等以上に。
- **初日からシャーディング**（1k → 100k+ サーバーを想定）。
- **PostgreSQL + Valkey**、Docker Compose で Linux 本番を半永久稼働。
- **Apache-2.0** / セキュリティ重視 / サプライチェーン対策。

## コマンド

| コマンド | 説明 |
|---|---|
| `/play <url or file>` | リンクまたは音源ファイルを再生 |
| `/quit` | ボイスチャンネルから退出 |
| `/now` | 音楽プレイヤー（ComponentsV2）を表示 |

## 技術スタック

.NET 10 / Discord.Net 3.20.1 / FFmpeg(LGPL, 別プロセス) / yt-dlp /
PostgreSQL 18 (Npgsql + Dapper) / Valkey / ASP.NET Core + SignalR /
Next.js 16 + React 19 + TypeScript + Tailwind CSS v4。

詳細と選定理由は [`docs/blueprint/00-overview-and-decisions.md`](docs/blueprint/00-overview-and-decisions.md)。

## ドキュメント

- 設計ブループリント: [`docs/blueprint/`](docs/blueprint/README.md)
- セルフホスト手順（実装後に整備）: `docs/self-hosting.md`
- 第三者ライセンス: `docs/THIRD-PARTY-NOTICES.md`
- FFmpeg ライセンス方針: `docs/licensing/FFMPEG.md`

## 設定

トークンや API キー（Discord / Turso→不使用 / Spotify / Apple Music など）は
`config/` テンプレートを埋めて設定する（秘密情報はコミットしない）。
本番は環境変数 + Docker secrets。詳細は [`docs/blueprint/07-security-config-ops.md`](docs/blueprint/07-security-config-ops.md)。

> DISCORD トークン・CLIENT ID/SECRET・Spotify/Apple のキー等は利用者が後から設定する。

## 免責

本 BOT は各配信サイトの利用規約・著作権の遵守を利用者（運用者）の責任とする。
詳細は `DISCLAIMER.md`（整備予定）。

## ライセンス

[Apache License 2.0](LICENSE.md)。
