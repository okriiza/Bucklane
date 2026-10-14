# Bucklane — Technical Specification Document

**Document ID:** BUCK-TSD-001  
**Version:** 1.0  
**Status:** Draft for Implementation  
**Related Document:** Bucklane PRD v1.0  
**Architecture:** Local-first Desktop Application  
**Primary Stack:** Tauri 2 / Vue 3 / TypeScript / Rust

---

# 1. System Overview

Bucklane merupakan aplikasi desktop untuk mengakses dan mengelola Amazon S3 maupun S3-compatible object storage tanpa menggunakan backend cloud terpusat.

Seluruh operasi penyimpanan, manajemen credentials, dan transfer file diproses melalui Rust backend yang berjalan sebagai bagian dari aplikasi desktop.

Frontend Vue berkomunikasi dengan Rust menggunakan Tauri IPC.

## 1.1 System Boundaries

Frontend bertanggung jawab untuk:

- User interface dan navigasi.
- State management.
- Rendering file explorer.
- Menampilkan status koneksi.
- Menampilkan dan mengontrol transfer.
- Menampilkan error serta dialog konfirmasi.

Rust backend bertanggung jawab untuk:

- Validasi permintaan dan otorisasi operasi lokal.
- Manajemen konfigurasi koneksi.
- Penyimpanan kredensial secara aman.
- Komunikasi dengan S3-compatible API.
- Operasi bucket dan object.
- Transfer file dan progress.
- Penyimpanan SQLite.
- Logging dan error mapping.

## 1.2 Architectural Principles

1. Local-first: tidak memerlukan akun Bucklane atau server aplikasi.
2. Secure by default: secret tidak disimpan di SQLite atau state frontend.
3. Provider agnostic: operasi S3 standar menggunakan abstraksi yang sama.
4. Streaming-first: file besar tidak dimuat seluruhnya ke RAM.
5. Non-blocking: request jaringan dan transfer tidak memblokir antarmuka.
6. Explicit destructive actions: delete dan overwrite harus aman dan dapat ditelusuri.
7. Testable components: business logic dipisahkan dari Tauri command handlers.
8. Maintainable: interface dan repository dibuat sesuai kebutuhan, bukan untuk setiap model secara otomatis.

# 2. Backend Architecture

## 2.1 Layer Responsibilities

### Commands Layer

Merupakan entry point Tauri IPC.

Responsibilities:

- Menerima request dari frontend.
- Memvalidasi struktur input.
- Memanggil service yang sesuai.
- Mengembalikan response atau application error.

Commands tidak diperbolehkan mengandung implementasi langsung operasi S3 atau query SQL.

### Services Layer

Menangani application use cases dan business rules.

Core services:

- ConnectionService
- StorageService
- ObjectService
- TransferService
- SettingsService

### Provider Layer

Mengabstraksi implementasi akses object storage.

Komponen:

- StorageProvider trait
- S3StorageProvider
- S3ClientFactory
- ProviderProfiles
- CapabilityRegistry

Pada MVP, seluruh provider menggunakan S3StorageProvider dengan konfigurasi endpoint yang berbeda.

### Repository Layer

Menangani data lokal.

Repositories:

- ConnectionRepository
- TransferRepository
- SettingsRepository

Repository tidak bertanggung jawab mengakses objek pada remote S3.

### Security Layer

Menangani penyimpanan serta pengambilan credentials, validasi endpoint, dan kebijakan keamanan.

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
│   └── transfer_service.rs
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
│   ├── transfer_repository.rs
│   └── settings_repository.rs
│
├── database/
│   ├── mod.rs
│   ├── sqlite.rs
│   └── migrations.rs
│
├── security/
│   ├── mod.rs
│   ├── credential_store.rs
│   └── endpoint_policy.rs
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
    Custom,
}
```

Connection identifiers menggunakan UUID v7.

Credentials tidak disertakan dalam response connection yang dikirim ke frontend.

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

Ukuran tidak wajib untuk folder virtual.

Object key harus diperlakukan sebagai identifier S3, bukan path filesystem lokal.

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

Transfer menyimpan connection ID, bucket, object key, local path, jumlah bytes, status, dan error jika ada.

# 5. Storage Provider Abstraction

Provider abstraction merupakan bagian utama yang memungkinkan Bucklane mendukung berbagai S3-compatible storage.

## 5.1 StorageProvider Trait

Contoh kontrak konseptual Rust:

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
}
```

File transfer menggunakan TransferService dan streaming API yang memanfaatkan S3 client melalui provider adapter.

Operasi upload/download tidak harus mengembalikan konten file dalam memory.

## 5.2 Client Factory

S3ClientFactory membuat client berdasarkan:

- Provider type.
- Endpoint.
- Region.
- Access credentials.
- Path-style configuration.
- Timeout dan retry policy.
- TLS policy.

Credentials diambil dari OS Keychain saat diperlukan.

Client boleh di-cache secara in-memory menggunakan connection ID serta revision konfigurasi.

Cache harus diinvalidasi ketika connection diperbarui, credential dirotasi, atau connection dihapus.

Tidak diperbolehkan menggunakan default credential chain secara tidak sengaja untuk connection dengan kredensial eksplisit.

## 5.3 Provider Profiles

Provider profiles menentukan default konfigurasi, tetapi tidak menggantikan validasi endpoint atau capability checks.

**AWS S3**

- Region wajib.
- Endpoint mengikuti AWS region secara default.
- HTTPS enabled.
- Virtual-hosted addressing digunakan secara default jika kompatibel.

**Supabase**

- Endpoint menggunakan `/storage/v1/s3`.
- Region mengikuti konfigurasi proyek.
- Path-style addressing diaktifkan.
- Credentials menggunakan S3 access keys atau metode autentikasi yang didukung.

**MinIO**

- Custom endpoint.
- Region configurable.
- Path-style addressing sebagai default awal.
- HTTP hanya diperbolehkan melalui explicit opt-in.

**Cloudflare R2**

- Account-specific endpoint.
- Region dan S3 settings mengikuti ketentuan Cloudflare.
- Compatibility testing wajib.

**Custom S3**

- Endpoint manual.
- Region manual.
- Path-style configurable.
- Tidak mengasumsikan seluruh capability AWS S3 didukung.

## 5.4 Capability System

CapabilityRegistry menyediakan status:

- Supported
- Unsupported
- Unknown

Capability yang perlu dilacak:

- LIST_BUCKETS
- HEAD_BUCKET
- LIST_OBJECTS
- PUT_OBJECT
- GET_OBJECT
- COPY_OBJECT
- DELETE_OBJECT
- MULTIPART_UPLOAD
- PRESIGNED_URL

Capability unknown tidak boleh dianggap supported secara otomatis untuk operasi yang berisiko.

Capability bisa berasal dari provider profile, dokumentasi vendor, serta hasil operasi aktual.

Read-only credentials tidak berarti provider tidak mendukung upload; permission denied harus dipisahkan dari unsupported API.

# 6. Tauri IPC Contract

Frontend hanya berkomunikasi dengan backend melalui commands yang terdaftar.

## 6.1 Connection Commands

```text
create_connection
update_connection
delete_connection
list_connections
get_connection
test_connection
```

Input CreateConnection:

- name
- provider
- endpoint
- region
- accessKeyId
- secretAccessKey
- forcePathStyle
- allowInsecureHttp

Output menggunakan DTO connection tanpa secret.

TestConnection harus dapat menggunakan konfigurasi sementara tanpa menyimpan credentials secara permanen.

## 6.2 Bucket Commands

```text
list_buckets
head_bucket
```

`list_buckets` menerima connection ID.

`head_bucket` menerima connection ID dan nama bucket.

Jika ListBuckets ditolak, pengguna tetap dapat menggunakan bucket yang telah dikonfigurasi secara manual.

## 6.3 Object Commands

```text
list_objects
head_object
create_folder
copy_object
move_object
delete_objects
```

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

Continuation token harus diperlakukan sebagai opaque value dan tidak dimodifikasi frontend.

Object listing menggunakan API S3 ListObjectsV2 dengan prefix, delimiter, dan pagination.

## 6.4 Transfer Commands

```text
enqueue_upload
enqueue_download
list_transfers
get_transfer
cancel_transfer
retry_transfer
```

Transfer commands mengembalikan transfer ID.

Operasi berjalan melalui scheduler tanpa menahan IPC request hingga transfer selesai.

File picker menggunakan Tauri dialog dengan izin yang dibatasi.

Untuk membuka file lokal, backend harus menggunakan path yang divalidasi dan dipilih pengguna. Jangan memberi akses filesystem arbitrer hanya melalui string path dari frontend.

## 6.5 Transfer Events

Event namespace:

```text
transfer://started
transfer://progress
transfer://completed
transfer://failed
transfer://canceled
```

Contoh event:

```json
{
  "transferId": "uuid",
  "connectionId": "uuid",
  "status": "RUNNING",
  "bytesTransferred": 5242880,
  "bytesTotal": 10485760,
  "percentage": 50
}
```

Progress events dikirim secara terbatas, misalnya setiap 200–500 ms, agar transfer besar tidak membanjiri frontend.

Event bersifat informasi sementara. SQLite dan runtime transfer manager menjadi sumber status otoritatif.

