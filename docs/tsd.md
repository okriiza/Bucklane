# Bucklane — Technical Specification Document

**Document ID:** BUCK-TSD-001  
**Version:** 1.0  
**Status:** Maintained alongside the code  
**Related Documents:** Bucklane PRD v1.0, [Database Design v1.0](database_design.md)  
**Architecture:** Local-first Desktop Application  
**Primary Stack:** Tauri 2 / Vue 3 / TypeScript / Rust

---

# 1. System Overview

Bucklane is a desktop application for accessing and managing Amazon S3 and S3-compatible object storage without a centralized cloud backend.

All storage operations, credential management, and file transfers are handled by a Rust backend that runs as part of the desktop application.

The Vue frontend communicates with Rust through Tauri IPC.

## 1.1 System Boundaries

The frontend is responsible for:

- User interface and navigation.
- State management.
- Rendering the file explorer.
- Displaying connection status.
- Displaying and controlling transfers.
- Displaying errors and confirmation dialogs.

The Rust backend is responsible for:

- Request validation and authorization of local operations.
- Connection configuration management.
- Secure credential storage.
- Communication with the S3-compatible API.
- Bucket and object operations.
- File transfers and progress.
- SQLite storage.
- Logging and error mapping.

## 1.2 Architectural Principles

1. Local-first: no Bucklane account or application server is required.
2. Secure by default: secrets are not stored in SQLite or in frontend state.
3. Provider agnostic: standard S3 operations use the same abstraction.
4. Streaming-first: large files are never loaded entirely into RAM.
5. Non-blocking: network requests and transfers do not block the interface.
6. Explicit destructive actions: delete and overwrite must be safe and traceable.
7. Testable components: business logic is separated from Tauri command handlers.
8. Maintainable: interfaces and repositories are created as needed, not automatically for every model.

## 1.3 Technology Stack

| Area               | Choice                                                   |
| ------------------ | -------------------------------------------------------- |
| Desktop shell      | Tauri 2                                                  |
| Tauri plugins      | `tauri-plugin-dialog` only (no `fs`, no `shell`)         |
| Backend language   | Rust (stable)                                            |
| Async runtime      | Tokio                                                    |
| S3 client          | `aws-sdk-s3` (custom endpoint, path-style, multipart)    |
| Database           | SQLite via SQLx (embedded migrations)                    |
| Credential store   | `keyring` crate                                          |
| IDs                | `uuid` with `v7` feature                                 |
| Logging            | `tracing`, `tracing-subscriber`, `tracing-appender`      |
| IPC type bindings  | `tauri-specta` → TypeScript                              |
| Frontend           | Vue 3 + TypeScript (Composition API, `<script setup>`)   |
| Routing / state    | Vue Router, Pinia                                        |
| Styling            | Tailwind CSS v4                                          |
| UI primitives      | shadcn-vue (Reka UI)                                     |
| i18n               | `vue-i18n`; MVP ships English only                       |
| Package manager    | pnpm                                                     |

## 1.4 Credential Scope (MVP)

The MVP supports only **static access key ID + secret access key**.

Not supported in the MVP:

- Session token / STS temporary credentials.
- AWS shared config/credentials profiles.
- AWS SSO / IAM Identity Center.
- Instance metadata or the default credential chain.

# 2. Backend Architecture

## 2.1 Layer Responsibilities

### Commands Layer

The Tauri IPC entry point.

Responsibilities:

- Receive requests from the frontend.
- Validate input structure.
- Call the appropriate service.
- Return a response or an application error.

Commands must not contain direct implementations of S3 operations or SQL queries.

### Services Layer

Handles application use cases and business rules.

Core services:

- ConnectionService
- StorageService
- ObjectService
- TransferService
- SettingsService

### Provider Layer

Abstracts the object storage access implementation.

Components:

- StorageProvider trait
- S3StorageProvider
- S3ClientFactory
- ProviderProfiles
- CapabilityRegistry

In the MVP, all providers use S3StorageProvider with different endpoint configurations.

### Repository Layer

Handles local data.

Repositories:

- ConnectionRepository
- BucketBookmarkRepository
- TransferRepository
- SettingsRepository

Repositories are not responsible for accessing objects in remote S3.

### Security Layer

Handles credential storage and retrieval, endpoint validation, and security policies.

# 3. Rust Project Structure

```text
src-tauri/src/
├── main.rs
├── lib.rs
├── app_state.rs
│
├── commands/
│   ├── mod.rs
│   ├── connections.rs
│   ├── buckets.rs
│   ├── objects.rs
│   ├── transfers.rs
│   └── settings.rs
│
├── domain/
│   ├── mod.rs
│   ├── connection.rs
│   ├── bucket.rs
│   ├── object.rs
│   ├── transfer.rs
│   └── capability.rs
│
├── services/
│   ├── mod.rs
│   ├── connection_service.rs
│   ├── storage_service.rs
│   ├── object_service.rs
│   ├── transfer_service.rs
│   └── settings_service.rs
│
├── providers/
│   ├── mod.rs
│   ├── storage_provider.rs
│   ├── s3_provider.rs
│   ├── s3_client_factory.rs
│   ├── provider_profiles.rs
│   └── capability_registry.rs
│
├── repositories/
│   ├── mod.rs
│   ├── connection_repository.rs
│   ├── bucket_bookmark_repository.rs
│   ├── transfer_repository.rs
│   └── settings_repository.rs
│
├── database/
│   ├── mod.rs
│   ├── sqlite.rs
│   └── migrations.rs        # embeds src-tauri/migrations/*.sql
│
├── security/
│   ├── mod.rs
│   ├── credential_store.rs
│   ├── endpoint_policy.rs
│   └── local_selection.rs   # dialog-picked path registry (§6.4)
│
├── transfers/
│   ├── mod.rs
│   ├── scheduler.rs
│   ├── upload.rs
│   ├── download.rs
│   ├── multipart.rs
│   └── progress.rs
│
└── errors/
    ├── mod.rs
    └── app_error.rs
```

