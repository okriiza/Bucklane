# Security policy

## Supported versions

Only the latest release receives security fixes. Bucklane updates itself from v0.4.0 onward.

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report them privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability**. Include the Bucklane version, your operating system, and steps to reproduce. You will get a reply within a few days, and you will be credited in the release notes unless you prefer otherwise.

## Scope

Especially welcome:

- Ways to read or leak secret access keys or presigned URLs.
- Ways for content from a bucket (object names, metadata, previewed files) to run code or call Bucklane's backend.
- Ways to make Bucklane read or write files outside what the user selected.
- Weaknesses in the update mechanism.

Bucklane's design is described in [docs/tsd.md](docs/tsd.md) (see "Security").
