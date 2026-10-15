# Bucklane

Local-first desktop manager for Amazon S3 and S3-compatible object storage, built with Tauri 2, Vue 3, and Rust.

See [docs/prd.md](docs/prd.md), [docs/tsd.md](docs/tsd.md), and [docs/database_design.md](docs/database_design.md).

## Prerequisites

- Node.js 22+ and pnpm
- Rust (stable, via rustup)
- Platform dependencies from the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
- Linux: a Secret Service provider (GNOME Keyring or KWallet) for credential storage

## Development

```bash
pnpm install
pnpm tauri dev
```

TypeScript IPC bindings (`src/services/tauri/bindings.ts`) are generated from Rust on every debug run and by `cargo test`. Commit the regenerated file; CI fails if it is out of date.

## Checks

```bash
pnpm lint && pnpm format:check && pnpm typecheck && pnpm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Releasing

Push a version tag (for example `v0.1.0`) to build macOS, Windows, and Linux installers into a draft GitHub release. See [docs/releasing.md](docs/releasing.md).