# 4. Domain Models

## 4.1 StorageConnection

```rust
pub struct StorageConnection {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub endpoint: Option<String>,
    pub region: String,
    pub access_key_id: String,
    pub credential_ref: String,
    pub force_path_style: bool,
    pub allow_insecure_http: bool,
}
```

ProviderType:

```rust
pub enum ProviderType {
    Aws,
    Supabase,
    Minio,
    CloudflareR2,
    Wasabi,
    DigitalOceanSpaces,
    Custom,
}
```

String representation (IPC and SQLite): `aws`, `supabase`, `minio`, `cloudflare_r2`, `wasabi`, `digitalocean_spaces`, `custom`.

Connection identifiers use UUID v7.

Credentials are not included in connection responses sent to the frontend.

## 4.2 StorageObject

```rust
pub struct StorageObject {
    pub key: String,
    pub name: String,
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub is_prefix: bool,
}
```

Size is optional for virtual folders.

An object key must be treated as an S3 identifier, not a local filesystem path.

## 4.3 Transfer

```rust
pub enum TransferDirection {
    Upload,
    Download,
}

pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}
```

A transfer stores the connection ID (nullable after the connection is deleted), a snapshot of the connection name, bucket, object key, local path, byte counts, status, retry count, error code/message if any, and the multipart upload ID when relevant.

# 5. Storage Provider Abstraction

The provider abstraction is the core piece that lets Bucklane support a variety of S3-compatible storage.

## 5.1 StorageProvider Trait

Example conceptual Rust contract:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn list_buckets(
        &self,
    ) -> Result<Vec<BucketInfo>, AppError>;

    async fn head_bucket(
        &self,
        bucket: &str,
    ) -> Result<(), AppError>;

    async fn list_objects(
        &self,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResult, AppError>;

    async fn head_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectMetadata, AppError>;

    async fn copy_object(
        &self,
        request: CopyObjectRequest,
    ) -> Result<(), AppError>;

    async fn delete_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<(), AppError>;

    // Single-request upload: small files and folder marker objects.
    async fn put_object(
        &self,
        request: PutObjectRequest, // body: streaming source, not Vec<u8>
    ) -> Result<PutObjectResult, AppError>;

    // Returns a streaming body; caller writes it to a temporary file.
    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectStream, AppError>;

    async fn create_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<String, AppError>; // upload ID

    async fn upload_part(
        &self,
        request: UploadPartRequest,
    ) -> Result<CompletedPart, AppError>; // part number + ETag

    async fn complete_multipart_upload(
        &self,
        request: CompleteMultipartRequest,
    ) -> Result<(), AppError>;

    async fn abort_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), AppError>;
}
```

File transfers are orchestrated by TransferService (scheduler, progress, retry, cancel), while StorageProvider only provides the primitive S3 operations above.

Upload/download operations must not load the entire file content into memory; request/response bodies are streamed.

## 5.2 Client Factory

S3ClientFactory builds clients based on:

- Provider type.
- Endpoint.
- Region.
- Access credentials.
- Path-style configuration.
- Timeout and retry policy.
- TLS policy.

Credentials are fetched from the OS Keychain when needed.

Clients may be cached in memory using the key `(connection_id, updated_at)`; `updated_at` serves as the configuration revision.

The cache must be invalidated when a connection is updated, a credential is rotated, or a connection is deleted.

**AWS bucket region resolution:** On AWS, ListBuckets returns buckets from all regions, while a client is configured for a single region. For AWS connections without a custom endpoint, a bucket's region is detected once from the `x-amz-bucket-region` header of HeadBucket (this header is still sent on 301/403 responses), cached per `(connection_id, bucket)`, and a client for that region is built with the same credentials. Users do not need to create one connection per region.

The default credential chain must never be used accidentally for a connection with explicit credentials.

## 5.3 Provider Profiles

Provider profiles define default configuration, but do not replace endpoint validation or capability checks.

**AWS S3**

- Region required.
- Endpoint follows the AWS region by default.
- HTTPS enabled.
- Virtual-hosted addressing is used by default when compatible.

**Supabase**

- Endpoint uses `/storage/v1/s3`.
- Region follows the project configuration.
- Path-style addressing enabled.
- Credentials use S3 access keys or another supported authentication method.

**MinIO**

- Custom endpoint.
- Region configurable.
- Path-style addressing as the initial default.
- HTTP allowed only through explicit opt-in.

**Cloudflare R2**

- Account-specific endpoint.
- Region and S3 settings follow Cloudflare's requirements.
- Compatibility testing required.

**Wasabi**

- Wasabi's region-specific endpoint.
- Region required and must match the endpoint.
- HTTPS enabled.
- Compatibility testing required.

**DigitalOcean Spaces**

- Endpoint based on the Spaces region.
- Region follows the Spaces datacenter.
- HTTPS enabled.
- Compatibility testing required.

**Custom S3**

- Manual endpoint.
- Manual region.
- Path-style configurable.
- Does not assume every AWS S3 capability is supported.

## 5.4 Capability System

CapabilityRegistry provides the statuses:

- Supported
- Unsupported
- Unknown

Capabilities to track:

- LIST_BUCKETS
- HEAD_BUCKET
- LIST_OBJECTS
- PUT_OBJECT
- GET_OBJECT
- COPY_OBJECT
- DELETE_OBJECT
- MULTIPART_UPLOAD
- PRESIGNED_URL

An unknown capability must not be automatically treated as supported for risky operations.

Capabilities can come from the provider profile, vendor documentation, and the results of actual operations.

Read-only credentials do not mean the provider does not support uploads; permission denied must be distinguished from an unsupported API.

# 6. Tauri IPC Contract

The frontend communicates with the backend only through registered commands.

## 6.0 Type Generation

All command inputs/outputs and event payloads are defined as Rust structs (`serde`, `camelCase`), and TypeScript types are generated with `tauri-specta` into `src/services/tauri/bindings.ts`. The generated file is never edited manually; CI fails if the bindings are out of sync.

The TypeScript examples below are illustrative; the Rust structs are the source of truth.

All commands return `Result<T, AppError>` (see §11).

## 6.1 Connection Commands

```text
create_connection
update_connection
delete_connection
list_connections
get_connection
test_connection
list_provider_profiles
```

`list_provider_profiles` returns per-provider defaults and form hints (label, required/optional endpoint, placeholder, default region, default path-style), so the frontend does not keep its own copy of the provider configuration.

`delete_connection` returns `{ secretRemoved: boolean }`; if `false`, the connection has been deleted but the credential store entry could not be cleaned up, and the UI shows a warning.

```typescript
type ProviderType =
  | "aws" | "supabase" | "minio" | "cloudflare_r2"
  | "wasabi" | "digitalocean_spaces" | "custom";

