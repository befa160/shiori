# Contributing

Shiori is a "clean room" rewrite: never copy nor take inspiration from Kavita's code (GPL). The complete rules are in [CLAUDE.md](https://github.com/befa160/shiori/blob/main/CLAUDE.md), the living roadmap in [ROADMAP.md](https://github.com/befa160/shiori/blob/main/ROADMAP.md) (both in French).

## Requirements

- Stable Rust (pinned by `backend/rust-toolchain.toml`, installed by `rustup`).
- Current Node.js (`web/.node-version`, 26 today).

## Commands

Backend (from `backend/`):

```bash
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p shiori -- serve
```

Front end (from `web/`):

```bash
npm ci
npm run dev      # Vite, with a `shiori serve` running next to it
npm run build    # type checking included
npm run lint
npx vitest run --coverage
npm run e2e      # Playwright against a real server (once: npx playwright install chromium)
```

Documentation (from `docs/`):

```bash
npm ci
npm run dev      # http://localhost:5173/shiori/
npm run build
```

## Working rules

- One branch and one PR per batch (`feat/…`, `fix/…`, `docs/…`…), never a direct commit on `main`.
- Before each commit: clippy without warnings, clean `cargo fmt`, green tests, front-end build.
- Coverage of at least 80% of lines and branches, Rust and front end; a branch that is hard to reach is tested anyway.
- Always the latest stable versions of dependencies.
- Interface: no hard-coded text (`fr.ts` and `en.ts` catalogs), never a hard-coded color, screenshots at four sizes.
- Documentation: every page exists in French (root) and English (`docs/en/`).

## Releasing

A `vX.Y.Z` tag on `main` starts `release.yml`: binaries, archives, `SHA256SUMS`, `ghcr.io/befa160/shiori` Docker image. The documentation is published at each merge into `main` by `docs.yml`.
