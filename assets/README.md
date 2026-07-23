# assets

ブランディングと音声リソース。後から差し替え可能。

| フォルダ | 用途 |
|---|---|
| `icons/` | BOT アバター・favicon・モノクロマーク |
| `brand/` | ロゴ・Apple Music 風カラートークン（WebGUI 用） |
| `audio/hesuvi/` | HeSuVi インパルス応答 WAV。既定プリセット `Aura` = `aura.wav` |

## Aura プリセット

`audio/hesuvi/aura.wav` に HeSuVi の true-stereo(4ch) インパルス応答を置く。
チャネル構成（2/4/14ch）は [docs/blueprint/02-audio-pipeline.md](../docs/blueprint/02-audio-pipeline.md) を参照。
改竄検知のため checksum マニフェストの併置を推奨。

> M1 時点では Aura 畳み込みは未実装（素通し）。M2 で afir を有効化する。