interface CreateConnectionInput {
  name: string;
  provider: ProviderType;
  endpoint?: string;
  region: string;
  accessKeyId: string;
  secretAccessKey: string;
  forcePathStyle: boolean;
  allowInsecureHttp: boolean;
}

interface UpdateConnectionInput {
  id: string;
  name: string;
  provider: ProviderType;
  endpoint?: string;
  region: string;
  accessKeyId: string;
  secretAccessKey?: string; // absent = keep existing secret
  forcePathStyle: boolean;
  allowInsecureHttp: boolean;
}

interface ConnectionDto {
  id: string;
  name: string;
  provider: ProviderType;
  endpoint?: string;
  region: string;
  accessKeyId: string;
  forcePathStyle: boolean;
  allowInsecureHttp: boolean;
  createdAt: string;
  updatedAt: string;
  // never: secretAccessKey, credentialRef
}

interface TestConnectionInput {
  connectionId?: string;           // test a saved connection, or
  config?: CreateConnectionInput;  // test an unsaved configuration
  testBucket?: string;
}
```

**Update semantics:**

- `secretAccessKey` empty/not sent → the existing secret is kept.
- `secretAccessKey` provided → run the credential rotation flow (§8.2).
- If `accessKeyId` changes, `secretAccessKey` is **required**.
- Every update invalidates the client cache for that connection.

TestConnection must be able to use a temporary configuration without permanently storing credentials. If ListBuckets is denied and `testBucket` is provided, the test uses HeadBucket on that bucket.

## 6.2 Bucket Commands

```text
list_buckets
head_bucket
list_bucket_bookmarks
add_bucket_bookmark
update_bucket_bookmark
remove_bucket_bookmark
```

`list_buckets` takes a connection ID.

`head_bucket` takes a connection ID and a bucket name.

Bookmark commands manage the `connection_buckets` table. If ListBuckets is denied, the user can still open a bucket through a bookmark; access is verified with `head_bucket` when it is opened.

## 6.3 Object Commands

```text
list_objects
head_object
create_folder
copy_object
move_object
delete_objects
prepare_prefix_delete
execute_prefix_delete
create_share_link
preview_object
```

`preview_object` fetches object content for in-app preview: images (≤ 15 MB) and PDFs (≤ 25 MB) in full, text only the first 512 KB via HTTP Range. The content is sent as base64 and displayed through a `blob:` URL, so the CSP only needs `img-src blob:` and `frame-src blob:` added, without allowing remote origins.

The preview content type is determined by the preview kind, not by the object's `Content-Type`, which anyone with write access to the bucket can set: PDFs are always `application/pdf`, images only use types from an allowlist (raster and `image/svg+xml`, which are safe in `<img>`), and everything else is `application/octet-stream`. The frontend also picks the blob type from the preview kind. Reading stops at the size limit even if HEAD does not report a size or the server ignores Range.

`create_share_link` creates a presigned GET URL for a single object (60 seconds to 7 days, the SigV4 limit). The object's existence is checked first. The URL is neither stored nor logged because it acts as a bearer credential (SEC-004); a link can only be revoked early by rotating the access key.

Rename is `move_object` with the same bucket and a new destination key; there is no separate rename command.

`copy_object` and `move_object` take a `conflictPolicy` (`ASK` | `OVERWRITE`) and return a tagged union:

```typescript
type ObjectOperationOutcome =
  | { kind: "completed" }
  | { kind: "conflict"; existingSize: number | null; existingLastModified: string | null } // nothing was changed
  | { kind: "sourceKept"; reason: string }; // move: copy succeeded, source deliberately kept
