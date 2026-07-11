# PepeAudio

[![License: LGPL v3](https://img.shields.io/badge/license-LGPL--3.0-blue.svg)](LICENSE.md)
[![Node.js](https://img.shields.io/badge/node-%3E%3D22.12.0-brightgreen.svg)](package.json)
[![discord.js](https://img.shields.io/badge/discord.js-v14-5865F2.svg)](https://discord.js.org)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.7-3178C6.svg)](https://www.typescriptlang.org/)
[![pnpm](https://img.shields.io/badge/pnpm-managed-F69220.svg)](https://pnpm.io)
[![Docker Ready](https://img.shields.io/badge/docker-ready-2496ED.svg)](docker-compose.yml)

discord.js製の音楽Bot。Components V2で構築され、
HRIR Audio と Spotify/SoundCloud/YouTube/Apple Music 対応を備える。

## セットアップ

1. `pnpm install`
2. `pnpm run setup-ffmpeg` — sofalizer(libmysofa)対応のffmpegバイナリを
   `bin/` に配置します(下記の対応表で確認済みのプラットフォームのみ自動化)。
   実行しなくても問題なく、その場合は3Dオーディオが簡易版のフィルターチェイン
   にフォールバックします。
3. `.env.example` を `.env` にコピーし、`DISCORD_TOKEN` / `CLIENT_ID` を入力
   (開発中は `GUILD_ID` も設定するとギルド限定でコマンドが即反映されます)。
4. `pnpm run dev`(`tsx`でTypeScriptを直接実行)、または
   `pnpm run build && npm start`(コンパイル後に実行)。
   スラッシュコマンド(`/play`等)はBot起動時に自動で登録されるので、
   別途登録コマンドを実行する必要はありません。

## Docker
### 公開運用向け

同梱の `docker-compose.yml` は、非root実行・DBの永続化・トークンのファイル
マウント(env変数に載せない)・再起動ポリシーをまとめて設定します。

```
mkdir -p secrets
printf '%s' 'YOUR_BOT_TOKEN' > secrets/discord_token.txt   # secrets/ はgitignore対象
chmod 600 secrets/discord_token.txt
echo 'CLIENT_ID=あなたのアプリID' > .env                   # CLIENT_IDは公開値(秘密ではない)
docker compose up -d --build
```

- **トークン**は `secrets/discord_token.txt` からファイルとして注入されます
  (`DISCORD_TOKEN_FILE`)。env変数(`docker inspect` や `/proc` から覗ける)には載りません。
- **ギルド設定(SQLite)** は名前付きボリューム `pepeaudio-data`(`/data`)に永続化され、
  再デプロイしても消えません。単発の `docker run` で永続化したい場合は
  `-e DATA_DIR=/data -v pepeaudio-data:/data` を付けてください。


## クレジット
- Dolby ー Dolby Home Theater V4の実装
- Discord.js ー 彼らのフレームワークがなければ、このBOTは今頃絡まりあった縄跳びのようになっていたでしょう
- Claude ー BOTのHRIR/HRTFアルゴリズムの開発に尽力してくれました。
- ffmpeg ー 彼らのツールがなければ今頃このBOTは存在しなかったでしょう。


## ライセンス

[LICENSE.md(LGPL-3.0)](LICENSE.md)を参照してください。