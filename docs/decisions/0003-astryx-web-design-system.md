# ADR 0003: Web UIにAstryx Neutral Themeを採用する

- Status: Accepted
- Date: 2026-08-14

## Context

PepeAudioのWeb dashboardは、guild navigation、再生状態、queue、HRIR、360° Audio、固定playerをdesktopからmobileまで一貫して提供する必要がある。旧UIは独自CSSとSpotifyを参照した視覚表現を持っていたため、component semantics、accessibility、状態表示、design tokenの責務が分散していた。

利用者から、Meta Astryx design systemへ完全に沿ってWeb UIを書き直す明示的な要件が追加された。React 19は既に採用済みで、Astryxのpeer要件を満たす。

## Decision

- `@astryxdesign/core`、`@astryxdesign/theme-neutral`、`@astryxdesign/cli`をexact versionで固定する。
- `@stylexjs/stylex`と公式unpluginをexact versionで固定し、ViteではStyleX pluginをReact pluginより前に置く。
- Astryxのreset、component CSS、Neutral Theme CSSを公式順序で読み込む。
- rootをAstryx `Theme`と日本語`InternationalizationProvider`で包む。
- navigation、layout、list、form、feedback、player controlはAstryxの公開componentを優先する。
- PepeAudio固有のlayoutだけを小さなStyleX moduleへ置き、raw colorやAstryx内部DOM／class名へ依存しない。
- 認証、SSE、command result、wire validationはUI componentから分離し、既存backend契約を維持する。
- Spotifyを含む他サービスの固有UI、asset、商標、文言を複製しない。
- 0.xのAPI driftを自動で受け入れず、upgradeはCLI docs、doctor、型検査、component test、browser QA、license inventoryを同時に更新する。

## Consequences

### Positive

- component semantics、focus、disabled reason、loading stateを共通契約で扱える。
- desktop／mobileで同じ情報構造を保ちやすい。
- design tokenとapp stateの境界が明確になる。
- 独自CSSとブランド模倣の保守負担を減らせる。

### Costs and constraints

- Astryxは0.xであり、minor upgradeでもAPIを再監査する必要がある。
- prebuilt Astryx CSSは一定のbundle sizeを持つため、React／Astryx／iconを分割して配信する。
- `style-src 'unsafe-inline'`は現在のruntime style契約で必要であり、Astryx／StyleX側の変更時にCSPを再評価する。
- AstryxとStyleXのlicense noticeをWeb artifactとOCI imageへ同梱する。

## Verification

- `pnpm exec astryx doctor`
- `pnpm check`
- `pnpm test`
- `pnpm build`
- 320、360、1024、1440pxのbrowser QA
- keyboard navigation、landmark、accessible name、focus returnの確認
- Caddy image内のasset、CSP、cache policy、license notice確認

## References

- [Astryx Getting Started](https://astryx.atmeta.com/docs/getting-started)
- [Astryx source](https://github.com/facebook/astryx)
- [StyleX Vite integration](https://stylexjs.com/docs/learn/installation/vite/vite-react)
- [React reference](https://react.dev/reference/react)
