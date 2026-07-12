# PepeAudio

[![License: LGPL v3](https://img.shields.io/badge/license-LGPL--3.0-blue.svg)](LICENSE.md)
[![Node.js](https://img.shields.io/badge/node-%3E%3D22.12.0-brightgreen.svg)](package.json)
[![discord.js](https://img.shields.io/badge/discord.js-v14-5865F2.svg)](https://discord.js.org)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.7-3178C6.svg)](https://www.typescriptlang.org/)
[![pnpm](https://img.shields.io/badge/pnpm-managed-F69220.svg)](https://pnpm.io)
[![Docker Ready](https://img.shields.io/badge/docker-ready-2496ED.svg)](docker-compose.yml)

**Aura Sounds System** を核に据えた discord.js 製の音楽Bot。Components V2 の再生パネルから、
頭外定位のバイノーラル3Dオーディオ（**Aura HRIR**）と、立体感・重低音のエンハンサー（**Aura 360°**）を
切り替えられます。Spotify / Apple Music / SoundCloud / YouTube のリンク・検索に対応。

---

## 主な機能

### 再生
- **対応ソース** — Spotify / Apple Music / SoundCloud / YouTube のリンク、または検索ワード（`/play`）
- **キュー** — 最大500曲、プレイリスト取り込み（最大50曲）、履歴50曲、前の曲へ戻る
- **ループ / シャッフル** — ループ（オフ・1曲・キュー全体）、シャッフル
- **オートプレイ** — キューが尽きたら関連曲を自動で継続
- **音量** — 0〜100%（5%刻み、デフォルト50%）
- **24/7モード** — 無人・キューが空でも自動退出しない（`/stay`）
- **操作パネル** — Components V2 のボタン＋セレクトメニュー。誰が操作できるかはサーバーごとに設定可能
  （同じVCの全員／DJロール保持者のみ／曲をリクエストした人のみ）
- 無人時60秒・キュー枯渇5分で自動退出

### Aura Sound System（3Dオーディオ）
パネルから独立してトグルできる2つのエフェクト＋プリセット選択を備えます。

- **Aura HRIR**（デフォルト: オン） — BRIR畳み込み（ffmpeg `afir`）によるバイノーラル・バーチャルサラウンド。
  フロント＋サイドの仮想スピーカー構成で音像を頭の外へ押し出し、プロファイルごとに自動でレベルマッチします。
- **Aura 360°**（デフォルト: オフ） — 畳み込みとは別系統の、ステレオ幅拡大＋重低音＋初期反射による
  遠近感エンハンサー（立体感・没入感）。
- **Aura プリセット** — 複数のBRIR/HRIRプロファイルをパネルのセレクトメニューから切り替え（曲を止めずに反映）。

> ### ⚠️ Aura Sound System について
> コア機能である **Aura Sound System は Sumire Labs の独自開発による非公開システムです。**
> その中核となるチューニングおよび Aura プリセット（測定済みのBRIR/HRIRプロファイル `.wav`）は、
> 本リポジトリには**含まれておらず、配布もされません**。この公開リポジトリで提供されるのは、
> Bot 本体（再生・操作パネル・各ソース連携）と、各自が用意した BRIR/HRIR ファイルを読み込むための
> 仕組みのみです。

## コマンド

| コマンド | 説明 |
| --- | --- |
| `/play <query>` | リンクまたは検索ワードを再生（Spotify / Apple Music / SoundCloud / YouTube） |
| `/skip` | 現在の曲をスキップ |
| `/stop` | 再生を停止してVCから退出 |
| `/quit` | VCから強制退出（24/7モード中でも退出） |
| `/stay <enabled>` | 24/7モードの切り替え |
| `/now show` | 再生パネルをこのチャンネルに表示 |
| `/settings show` | 現在の操作権限設定を表示 |
| `/settings permission <mode> [dj_role]` | 誰がBotを操作できるかを設定 |

スラッシュコマンドはBot起動時に自動登録されます（`GUILD_ID` を設定するとギルド限定で即時反映、
未設定ならグローバル登録）。

---

## セットアップ

1. `pnpm install`
   （`better-sqlite3` はネイティブモジュールのため、環境によっては新しめの Node LTS が必要な場合があります）
2. **（任意）** `pnpm run setup-ffmpeg` — フル機能の ffmpeg バイナリを `bin/` に配置します。
   Aura Sound System は ffmpeg の `afir` フィルターで動作し、これは同梱の `ffmpeg-static` にも含まれるため、
   このステップは**必須ではありません**（実行しない場合は `ffmpeg-static` を使用）。
3. `.env.example` を `.env` にコピーし、`DISCORD_TOKEN` / `CLIENT_ID` を入力
   （開発中は `GUILD_ID` も設定するとコマンドが即反映されます）。
4. 起動：
   - 開発： `pnpm run dev`（`tsx` でTypeScriptを直接実行）
   - 本番： `pnpm run build && pnpm start`（コンパイル後に実行）

---

## Docker（公開運用向け）

同梱の `docker-compose.yml` は、非root実行・DBの永続化・トークンのファイルマウント
（env変数に載せない）・再起動ポリシーをまとめて設定します。ffmpeg はイメージ内でソースからビルドされます。

```sh
mkdir -p secrets
printf '%s' 'YOUR_BOT_TOKEN' > secrets/discord_token.txt   # secrets/ はgitignore対象
chmod 600 secrets/discord_token.txt
echo 'CLIENT_ID=あなたのアプリID' > .env                   # CLIENT_IDは公開値（秘密ではない）
docker compose up -d --build
```

- **トークン**は `secrets/discord_token.txt` からファイルとして注入されます（`DISCORD_TOKEN_FILE`）。
  env変数（`docker inspect` や `/proc` から覗ける）には載りません。
- **ギルド設定（SQLite）** は名前付きボリューム `pepeaudio-data`（`/data`）に永続化され、
  再デプロイしても消えません。

---

## 環境変数

| 変数 | 必須 | 説明 |
| --- | --- | --- |
| `DISCORD_TOKEN` | ● | Botトークン（本番は `DISCORD_TOKEN_FILE` でファイル注入を推奨） |
| `CLIENT_ID` | ● | アプリケーションID（公開値） |
| `GUILD_ID` | | 設定するとギルド限定でコマンドを即時登録（開発用） |
| `LOG_LEVEL` | | ログレベル（既定 `info`） |
| `DATA_DIR` | | SQLite DBの保存先（Dockerではマウントボリュームを指定） |
| `FFMPEG_PATH` | | ffmpegバイナリのパス（未設定なら `bin/` → `ffmpeg-static` の順に解決） |
| `HRIR_PROFILES_DIR` | | 自前のBRIR/HRIR `.wav` を置くフォルダ（既定 `assets/hrir_profiles/`） |

---

## 技術スタック

TypeScript 5.7 / discord.js v14 / @discordjs/voice / better-sqlite3（SQLite）/ ffmpeg（`afir`）/ pnpm

---

## クレジット

- **discord.js / @discordjs/voice** — Bot本体と音声再生の基盤
- **ffmpeg** — `afir` 畳み込みをはじめ、音声処理全般を支える立役者
- **HeSuVi / Equalizer APO コミュニティ** — 14ch BRIRフォーマットの参考
- **Claude（Anthropic）** — Aura Sound System のDSP設計・実装のペアプロ相手

---

## ライセンス

[LICENSE.md（LGPL-3.0）](LICENSE.md) を参照してください。