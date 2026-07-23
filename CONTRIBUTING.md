# Contributing to PepeAudio

Thanks for your interest! This project is Apache-2.0 licensed.

## Getting started

```bash
dotnet build
dotnet test
```

See [docs/self-hosting.md](docs/self-hosting.md) to run the bot, and
[docs/blueprint/](docs/blueprint/README.md) for the architecture — the blueprint
is the source of truth for design decisions.

## Ground rules

- **Small, focused files** (roughly ≤150 lines) and **minimal, ordinary comments**
  (no "phase X" or AI-authored comments).
- Every source file starts with `// SPDX-License-Identifier: Apache-2.0`
  (enforced by `.editorconfig` / `dotnet format`).
- **Never commit secrets.** Committed config files hold placeholders only.
- Discord UI uses **Components V2 only** (no embeds), default colors.
- Run `dotnet build` and `dotnet test` before opening a PR; fill in the PR template.

## Commits & DCO

Keep commits scoped and messages descriptive. Contributions are accepted under
the Developer Certificate of Origin — sign off your commits with `git commit -s`.

## Where things live

- `src/PepeAudio.Core` — domain + shared contracts (no dependencies)
- `src/PepeAudio.{Sources,Audio,Data,Cache}` — infrastructure
- `src/PepeAudio.{Discord,Web}` — presentation
- `src/PepeAudio.Host` — composition root
