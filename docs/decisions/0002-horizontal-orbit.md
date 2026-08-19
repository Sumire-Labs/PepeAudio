# ADR 0002: HeSuViによる水平360° Audio

- Status: Accepted for MVP
- Date: 2026-08-13
- Scope: Discord Voiceへ送るguild共通の空間音響

## Context

HeSuVi互換WAVは、水平面上の7つの仮想スピーカー方向について左右耳へのimpulse responseを提供する。高さ方向や任意azimuthを連続queryできるHRTF surfaceではない。またDiscordの一つのVoice接続から送信できるのは完成済みの一つのstereo streamであり、listenerごとに別のHRIRや方向を送ることはできない。

製品には分かりやすい360° Audio toggleが必要だが、固定前方のHRIRを単にon/offするだけでは名称と実動作が一致しない。一方、初期MVPでSOFA、head tracking、listener別streamまで導入すると、データ契約と運用範囲が大きく変わる。

## Decision

初期の「360° Audio」は、次のguild共通・水平orbitとして定義する。

- stereo入力の左・右channelを、中心角に対してそれぞれ+30°／-30°の60°幅pairとして配置する。
- 曲の先頭ではpair中心を正面0°とする。
- pair中心は60秒で時計回りに一周する。
- PCMの処理frame数をclockにする。wall clockは使わず、pause中は位相を進めない。
- seek後は曲位置を60秒で剰余し、同じ曲位置なら同じ方向へ復元する。
- toggleをoffにしている間もdecode済みPCM frameに従って位相と入力履歴を進め、onへ戻したとき現在の曲位置から復帰する。
- 各azimuthはHeSuViの隣接する水平anchor二つをequal-powerで補間する。±180°の境界も同じ規則でwrapする。
- HRIR preset、toggle、orbit位相はguild単位であり、同じVoice Channelの全listenerが同じ出力を聴く。
- UIには「水平7方向を補間したguild共通の音場」と表示し、高さ付き3Dやhead trackingとは表現しない。

実時間rendererは256-frame uniform partitionとoverlap-saveを使い、sourceごとに一つのfrequency-domain delay lineを共有する。現在の隣接2方向のIR spectrumをmixして左右耳を逆変換するため、方向を跨いでも全方向の過去入力はwarmなままで、14個の独立した入力FFTを常時処理しない。partial blockは現在までの入力とzero paddingで再計算して当該sampleだけを確定するため、partition一個分のalgorithmic latencyを追加しない。完全dryでtransition中でない場合は逆変換を省略するが、入力spectrum履歴は更新する。

## Safety and performance boundary

Direct time-domain FIRを数値oracleとして残し、production rendererはshared-FDL partitioned convolutionを使う。Windows 11 reference hostのrelease測定では、10秒・960-frame blockの単一orbitを9,600 tapsで248.9x realtime処理した。production catalogは48 kHzで200 msに相当する9,600 prepared framesを上限としてfail closedし、44.1 kHz assetもresample後に再検査する。preset crossfadeはrendererを二つ動かすため、単一guild測定はmulti-guild容量の保証ではない。最終PCMは有限値検査と安全上限を通す。音響的なheadroom、定位、click、CPU、underrunは実HRIRを使った測定と聴感試験をrelease条件として別に記録する。

## Consequences

### Positive

- 360° toggleが実際の方向移動を制御し、UI名称とDSP pathが一致する。
- track位置から位相を復元でき、pause、seek、Web再接続でwall-clock driftを持ち込まない。
- HeSuVi assetだけで水平移動を提供しつつ、元dataset以上の高さ情報があるとは主張しない。
- shared FDLによりanchor切替のcold history、方向ごとの重複forward FFT、全7方向の常時inverse FFTを避けられる。

### Negative

- 出力はguild共通で、listenerの向きや位置を反映しない。
- 原音のstereo image全体が回転するため、楽曲によっては意図的でない定位変化に聞こえる。
- 補間は測定された連続HRTFではなく、音色や定位の自然さを保証しない。
- partitioned backendでもIR長、crossfade中の倍負荷、同時wet guild数に運用上限がある。

## Rejected for the initial MVP

- 固定前方HRIRのon/offだけを360° Audioと呼ぶ。
- 7.1入力を必須にする。通常のmusic sourceはstereoであり、入力要件が変わる。
- listener別HRIR、head tracking、elevationをHeSuViだけで提供する。
- wall clockでorbitを進め、pauseやprocess stallの間にも位相を変える。

## Follow-up acceptance

1. 権利を確認した実HRIRでfront、side、rear、wrapを録音・聴感確認する。
2. toggleとpreset transitionのpeak、click、NaN、hard-limit発生回数を測定する。
3. production上限のHRIRでguildあたりCPU、block deadline、underrunを測定する。
4. 複数guild同時再生の許容数をUbuntu Server 26.04 target hostで決める。
5. 高さ付き連続HRTFが必要になった場合は、SOFA等のasset contractと別backendを新しいADRで定義する。