```

Copy uses `x-amz-copy-source-if-match` with the source ETag so that a source changed mid-operation is not copied. Move deletes the source only after the destination is verified (same size) and the source ETag is rechecked as unchanged; when in doubt, the result is `sourceKept`.

`copy_object` and `move_object` apply only to single objects in the MVP. Folder (prefix) copy/move is Post-MVP. Objects larger than 5 GiB are rejected with `OPERATION_UNSUPPORTED` because they require multipart copy (see §9.6).

ListObjectsRequest:

```typescript
interface ListObjectsRequest {
  connectionId: string;
  bucket: string;
  prefix?: string;
  delimiter?: string;
  continuationToken?: string;
  pageSize?: number;
}
```

ListObjectsResponse:

```typescript
interface ListObjectsResponse {
  objects: StorageObject[];
  nextContinuationToken?: string;
  isTruncated: boolean;
}
```

The continuation token must be treated as an opaque value and not modified by the frontend.

Object listing uses the S3 ListObjectsV2 API with prefix, delimiter, and pagination.

Server-side name search uses the same listing with `prefix = folder + query` (query without `/`). S3 only matches the start of the key and is case-sensitive, so the results are items in the active folder whose names start with the query. The `name` field is always relative to the folder containing the prefix (the part up to the last `/`), so search results still show the full name.

### 6.3.1 Bulk Delete Result

```typescript
interface DeleteObjectsResult {
  deleted: string[];
  failed: { key: string; error: AppError }[];
}
```

`delete_objects` uses the DeleteObjects API in batches of at most 1,000 keys. If the provider does not support DeleteObjects, it falls back to DeleteObject per key. Partial failures are always reported; the UI must not show full success if `failed` is not empty.

### 6.3.2 Folder (Prefix) Delete

Deleting a folder means deleting every object under the prefix, so it is done in two phases:

```typescript
interface PrefixDeletePlan {
  planId: string;
  bucket: string;
  prefix: string;
  objectCount: number;
  totalBytes: number;
  sampleKeys: string[]; // first N keys for preview
  expiresAt: string;
}
```

1. `prepare_prefix_delete(connectionId, bucket, prefix)` — the backend lists every object under the prefix (without a delimiter), keeps a snapshot of the keys in memory, and returns a `PrefixDeletePlan`.
2. The UI shows a confirmation with the bucket name, prefix, object count, and total size.
3. `execute_prefix_delete(planId)` — the backend deletes **only the keys in the snapshot**, so objects added after the plan was created are not deleted. The result uses `DeleteObjectsResult`.

Rules:

- A plan expires after 10 minutes and can be executed only once.
- Plans with more than 100,000 objects are rejected in the MVP with `OPERATION_UNSUPPORTED`.
- An empty prefix (bucket root) is not allowed.

## 6.4 Local File Selection

The frontend **never** sends filesystem paths to the backend for read/write operations. Local files and folders can only be used after the user selects them through a native dialog opened from the Rust side.

```text
pick_upload_files
pick_download_directory
```

```typescript
interface LocalFileSelection {
  selectionId: string;
  fileName: string;
  sizeBytes: number;
  displayPath: string; // UI only; never accepted back as input
}

interface LocalDirectorySelection {
  selectionId: string;
  displayPath: string;
}
```

Rules:

- The backend opens the dialog using `tauri-plugin-dialog` from Rust and stores the selected paths in `AppState` (in-memory registry).
- The frontend only receives an opaque `selectionId`.
- A selection is valid for 30 minutes and is not persisted.
- Drag & drop (FIL-008) is handled through the window drag-drop event in Rust, which registers the selection and sends the `selectionId` to the frontend.
- The Tauri `fs` plugin is not used.
- Transfers that have already been created store the path in SQLite; `retry_transfer` uses that path, not a new selection.

## 6.5 Transfer Commands

```text
enqueue_upload
enqueue_download
list_transfers
get_transfer
cancel_transfer
retry_transfer
clear_transfer_history
reveal_transfer
```

`reveal_transfer(id)` shows a transfer's local file in Finder / File Explorer. The path is read only in the backend from the transfer row; the frontend just sends the ID. The opener plugin is used as a Rust library and is not registered, so no opener command is exposed to the frontend.

Operations run through the scheduler without holding the IPC request until the transfer finishes.

```typescript
type ConflictPolicy = "ASK" | "OVERWRITE" | "SKIP" | "RENAME";

interface EnqueueUploadInput {
  connectionId: string;
  bucket: string;
  prefix: string;
  selectionIds: string[];
  conflictPolicy: ConflictPolicy; // default "ASK"
}

interface EnqueueDownloadInput {
  connectionId: string;
  bucket: string;
  keys: string[];
  directorySelectionId: string;
  conflictPolicy: ConflictPolicy; // default "ASK"
}

type EnqueueItemResult =
  | { kind: "queued"; transferId: string; key: string }
  | { kind: "conflict"; key: string; existingSize?: number; existingLastModified?: string }
  | { kind: "skipped"; key: string }
  | { kind: "rejected"; key: string; error: AppError };

