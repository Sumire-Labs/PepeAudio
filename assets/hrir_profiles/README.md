# HRIRプロファイル（bring-your-own）

このフォルダに`.wav`形式のインパルスレスポンス（HRIR/BRIR）ファイルを1つ置くと、再生中の全トラックに自動的に適用されます。**選択メニューなどはありません** — 常時オン、ファイルを差し替えるにはBotの再起動が必要です。ファイルは同梱されません — 各自で用意してこのフォルダに配置してください（`.gitignore`でこのフォルダ内の`.wav`はコミット対象外になっています）。

## 対応フォーマット

起動時に各`.wav`のチャンネル数を自動判定し、対応する2種類のどちらかとして扱います。

- **モノラル/ステレオ2ch** — そのまま`afir`で畳み込みます
- **HeSuVi形式の14ch** — 本家HeSuVi(`C:\Program Files\EqualizerAPO\config\HeSuVi\hrir`配下)の"reverbあり"ファイル(例: `atmos.wav`, `dht.wav`)と同じ形式。実機で検証済みのチャンネル配置(フロントL/Rスピーカーのtrue stereoペアのみ抽出)で処理します。詳細は`src/audio/ffmpegFilters.ts`の`HRIR_HESUVI14_FILTER_COMPLEX`を参照
- **上記以外のチャンネル数は非対応**(起動ログに警告が出て、そのファイルはスキップされ、アルファベット順で次の対応ファイルが探されます)。特にHeSuViの"No Reverb"(`-`が付くファイル、例: `atmos-.wav`)は7chの別形式で、今のところ未対応です

複数ファイルを置いた場合、**アルファベット順で最初に見つかった対応チャンネル数のファイル1つだけ**が使われます（選択メニューはないため、それ以外のファイルは無視されます）。

## 配置場所

デフォルトは `assets/hrir_profiles/`(このフォルダ)です。別の場所に置きたい場合は `.env` の `HRIR_PROFILES_DIR` で上書きできます。

## 入手元・プリセット一覧

本家HeSuVi(Equalizer APOのアドオン、オーディオ愛好家コミュニティで長年配布されている無料ツール)を導入すると、上記の`hrir`フォルダにプリセット一式が入っています。

- [HeSuVi (SourceForge、配布元)](https://sourceforge.net/projects/hesuvi/files/) — 「Download Latest Version」で入手
- [Equalizer APO](https://equalizerapo.sourceforge.io) — HeSuViの前提として先にインストールが必要

`hrir`フォルダ内の`info.csv`に、`atmos`→`Dolby Atmos 7.1 virtual surround sound for headphones`のような短縮名↔正式名の対応表が入っています。主なプリセット系統:

- **Dolby系**: `atmos`(Dolby Atmos) / `dh+`/`dh++`(Dolby Headphone) / `dht`(Dolby Home Theater v4)
- **OpenAL/DirectSound3D系**: `oal+`/`oal++`/`oal+++`、`ds3d`系 — ルーム特性違い(Room/Livingroom/Genericなど)
- **サウンドカード/コンソール系**: `cmss_ent`/`cmss_game`(Creative CMSS-3D)、`sbx33`/`sbx67`/`sbx100`(SBX Pro Studio)
- **OS/プラットフォーム標準**: `sonic`(Windows Sonic)、`dtshx`(DTS Headphone:X)
- **ヘッドセットベンダー系**: `gsx`(Sennheiser GSX)、`razer`(Razer Surround)
- **サードパーティ系**: `waves`(Waves Nx)、`ooyh0`/`ooyh1`(Out Of Your Head)、`hear`(Flux HEar V3)

これらの多くはDolby/Waves等の商用製品の出力を有志が録音・逆算した非公式データとして、オーディオ愛好家コミュニティで共有されています。当プロジェクトはこれらのファイルをダウンロード・配布・同梱しません。個人利用の範囲で自己責任にて入手・配置してください。
