# PepeAudio-rs 開発規約

このファイルは、リポジトリ全体に適用する実装上の規約です。

## モジュール分割

- 手書きのproduction sourceは、一つの明確な責務だけを持たせる。
- 1ファイルは300行前後を目安とし、400行を超える場合は分割できない理由をレビューする。
- `lib.rs`と`mod.rs`は、原則としてmodule宣言、公開re-export、crate-level documentationに限定する。
- transport、domain、storage、DSP、Discord、HTTPの型を一つのmoduleへ混在させない。
- 公開APIとwire formatは小さなadapter境界に置き、Serenity、SQLx、Valkey等の型をdomainへ漏らさない。
- testが大きくなった場合は`tests/`またはmodule別test fileへ分ける。
- generated source、database migration、fixture、設計文書は行数目安の例外とする。

## 品質境界

- `unsafe`は原則禁止する。
- 外部入力、URL、添付ファイル、HRIR、component ID、database/cache dataを信頼しない。
- build/test成功だけで、Discord実機、音質、低遅延、failoverを検証済みと表現しない。
- realtime audio pathではnetwork、filesystem、database、cache、allocation-heavyな準備処理を行わない。
- SnowflakeはJSONで10進文字列として扱い、JavaScriptの整数精度に依存しない。
- secret、token、password、OAuth credentialをsource、fixture、logへ入れない。

## 変更時の検証

Rust workspaceを変更した場合は、最低限次を実行する。

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Docker、Web、database、audio runtimeを追加した場合は、各層のintegration／E2E手順も追加する。

## Gitと公開

- 明示的な依頼がない限り、commit、push、PR作成、release、外部deployを行わない。
- 既存の関連projectやassetを、ライセンスと移行方針の確認なしにコピーしない。