interface TransferDto {
  id: string;
  connectionId?: string;
  connectionNameSnapshot: string;
  direction: "UPLOAD" | "DOWNLOAD";
  bucket: string;
  objectKey: string;
  localFileName: string; // basename only
  bytesTotal?: number;
  bytesTransferred: number;
  status: "QUEUED" | "RUNNING" | "COMPLETED" | "FAILED" | "CANCELED";
  retryCount: number;
  error?: AppError;
  createdAt: string;
  startedAt?: string;
  completedAt?: string;
}
```

### 6.5.0 Folders

- `pick_upload_folder` and folder drag-and-drop register a selection of kind `UploadDirectory`. This selection is read-only: it cannot be used as a download destination.
- `enqueue_upload` expands folders recursively (symlinks and OS metadata files such as `.DS_Store` are skipped); each file becomes a separate transfer with the key `<prefix><folder name>/<relative path>`. Object existence is checked with a single listing of the destination prefix, not a HEAD per file.
- `enqueue_download` accepts folder keys (ending in `/`); every object under it is downloaded, with the folder structure recreated at the destination. Each path segment is sanitized so that a key cannot place a file outside the destination folder.
- Limit of 10,000 files per folder. Conflict items for files inside a folder are given the ID `<folder><U+001F><path/key>` so that resubmitting with the user's choice processes only those files.

### 6.5.1 Conflict Policy

Upload:

1. For each file, the backend runs HeadObject on the destination key.
2. If the key already exists and `conflictPolicy = "ASK"`, the item is **not** queued and is returned as `conflict`.
3. The UI shows an Overwrite / Skip / Rename dialog (with an "apply to all" option), then calls `enqueue_upload` again for those items with the chosen policy.
4. `RENAME` produces the keys `name (1).ext`, `name (2).ext`, etc. until an unused key is found.

Download:

- The same rules apply to files in the destination directory.
- The destination file's existence is rechecked at finalization (§9.4); if a new file appears between the check and finalization, the transfer fails with `LOCAL_FILE_ERROR` instead of overwriting.

The upload conflict check is best-effort, not atomic, because conditional writes are not uniformly supported by all providers.

### 6.5.2 Retry & History

- `retry_transfer` applies only to the `FAILED` status. The same transfer moves to `QUEUED`, `retry_count` is incremented, and the error is cleared.
- For multipart uploads, retry attempts `AbortMultipartUpload` on the old upload ID (best-effort), deletes the related `transfer_parts`, then starts the upload from the beginning.
- `clear_transfer_history` deletes only transfers in a terminal status (`COMPLETED`, `FAILED`, `CANCELED`). Transfers with a multipart upload that has not been successfully cleaned up are not deleted.

## 6.6 Settings Commands

```text
get_settings
update_settings
reset_settings
```

```typescript
interface AppSettings {
  theme: "system" | "light" | "dark";
  maxConcurrentTransfers: number; // 1–10
  multipartThresholdMb: number; // 5–5120
  multipartPartSizeMb: number; // 5–5120
  explorerPageSize: number; // 50–1000
}
```

Settings are sent as a single typed object and validated by SettingsService; storage remains key/value JSON in the `settings` table (DB design §5). Invalid stored values are ignored and replaced with defaults. `update_settings` applies transfer settings to the TransferManager immediately; lowering the concurrency limit does not stop transfers that are already running.

Only settings the application actually applies can be changed. Options that weaken data safety (for example disabling delete/overwrite confirmation, SEC-005) are deliberately not provided.

## 6.7 Events

Events are defined as Rust structs and generated by `tauri-specta` (`events.*` in `bindings.ts`); event names are derived from the struct names.

| Event | Payload | When |
| --- | --- | --- |
| `transfer-update` | Full `TransferDto` | Every status change and periodically while RUNNING |
| `files-dropped` | `{ files: LocalFileSelection[], skipped: number }` | Files from the OS are dropped onto the window; already registered in the selection registry |
| `file-drag-hover` | `boolean` | Files from the OS start/stop being dragged over the window |

A single `transfer-update` event with the full DTO replaces five separate events (`started`/`progress`/`completed`/`failed`/`canceled`): the frontend simply upserts by `id`, and no event ordering can produce a contradictory status.

Progress is sent every 300 ms and persisted to SQLite roughly every 2 seconds.

Events are transient information. SQLite and the runtime transfer manager are the authoritative source of status. The frontend reloads status through `list_transfers` on startup.

# 7. Local Database Design

Database: SQLite via SQLx.

The database location follows each OS's application data directory.

The full schema, constraints, indexes, initial migration, and data lifecycle are defined in **[Database Design](database_design.md)**, which is the single source of truth for the schema. This section only summarizes it.

## 7.1 Tables

| Table                | Purpose                                                      |
| -------------------- | ------------------------------------------------------------ |
| `connections`        | Non-secret connection profiles + `credential_ref` (UNIQUE, 1:1) |
| `connection_buckets` | Bucket bookmarks per connection (cascade on connection delete) |
| `transfers`          | Transfer history and status                                  |
| `transfer_parts`     | Completed multipart upload parts (cascade)                   |
| `settings`           | Application preferences as JSON-encoded values               |

## 7.2 Connection Deletion Policy

1. Active transfers (`QUEUED`/`RUNNING`) belonging to the connection must be canceled or stopped first.
2. The connection is deleted; its bookmarks are deleted with it (`ON DELETE CASCADE`).
3. Transfer history is **retained**: `transfers.connection_id` becomes `NULL` (`ON DELETE SET NULL`) and `connection_name_snapshot` still shows the connection name.
4. The secret is deleted from the OS credential store.

## 7.3 Migration Strategy

- Every schema change is made in a versioned migration.
- Migrations run at application startup.
- Migrations must be atomic where possible.
- A migration failure must stop operations that require the database.
- Upgrades must never automatically delete user configuration.

# 8. Credentials & Security

## 8.1 Secure Credential Store

Interface:

```rust
pub trait CredentialStore {
    fn save(
        &self,
        reference: &str,
        secret: &str,
    ) -> Result<(), AppError>;

    fn get(
        &self,
        reference: &str,
    ) -> Result<String, AppError>;

