# PepeAudio security patch

This directory starts from the published `hpke-rs` 0.6.1 crate. The original
crate archive has SHA-256
`b6ad6a58eb3e0ee30be8bfc7a9770ae98adcfa1d9bc820a5847732ce84f70837`.

The local manifest makes two deliberately narrow changes:

- `libcrux-sha3` is raised from 0.0.8 to 0.0.10. This is the first release
  containing the fixes for RUSTSEC-2026-0207 and RUSTSEC-2026-0208.
- The optional `hpke-rs-libcrux` provider and its feature references are
  removed. PepeAudio uses only `hazmat` and `serialization`, while retaining
  that unused provider would keep vulnerable Libcrux AEAD packages in
  `Cargo.lock` despite compiling none of them.
- The now-unreachable `libcrux` re-export is removed from `src/lib.rs`. This is
  the only Rust source change; the HPKE implementation itself is unchanged.

All other dependency versions and the 0.6.1 API are preserved. In particular,
this does not take the simultaneous `tls_codec` and crypto-provider upgrades
from the still-unreleased upstream dependency update.

Upstream source: <https://crates.io/crates/hpke-rs/0.6.1>

Pending upstream update: <https://github.com/celabshq/hpke-rs/pull/168>

`hpke-rs` is licensed under MPL-2.0. The full license text is included as
`LICENSE-MPL-2.0`.
