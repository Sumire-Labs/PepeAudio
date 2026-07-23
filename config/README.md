# 設定

秘密情報はコミットしない。ここにあるのはプレースホルダのみ。

## ファイル

| ファイル | 用途 |
|---|---|
| `.env.example` | `.env` にコピーして値を埋める（docker-compose が読む） |
| `appsettings.json` | 非秘密の既定値 |
| `appsettings.Development.json` | 開発時の上書き（localhost DB 等） |
| `appsettings.Production.json` | 本番トグル（秘密は env / Docker secrets から） |
| `secrets/` | Docker secrets のマウント先（gitignore 済み） |

## 設定の優先順位（後が勝つ）

`appsettings.json` → `appsettings.{Environment}.json` → user-secrets(Dev のみ) → 環境変数 → `/run/secrets`(AddKeyPerFile)

- 環境変数は二重アンダースコアで階層化: `DISCORD__TOKEN` → `Discord:Token`。
- 本番の秘密は環境変数か Docker secrets で注入する。`appsettings.*` には実値を書かない。

## 主なキー

- `Discord:Token` / `Discord:ClientId` / `Discord:ClientSecret`
- `Discord:TotalShards`（全コンテナで同一）/ `Discord:ShardIds`（コンテナ毎）
- `ConnectionStrings:Postgres` / `ConnectionStrings:Valkey`
- `Audio:*`（プリセット・ffmpeg/yt-dlp パス・同時 voice 数・バッファ）
- `Sources:*` / `Security:*` / `RateLimit:*`

詳細は [docs/blueprint/07-security-config-ops.md](../docs/blueprint/07-security-config-ops.md)。
