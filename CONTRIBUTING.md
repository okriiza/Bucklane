# Contributing to Bucklane

Thanks for your interest! Bug reports, ideas, and pull requests are welcome.

## Before you start

- For anything larger than a small fix, open an issue first so we can agree on the approach.
- Security problems go through the private process in [SECURITY.md](SECURITY.md), not public issues.

## Development setup

See [For developers](README.md#for-developers) in the README for prerequisites, running the app, and the S3 integration tests.

## Making a change

1. Branch from `main`.
2. Keep the existing architecture: commands → services → providers/repositories in Rust, Pinia stores and `vue-i18n` strings in the frontend. Secrets never leave the system keychain, and the frontend never sends raw file paths (see [docs/tsd.md](docs/tsd.md)).
3. Add or update tests for behavior you change.
4. Run every check before opening a pull request:

   ```bash
   pnpm lint && pnpm format:check && pnpm typecheck && pnpm test && pnpm build
   ```

   ```bash
   cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
   ```

   If you change a Rust command or type used by the frontend, commit the regenerated `src/services/tauri/bindings.ts` too (`cargo test` regenerates it).

5. Write commit messages in the [Conventional Commits](https://www.conventionalcommits.org/) style: `feat: …`, `fix: …`, `docs: …`, `chore: …`.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