Frontend harus melakukan refresh status melalui command ketika startup atau reconnect.

# 7. Local Database Design

Database: SQLite.

Lokasi database mengikuti application data directory masing-masing OS.

## 7.1 connections

```sql
CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    endpoint TEXT,
    region TEXT NOT NULL,
    access_key_id TEXT NOT NULL,
    credential_ref TEXT NOT NULL,
    force_path_style INTEGER NOT NULL DEFAULT 0,
    allow_insecure_http INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## 7.2 transfers

```sql
CREATE TABLE transfers (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    bucket TEXT NOT NULL,
    object_key TEXT NOT NULL,
    local_path TEXT NOT NULL,
    bytes_total INTEGER,
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (connection_id)
        REFERENCES connections(id)
);
```

Kebijakan penghapusan connection dengan transfer history harus ditentukan secara eksplisit. MVP dapat mensyaratkan transfer aktif dihentikan sebelum connection dihapus dan menghapus history terkait dalam transaksi database.

## 7.3 settings

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

## 7.4 Migration Strategy

- Setiap perubahan skema dibuat dalam versioned migration.
- Migration dijalankan ketika aplikasi startup.
- Migration harus atomic jika memungkinkan.
- Kegagalan migration harus menghentikan operasi yang membutuhkan database.
- Upgrade tidak boleh menghapus konfigurasi pengguna secara otomatis.

# 8. Credentials & Security

## 8.1 Secure Credential Store

Antarmuka:

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
- Linux: Secret Service-compatible keyring, jika tersedia.

Jika secure credential store tidak tersedia, Bucklane tidak boleh diam-diam menyimpan secret ke plaintext. Aplikasi harus menampilkan error atau menawarkan metode aman yang secara eksplisit didukung.

## 8.2 Credential Rotation

Ketika pengguna mengganti Secret Access Key:

1. Validasi konfigurasi baru.
2. Simpan credential baru dengan referensi baru.
3. Perbarui referensi dalam SQLite transaction.
4. Invalidasi client cache.
5. Hapus credential lama setelah pergantian berhasil.

Jika proses gagal, rollback atau recovery harus mencegah koneksi menunjuk ke secret yang tidak tersedia.

## 8.3 Endpoint Security

- HTTPS default.
- Certificate verification wajib secara default.
- Tidak mendukung opsi global ignore TLS errors pada MVP.
- HTTP hanya diizinkan melalui konfigurasi eksplisit.
- Endpoint dan region divalidasi sebelum client dibuat.
- Authorization headers dan secret tidak boleh masuk log.
- Redirect yang tidak diharapkan harus ditangani dengan aman, terutama untuk endpoint custom.

## 8.4 Tauri Security

- Minimal application capabilities.
- Hanya command yang diperlukan yang diregistrasikan.
- Tidak ada arbitrary shell execution.
- Tidak ada arbitrary filesystem read/write IPC.
- Content Security Policy dikonfigurasi.
- Tidak menggunakan remote scripts yang tidak dipercaya.

# 9. Transfer Engine Design

## 9.1 Architecture

TransferService mengelola:

- Transfer creation.
- Queue state.
- Concurrency management.
- Transfer execution.
- Progress reporting.
- Cancellation.
- Retry.
- Transfer persistence.

Tokio digunakan untuk asynchronous task execution.

Semaphore mengontrol jumlah transfer aktif.

Default concurrency: 3 transfer.

Concurrency dapat diubah melalui Settings.

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

Cancellation dapat diminta ketika transfer queued maupun running.

Task completion dan cancellation harus disinkronkan untuk mencegah status contradictory.

Ketika aplikasi restart, status RUNNING dari sesi sebelumnya direkonsiliasi menjadi FAILED dengan reason INTERRUPTED, kecuali tersedia mekanisme resume yang telah diverifikasi.

## 9.3 Upload Strategy

Small file:

- Single PutObject.
- Streaming file source.
- Progress tracking.

Large file:

- Multipart upload.
- Configurable part size.
- Bounded parallel part uploads.
- Complete multipart setelah seluruh part selesai.

Multipart sequence:

1. CreateMultipartUpload.
2. Read file chunks.
3. UploadPart.
4. Record completed part identifiers.
5. CompleteMultipartUpload.
6. Validate final result.

Jika terjadi kegagalan permanen sebelum completion, coba AbortMultipartUpload.

Jika abort gagal, catat upload ID secara aman untuk cleanup/recovery.

Part size harus memenuhi batasan S3, termasuk ukuran minimum part non-final dan jumlah maksimum part.

## 9.4 Download Strategy

1. Request object metadata.
2. Validate destination.
3. Create temporary local file in target filesystem.
4. Stream object data into temporary file.
5. Track progress.
6. Validate completion.
7. Finalize destination using safe replacement rules.

Download tidak boleh langsung menimpa file penting sebelum operasi selesai.

Temporary file yang gagal harus dibersihkan atau ditandai sebagai incomplete.

## 9.5 Retry Policy

Retry hanya dilakukan pada error yang layak dicoba ulang, misalnya:

- Temporary network errors.
- Connection reset.
- Selected server 5xx responses.
- Throttling.

Tidak otomatis retry pada:

- Invalid credentials.
- Access denied.
- Invalid object key.
- Unsupported operation.
- Permanent validation error.

Retry menggunakan exponential backoff dengan jitter dan batas percobaan.

Operasi non-idempotent atau yang mempunyai efek destruktif membutuhkan kebijakan retry khusus.

## 9.6 Copy / Move / Rename Safety

Rename dan move S3 menggunakan kombinasi copy dan delete.

Safe move sequence:

1. Validate source.
2. Validate destination.
3. Resolve overwrite policy.
4. Execute copy.
5. Verify copied destination.
6. Delete source only after successful verification.
7. Refresh explorer.

Untuk objek besar, provider mungkin membutuhkan multipart copy.

MVP harus memblokir copy/move untuk ukuran atau provider yang belum didukung secara aman, alih-alih mencoba operasi berisiko.

Object ETag tidak boleh selalu dianggap sebagai hash MD5 konten.

Concurrent modification dan versioning harus dipertimbangkan agar source yang telah berubah tidak dihapus secara tidak sengaja.

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
- Pagination state.
- Selection state.

**transferStore**

- Transfer items.
- Progress.
- Queue status.
- Error notifications.

Secrets tidak disimpan di Pinia secara persisten.

## 10.3 Explorer UI Requirements

Sidebar menampilkan connections dan navigasi bucket.

Main panel menampilkan:

- Breadcrumb.
- Upload button.
- New folder.
- Refresh.
- File listing.
- Context menu.
- Object metadata.

Transfer panel berada di bawah atau sebagai drawer.

Setiap operasi memiliki loading, empty, success, dan error state.

# 11. Error Model

Semua Rust errors yang diekspos ke frontend menggunakan format terstruktur.

```typescript
interface AppError {
  code: string;
  message: string;
  details?: string;
  retryable: boolean;
}
```

Error codes utama:

- CONNECTION_INVALID
- AUTHENTICATION_FAILED
- ACCESS_DENIED
- ENDPOINT_UNREACHABLE
- TLS_ERROR
- BUCKET_NOT_FOUND
- OBJECT_NOT_FOUND
- OPERATION_UNSUPPORTED
- TRANSFER_FAILED
- TRANSFER_CANCELED
- LOCAL_FILE_ERROR
- STORAGE_QUOTA_ERROR
- INTERNAL_ERROR

Pesan detail yang ditampilkan atau disimpan harus disanitasi agar tidak membocorkan secret maupun signed URL.

# 12. Observability

MVP menggunakan local structured logging.

Logging library: `tracing`.

Data yang boleh dicatat:

- Command name.
- Operation type.
- Provider type.
- Connection ID.
- Transfer ID.
- Duration.
- Sanitized error code.
- Result status.

Data yang tidak boleh dicatat:

- Secret Access Key.
- Authorization header.
- Session token.
- Signed URL.
- Isi file.
- Raw credential payload.

Telemetry jarak jauh tidak diaktifkan secara default.

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

Provider target:

- AWS S3.
- MinIO.
- Supabase Storage.

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

- Secret tidak tersimpan dalam SQLite.
- Secret tidak muncul pada logs.
- Secret tidak dikembalikan dalam connection DTO.
- Unsafe HTTP memerlukan explicit opt-in.
- Destructive actions memerlukan konfirmasi.
- Destination path traversal tidak diizinkan.
- Tauri IPC tidak membuka arbitrary filesystem access.

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

GitHub Actions bertanggung jawab untuk:

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

Code signing dan notarization menjadi release requirements untuk distribusi publik yang matang, sesuai sistem operasi.

Autoupdate tidak boleh menerima update tanpa signature verification.

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

Deliverables:

- Upload/download.
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

Implementasi teknis dianggap selesai apabila:

1. Requirement PRD terpenuhi.
2. API/IPC contract sesuai spesifikasi.
3. Unit dan integration tests tersedia.
4. Error handling diterapkan.
5. Security checks terpenuhi.
6. Tidak ada known critical data loss.
7. Dokumentasi diperbarui.
8. Perubahan lulus CI.
9. Telah direview.
10. Kompatibilitas provider relevan telah diuji.

---

**End of Document — Bucklane TSD v1.0**
