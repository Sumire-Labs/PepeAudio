# PepeAudio

PepeAudioは、HeSuViのHRIRを使って音楽を立体的に楽しむDiscord Botです。
Rustで動き、DiscordのコマンドとWebダッシュボードのどちらからでも操作できます。

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

## 導入

Cloudflare Tunnelで公開する場合は、[専用の導入手順](docs/cloudflare-tunnel.md)を使ってください。

## ライセンス

PepeAudioの独自コードと文書は[MIT License](LICENSE)です。依存ライブラリ、FFmpeg、HRIR、再生するメディアには、それぞれのライセンスと利用条件が適用されます。
