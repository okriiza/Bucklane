# Releasing Bucklane

Installers are built by GitHub Actions (`.github/workflows/release.yml`) and attached to a **draft** GitHub release. Nothing is published until you publish the draft.

## 1. Bump the version

The version must match in three files:

- `package.json` → `"version"`
- `src-tauri/Cargo.toml` → `version`
- `src-tauri/tauri.conf.json` → `"version"` (this one names the installers)

In `CHANGELOG.md`, rename **Unreleased** to the new version and date. Commit the change on `main`.

## 2. Tag and push

```bash
git tag v0.1.0
git push origin v0.1.0
```

The tag must start with `v`. Pushing it starts the **Release** workflow, which builds in parallel:

| Runner | Output |
| --- | --- |
| macOS (Apple Silicon) | `Bucklane_<version>_aarch64.dmg`, `.app.tar.gz` |
| macOS (Intel) | `Bucklane_<version>_x64.dmg`, `.app.tar.gz` |
| Windows x64 | `Bucklane_<version>_x64-setup.exe` (NSIS), `Bucklane_<version>_x64_en-US.msi` |
| Linux x64 | `.AppImage`, `.deb`, `.rpm` |

A run takes roughly 15–25 minutes. You can also start it manually from the Actions tab (**Release → Run workflow**).

## 3. Publish

1. Open **Releases** on GitHub and find the draft named `Bucklane v<version>`.
2. Check that every platform's files are attached, including `latest.json` and the `.sig` files, and edit the notes if needed.
3. Click **Publish release**. The files are now downloadable by anyone who can see the repository.

Publishing also releases the update: installed copies of Bucklane read `releases/latest/download/latest.json`, so they see the new version on their next check. A draft is invisible to the updater.

## In-app updates

Bucklane updates itself with the Tauri updater (v0.4.0 and later; older versions must be updated by hand once). Every update package is signed, and the app refuses packages that do not match the public key in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

The release workflow signs with two repository secrets:

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the private key file (e.g. `~/.tauri/bucklane.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The key's password |

The build fails without them, because `bundle.createUpdaterArtifacts` is on.

**Keep a backup of the private key and its password.** If the key is lost, existing installs can no longer verify updates and every user has to reinstall manually with a new key. To rotate keys, generate a new pair with `pnpm tauri signer generate -w ~/.tauri/bucklane.key`, update the secrets and the public key, and ship one release signed with the old key that carries the new public key.

If a build fails, fix it, delete the tag (`git push --delete origin v0.1.0` and `git tag -d v0.1.0`), delete the draft release, and tag again.

## Code signing (not configured yet)

Builds are currently unsigned, so users see a warning on first launch (the release notes explain how to proceed).

- **macOS** signing and notarization need an Apple Developer account. Add the certificate and credentials as repository secrets (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) and pass them to `tauri-action` as environment variables.
- **Windows** signing needs a code-signing certificate configured under `bundle.windows` in `tauri.conf.json`.

See the Tauri distribution guides for details.
