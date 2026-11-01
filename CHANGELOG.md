# Changelog

Notable changes to Bucklane. Versions follow [Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- **What's new.** After an update, Bucklane shows what changed in the new version once; open it again from **About → What's new**. **About → Updates** also shows the new version's changes before you install it.
- **Copy to another bucket or provider.** Copy files or whole folders to any bucket on the same or another connection, for example from Amazon S3 to Cloudflare R2, from the **⋯** menu (**Copy to another bucket…**) or for several selected items (**Copy to…**). Copies run in the transfer queue with progress, cancel, and retry; they are streamed from source to destination without touching your disk, keep their folder structure, content type, and metadata, and ask before replacing anything.

## 0.8.0 — 2026-11-01

### Added

- **Public URLs.** **Copy public URL** in a file's **⋯** menu copies its unsigned address, and the share dialog has a **Public URL** tab next to temporary links. Set a custom domain or CDN (CloudFront, r2.dev, your own domain) per bucket and every public URL uses it. Supabase public URLs use its public object path; Cloudflare R2 asks for the bucket's public domain.
- **Edit metadata.** Change a file's Content-Type, Cache-Control, Content-Disposition, Content-Encoding, Content-Language, and custom metadata without uploading it again. The contents stay the same. Storage class, encryption, and a public-read ACL are kept, and nothing is saved if the file changed on the server while you were editing.
- The details panel shows Cache-Control, Content-Disposition, Content-Encoding, and Content-Language when they are set.

### Changed

- Connections show their provider's logo (Supabase, MinIO, Cloudflare, Wasabi, DigitalOcean) instead of a two-letter monogram. Amazon S3 gets an orange bucket, and custom endpoints a server icon.

## 0.7.0 — 2026-11-01

### Added

- **Resumable downloads.** A download cut off by a dropped connection, closing Bucklane, or a shutdown keeps its partial file. **Resume** continues from the last safe point (saved every few seconds) instead of starting over, and starts over only if the file changed on the server. Optionally resume automatically when Bucklane starts (Settings → Transfers).
- **Pause downloads** and resume them later; a paused download keeps its partial file and survives "Clear finished".
- Canceled transfers can be started again with **Download again** / **Upload again**.

## 0.6.0 — 2026-10-22

### Added

- Rename, copy, and move **folders**, from the menu, with F2, or by dragging a folder onto another folder or the breadcrumb. Bucklane counts the folder first, shows existing objects at the destination (skip or replace), reports progress, and can be stopped. Originals are deleted only after their copies are verified.
- **Support Bucklane** in the sidebar and on the About page opens the project's Trakteer page. It is a plain link: nothing pops up, and nothing is tracked.

### Changed

- Smoother drag and drop in the explorer: no text gets selected, a card follows the pointer, the target folder opens as you hover it, and the item flies into the folder when you drop it.

### Security

- Updated `ctutils` to 0.4.5 ([RUSTSEC-2026-0385](https://rustsec.org/advisories/RUSTSEC-2026-0385)): non-constant-time comparisons in a crate used by the S3 request signer and the Linux keyring store.

## 0.5.2 — 2026-10-22

### Security

- Updated `rustls-webpki` and `webpki-root-certs` so TLS connections to storage providers and the update server no longer trust distrusted certificate authorities ([RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382), [RUSTSEC-2026-0383](https://rustsec.org/advisories/RUSTSEC-2026-0383)).

## 0.5.1 — 2026-10-19

### Changed

- Folders load completely in the background (up to 10,000 items), so subfolders always appear at the top and sorting and filtering cover the whole folder, not just the first page.

## 0.5.0 — 2026-10-19

### Security

- Previews choose how to render a file from its kind, never from the object's stored `Content-Type`, so a file disguised as a PDF or image cannot load as a web page.
- Previews stop reading at the size limit even if the server reports no size or ignores range requests.
- The app window can no longer be navigated away from Bucklane's own pages.
- Folder downloads refuse to follow a symbolic link that would place files outside the chosen folder.
- Windows device names such as `CON` or `NUL` are renamed when downloading.
- The updater no longer pulls in the `zip` crate, which had known advisories.
- CI audits Rust and production JavaScript dependencies on every change.

### Added

- **About → Report a problem**: copy a diagnostics summary (no connection names, endpoints, buckets, files, or keys) and open the logs folder.
- Unexpected interface errors are written to the log file, and a failing screen shows a recovery page instead of going blank.
- MIT license, security policy, contributing guide, and issue templates.

## 0.4.0 — 2026-10-18

### Added

- In-app updates: Bucklane checks GitHub Releases on startup (can be turned off in Settings), then downloads, verifies, and installs signed updates from **About → Updates**.

## 0.3.0 — 2026-10-18

### Added

- Share links (presigned URLs valid from 1 minute to 7 days) and quick copy of an object's key or `s3://` URI.
- Upload and download whole folders, with per-file conflict handling.
- Preview images, PDFs, and text files.
- Sort by name, size, or last modified, and search a whole folder on the server by name prefix.
- Transfer speed and time remaining, and **Show in Finder / File Explorer** for finished transfers.

### Changed

- Clearer messages for an unknown access key, a rejected signature (region, Supabase `stub` region, proxies), and clock skew.

### Fixed

- Uploaded folders appear without a manual refresh.

## 0.2.0 — 2026-10-15

### Changed

- Refreshed interface with a home dashboard, plus a rewritten README with screenshots.

## 0.1.0 — 2026-10-15

### Added

- First release: connections to any S3-compatible provider with secrets in the system keychain, bucket browsing, uploads and downloads with multipart and a transfer queue, copy, move, rename, delete, bookmarks, and settings.
