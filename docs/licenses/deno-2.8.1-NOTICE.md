# Deno 2.8.1 distribution notice

The PepeAudio bot image contains the official Deno 2.8.1 Linux executable.
It is downloaded from the immutable GitHub release and verified with the
SHA-256 digest published by GitHub before it is copied into the image.

Deno is licensed under the MIT License. The exact upstream license from the
`v2.8.1` tag is installed beside this notice as `LICENSE.md`.

The audited `v2.8.1` source tree does not publish a consolidated
`THIRD_PARTY_LICENSES` or `NOTICE` file. Dependency license metadata remains
available in the tag's source and lockfile, and the release workflow publishes
an SBOM for the complete bot image.

- Source: https://github.com/denoland/deno/tree/v2.8.1
- Release: https://github.com/denoland/deno/releases/tag/v2.8.1
- License: https://github.com/denoland/deno/blob/v2.8.1/LICENSE.md