    fn delete(
        &self,
        reference: &str,
    ) -> Result<(), AppError>;
}
```

Implementation adapters:

- macOS: Keychain.
- Windows: Credential Manager.
- Linux: Secret Service-compatible keyring, if available.

If a secure credential store is not available, Bucklane must never silently store secrets in plaintext. The application must show an error or offer an explicitly supported secure method.

On Linux without Secret Service (for example without GNOME Keyring/KWallet), connections cannot be saved. The application shows an error explaining the required dependency; the Linux installation documentation lists this dependency.

## 8.2 Credential Rotation

When the user changes the Secret Access Key:

1. Validate the new configuration.
2. Save the new credential under a new reference.
3. Update the reference in an SQLite transaction.
4. Invalidate the client cache.
5. Delete the old credential after the switch succeeds.

If the process fails, rollback or recovery must prevent the connection from pointing to an unavailable secret.

## 8.3 Endpoint Security

- HTTPS by default.
- Certificate verification required by default.
- No global ignore-TLS-errors option in the MVP.
- HTTP allowed only through explicit configuration.
- Endpoint and region are validated before the client is built.
- Authorization headers and secrets must never be logged.
- Unexpected redirects must be handled safely, especially for custom endpoints.

## 8.4 Tauri Security

- Minimal application capabilities.
- Only required commands are registered.
- No arbitrary shell execution.
- No arbitrary filesystem read/write IPC; local paths only through the selection registry (§6.4).
- The `fs` and `shell` plugins are not used.
- Content Security Policy is configured.
- No untrusted remote scripts.

Additional hardening:

- The `navigation-guard` plugin blocks webview navigation outside the application origin (`tauri://localhost`, `http(s)://tauri.localhost`, `blob:`, `about:blank`, and the dev server only in debug builds).
- Folder downloads are rejected if an existing parent folder points outside the destination folder through a symlink (`stays_within`). Windows device names (`CON`, `NUL`, `COM1`, ...) get a `_` suffix.
- `log_frontend_error` writes UI errors to the log with URLs and `X-Amz-*`/`Signature=`/`Credential=` parameters redacted, at most 200 entries per session. `diagnostics` returns only the version, OS, architecture, WebView version, and database status.
- CI runs `cargo audit` and `pnpm audit --prod`.

## 8.5 In-App Updates

- Uses `tauri-plugin-updater` from Rust only (`src-tauri/src/updates.rs`). Updater permissions are not granted to the frontend; the UI uses the commands `check_for_update`, `install_update`, `restart_app`, and `open_release_page(version)`. The release URL is built in Rust from a validated version string.
- Endpoint: `https://github.com/okriiza/Bucklane/releases/latest/download/latest.json`. Draft releases are not visible to the updater.
- Update packages are signed (minisign) by the release workflow using the `TAURI_SIGNING_PRIVATE_KEY` secret. The application verifies the signature against `plugins.updater.pubkey` before installing; packages that fail verification are rejected.
- Automatic check once when the application opens, which can be turned off with the `updates.check_on_startup` setting. No user data is sent. Development builds do not check automatically.
- Download progress is sent through the `update-progress` event. On Windows the installer closes the application; on macOS/Linux the UI asks for a restart. The UI asks for confirmation if transfers are still running.

# 9. Transfer Engine Design

## 9.1 Architecture

TransferService manages:

- Transfer creation.
- Queue state.
- Concurrency management.
- Transfer execution.
- Progress reporting.
- Cancellation.
- Retry.
- Transfer persistence.

Tokio is used for asynchronous task execution.

A semaphore controls the number of active transfers.

Default concurrency: 3 transfers; within a single multipart upload, at most 4 parts are uploaded in parallel.

Concurrency, the multipart threshold (default 64 MiB), and the part size (default 16 MiB, raised automatically to keep ≤ 10,000 parts and never below 5 MiB) are read from Settings and can be changed while the application is running.

## 9.2 Transfer Lifecycle

```text
QUEUED
   |
   v
RUNNING -------> COMPLETED
   |
   +-----------> FAILED
   |
   +-----------> CANCELED

FAILED --retry--> QUEUED
```

Cancellation can be requested while a transfer is queued or running.

Task completion and cancellation must be synchronized to prevent contradictory statuses.

When the application restarts, QUEUED and RUNNING statuses from the previous session are reconciled to FAILED with `error_code = INTERRUPTED` (transfers are not started automatically without user action), unless a verified resume mechanism is available.

## 9.3 Upload Strategy

Small file:

- Single PutObject.
- Streaming file source.
- Progress tracking.

Large file:

- Multipart upload.
- Configurable part size.
- Bounded parallel part uploads.
- Complete multipart after all parts finish.

Multipart sequence:

1. CreateMultipartUpload.
2. Read file chunks.
3. UploadPart.
4. Record completed part identifiers.
5. CompleteMultipartUpload.
6. Validate final result.

If a permanent failure occurs before completion, attempt AbortMultipartUpload.

If the abort fails, record the upload ID safely for cleanup/recovery.

The part size must satisfy S3 limits, including the minimum size of non-final parts and the maximum number of parts.

## 9.4 Download Strategy

1. Request object metadata.
2. Validate destination.
3. Create temporary local file in target filesystem.
4. Stream object data into temporary file.
5. Track progress.
6. Validate completion.
7. Finalize destination using safe replacement rules.

A download must never overwrite an important file directly before the operation completes.

Failed temporary files must be cleaned up or marked as incomplete.

## 9.5 Retry Policy

Retry is performed only for errors worth retrying, for example:

- Temporary network errors.
- Connection reset.
- Selected server 5xx responses.
- Throttling.

No automatic retry on:

