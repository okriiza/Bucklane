# Design documents

These are the documents Bucklane was planned and built from. They explain why the app works the way it does, especially around security. Read them before making a larger change.

| Document | What it covers |
| --- | --- |
| [prd.md](prd.md) | Product requirements: goals, scope, and user-facing behavior as originally planned. |
| [tsd.md](tsd.md) | Technical specification: architecture, IPC commands, security model, transfer engine, updates. Kept up to date as features land. |
| [database_design.md](database_design.md) | The local SQLite schema and what is (and is never) stored in it. |
| [releasing.md](releasing.md) | How to cut a release, sign updates, and publish installers. |

A few things to know:

- **Language.** The PRD and TSD are partly written in Indonesian. The README, code, and code comments are in English.
- **Status.** The PRD is the original plan and is not updated for every change. The TSD and database design are maintained, but they can lag behind.
- **Source of truth.** If a document disagrees with the code or the [README](../README.md), the code and README win. Pull requests that fix outdated docs are welcome.
