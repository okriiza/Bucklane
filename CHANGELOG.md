# Changelog

Notable changes to Bucklane. Versions follow [Semantic Versioning](https://semver.org/).

## 0.6.0 — 2026-10-22

### Added

- Rename, copy, and move **folders**, from the menu, with F2, or by dragging a folder onto another folder or the breadcrumb. Bucklane counts the folder first, shows existing objects at the destination (skip or replace), reports progress, and can be stopped. Originals are deleted only after their copies are verified.
- **Support Bucklane** in the sidebar and on the About page opens the project's Trakteer page. It is a plain link: nothing pops up, and nothing is tracked.

### Changed

- Smoother drag and drop in the explorer: no text gets selected, a card follows the pointer, the target folder opens as you hover it, and the item flies into the folder when you drop it.

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