- Invalid credentials.
- Access denied.
- Invalid object key.
- Unsupported operation.
- Permanent validation error.

Retry uses exponential backoff with jitter and an attempt limit.

Non-idempotent operations or operations with destructive effects require a dedicated retry policy.

## 9.6 Copy / Move / Rename Safety

S3 rename and move use a combination of copy and delete.

Safe move sequence:

1. Validate source.
2. Validate destination.
3. Resolve overwrite policy.
4. Execute copy.
5. Verify copied destination.
6. Delete source only after successful verification.
7. Refresh explorer.

For large objects, the provider may require multipart copy.

The MVP must block copy/move for sizes or providers that are not yet safely supported, instead of attempting risky operations. MVP limit: objects above 5 GiB (the single-request CopyObject limit) are rejected with `OPERATION_UNSUPPORTED`; the size is checked through HeadObject before copying.

An object ETag must not always be assumed to be an MD5 hash of the content.

Concurrent modification and versioning must be considered so that a source that has changed is not deleted accidentally.

# 10. Frontend Architecture

## 10.1 Vue Directory Structure

```text
src/
├── app/
├── assets/
├── components/
│   ├── common/
│   ├── connections/
│   ├── explorer/
│   └── transfers/
├── composables/
├── layouts/
├── pages/
├── router/
├── services/
│   └── tauri/
├── stores/
├── types/
├── utils/
├── App.vue
└── main.ts
```

## 10.2 State Management

Pinia stores:

**connectionStore**

- Connections.
- Selected connection ID.
- Connection status.
- Loading/error state.

**explorerStore**

- Selected bucket.
- Active prefix.
- Object listing.
- Pagination state. After the first page (Settings page size), the remaining pages load in the background with 1000 keys per request, up to 10,000 entries (`AUTO_LOAD_LIMIT`), because S3 interleaves folders and files by name and a subfolder can sit several pages deep. Loading stops when the user opens another folder; a failed background page is left to "Load more".
- Server search query (cleared when changing folders).
- Sort order (name, size, date; folders always on top). Sorted on the client over the already-loaded items, stored in localStorage as a preference.
- Selection state.

**transferStore**

- Transfer items.
- Progress.
- Queue status.
- Error notifications.

Secrets are not persisted in Pinia.

All UI text goes through `vue-i18n` from the start so other languages can be added without refactoring; the MVP provides English only.

## 10.3 Explorer UI Requirements

The sidebar shows connections and bucket navigation.

The main panel shows:

- Breadcrumb.
- Upload button.
- New folder.
- Refresh.
- File listing.
- Context menu.
- Object metadata.

The transfer panel sits at the bottom or as a drawer.

Every operation has loading, empty, success, and error states.

# 11. Error Model

All Rust errors exposed to the frontend use a structured format.

```typescript
interface AppError {
  code: string;
  message: string;
  details?: string;
  retryable: boolean;
}
```

Main error codes:

- CONNECTION_INVALID
- INVALID_INPUT
- AUTHENTICATION_FAILED
- ACCESS_DENIED
- ENDPOINT_UNREACHABLE
- TLS_ERROR
- BUCKET_NOT_FOUND
- OBJECT_NOT_FOUND
- OPERATION_UNSUPPORTED
- TRANSFER_FAILED
- TRANSFER_CANCELED
- INTERRUPTED
- LOCAL_FILE_ERROR
- STORAGE_QUOTA_ERROR
- DATABASE_UNAVAILABLE
- INTERNAL_ERROR

Detail messages that are displayed or stored must be sanitized so they do not leak secrets or signed URLs.

# 12. Observability

The MVP uses local structured logging.

Logging library: `tracing`.

Data that may be logged:

- Command name.
- Operation type.
- Provider type.
- Connection ID.
- Transfer ID.
- Duration.
- Sanitized error code.
- Result status.

Data that must not be logged:

- Secret Access Key.
- Authorization header.
- Session token.
- Signed URL.
- File contents.
- Raw credential payload.

Logs are written to each OS's application log directory using `tracing-appender` with daily rotation and retention of 7 files. The level follows the `logging.level` setting.

Remote telemetry is not enabled by default.

# 13. Testing Strategy

## 13.1 Unit Tests

- Connection validation.
- Endpoint policies.
- Capability resolution.
- Error mapping.
- Transfer state transitions.
- Retry classification.
- Safe destination validation.

## 13.2 Integration Tests

Target providers and how they are run:

| Provider         | Execution                                                       |
| ---------------- | --------------------------------------------------------------- |
| Local S3-compatible (RustFS) | Every PR in CI via `docker compose` (`BUCKLANE_S3_TEST_ENDPOINT`) |
| MinIO            | Manually against a MinIO server; the MinIO Docker image is no longer published |
| AWS S3           | Nightly or manual, credentials from GitHub repository secrets   |
| Supabase Storage | Nightly or manual, credentials from GitHub repository secrets   |

Integration tests that require external credentials are explicitly skipped (not failed) if the secrets are unavailable, for example on PRs from forks.

Test cases:

- Valid connection.
- Invalid credentials.
- Read-only credentials.
- ListBuckets denied.
- ListObjects pagination.
- Upload/download round trip.
- Delete object.
- Copy/move object.
- Multipart upload.
- Network interruption.
- Retry behavior.

## 13.3 Security Tests

- Secrets are not stored in SQLite.
- Secrets do not appear in logs.
- Secrets are not returned in the connection DTO.
- Unsafe HTTP requires explicit opt-in.
- Destructive actions require confirmation.
- Destination path traversal is not allowed.
- Transfer commands reject raw paths; only a valid, unexpired `selectionId` is accepted.
- Prefix delete deletes only the keys in the plan snapshot.
- Tauri IPC does not expose arbitrary filesystem access.

