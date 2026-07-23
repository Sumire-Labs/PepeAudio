# Security Policy

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

Report privately via GitHub Security Advisories:
`Security` tab → `Report a vulnerability` (or the link in the issue picker).

Please include:

- A description of the issue and its impact.
- Steps to reproduce or a proof of concept.
- Affected version/commit.

We aim to acknowledge reports within a few days and to ship a fix or mitigation
as soon as practical, coordinating disclosure with you.

## Scope notes

- PepeAudio invokes `ffmpeg` and `yt-dlp` as separate processes on untrusted
  input; sandboxing (non-root, read-only rootfs, cap-drop) is part of the
  security model — see `docs/blueprint/07-security-config-ops.md`.
- Never commit tokens or secrets. Report exposed credentials privately so they
  can be rotated.
