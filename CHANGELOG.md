# Changelog

Notable changes to Bucklane. Versions follow [Semantic Versioning](https://semver.org/).

## Unreleased

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