## 13.4 End-to-End Tests

Core user scenarios:

1. Add a Supabase connection.
2. Connect and list buckets.
3. Browse folder/prefix.
4. Upload a file.
5. Download the file.
6. Rename or move the file.
7. Delete the file.
8. Reopen application and reuse the saved connection.

# 14. CI/CD

GitHub Actions is responsible for:

- TypeScript lint/type checking.
- Vue unit tests.
- Rust formatting.
- Rust Clippy.
- Rust unit tests.
- Application builds.
- Artifact packaging.
- Release publishing.

Release targets:

- macOS Apple Silicon.
- macOS Intel.
- Windows x64.
- Linux x64.

Code signing and notarization are release requirements for mature public distribution, depending on the operating system.

Autoupdate must never accept an update without signature verification.

# 15. Implementation Milestones

## M00 — Foundation

Deliverables:

- Tauri 2 project.
- Vue 3 setup.
- Tailwind configuration.
- App shell.
- Rust application state.
- Error model.
- SQLite connection.
- CI baseline.

Acceptance:

- Desktop application launches.
- Frontend can invoke a Rust health command.
- Automated checks pass.

## M01 — Connection Management

Deliverables:

- Connection domain.
- Connection repository.
- OS Keychain adapter.
- S3ClientFactory.
- Provider profiles.
- Test connection.
- Connection CRUD UI.

Acceptance:

- User can add a Supabase connection.
- Credentials survive application restart securely.
- User can edit and delete connections.
- Invalid connection returns actionable error.

## M02 — Storage Explorer

Deliverables:

- StorageProvider adapter.
- Bucket listing.
- Configured bucket access.
- Object listing.
- Pagination.
- Prefix navigation.
- Explorer UI.

Acceptance:

- User can browse AWS, MinIO, and Supabase test buckets.
- Pagination works.
- Read-only permissions are handled properly.

## M03 — Object Operations

Upload/download moved to M04 because it depends on the transfer engine (queue, progress, cancel, multipart).

Deliverables:

- Create folder marker.
- Copy.
- Rename.
- Move.
- Delete.
- Confirmation flows.

Acceptance:

- No source deletion before verified copy success.
- Download failures do not overwrite valid local files.
- Object operations return structured error messages.

## M04 — Transfer Manager

Deliverables:

- Upload/download (with conflict policy and local file selection registry).
- Transfer scheduler.
- Progress events.
- Queue.
- Cancel.
- Retry.
- Multipart upload.
- Transfer history.

Acceptance:

- UI remains responsive during transfers.
- Multiple transfers are supported.
- Failures and cancellation states are consistent.

## M05 — QA & Release

Deliverables:

- Cross-platform build.
- Provider integration test results.
- Security review.
- Installation documentation.
- User guide.
- Signed release pipeline where available.

Acceptance:

- All P0 requirements pass.
- No known critical data-loss/security defects.
- Release artifacts can be installed successfully.

# 16. Architectural Decision Records

**ADR-001:** Use Tauri 2 instead of Electron to reduce runtime overhead.

**ADR-002:** Use Rust for storage network operations and file transfers.

**ADR-003:** Use SQLite only for local application metadata.

**ADR-004:** Store secret credentials in native OS secure storage.

**ADR-005:** Use one configurable S3 adapter for the MVP.

**ADR-006:** Implement transfers as asynchronous managed tasks.

**ADR-007:** Use provider capability metadata to avoid assuming full AWS API compatibility.

**ADR-008:** Do not build a centralized backend or user account system for MVP.

**ADR-009:** Prefer data safety over automatic retry for destructive actions.

**ADR-010:** Use `aws-sdk-s3` as the single S3 client for all providers, configured per connection (endpoint, region, path-style), with the default credential chain disabled.

**ADR-011:** Generate TypeScript IPC bindings from Rust types using `tauri-specta`; Rust is the source of truth for the IPC contract.

**ADR-012:** Local file access is mediated by a backend selection registry; native dialogs are opened from Rust and the frontend only handles opaque selection IDs.

**ADR-013:** Object conflicts default to asking the user; nothing is overwritten without an explicit OVERWRITE policy.

**ADR-014:** Folder deletion uses a two-phase plan/execute flow over a snapshot of keys.

**ADR-015:** MVP supports only static access key credentials; temporary and federated credentials are Post-MVP.

# 17. Risks & Mitigations

| Risk                             | Mitigation                            |
| -------------------------------- | ------------------------------------- |
| S3 API incompatibility           | Provider compatibility testing        |
| Credential exposure              | Native keychain and log redaction     |
| Multipart orphan uploads         | Abort and cleanup tracking            |
| Interrupted downloads            | Temporary files and safe finalization |
| Accidental deletion              | Confirmation and copy verification    |
| Large bucket performance         | Pagination and bounded memory         |
| OS-specific behavior             | Cross-platform CI and manual QA       |
| Credentials unavailable on Linux | Explicit secure-store error handling  |

# 18. Definition of Done

The technical implementation is considered done when:

1. PRD requirements are met.
2. The API/IPC contract matches the specification.
3. Unit and integration tests are in place.
4. Error handling is implemented.
5. Security checks pass.
6. There is no known critical data loss.
7. Documentation is updated.
8. Changes pass CI.
9. It has been reviewed.
10. Relevant provider compatibility has been tested.

---

**End of Document — Bucklane TSD v1.0**
