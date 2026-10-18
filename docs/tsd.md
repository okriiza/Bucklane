# Bucklane — Technical Specification Document

**Document ID:** BUCK-TSD-001  
**Version:** 1.0  
**Status:** Draft for Implementation  
**Related Documents:** Bucklane PRD v1.0, [Database Design v1.0](database_design.md)  
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

MVP hanya mendukung **static access key ID + secret access key**.

Tidak didukung pada MVP:

- Session token / STS temporary credentials.
- AWS shared config/credentials profiles.
- AWS SSO / IAM Identity Center.
- Instance metadata atau default credential chain.

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
- BucketBookmarkRepository
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

Representasi string (IPC dan SQLite): `aws`, `supabase`, `minio`, `cloudflare_r2`, `wasabi`, `digitalocean_spaces`, `custom`.

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

Transfer menyimpan connection ID (nullable setelah koneksi dihapus), snapshot nama koneksi, bucket, object key, local path, jumlah bytes, status, retry count, error code/message jika ada, dan multipart upload ID bila relevan.

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

File transfer diorkestrasi oleh TransferService (scheduler, progress, retry, cancel), sedangkan StorageProvider hanya menyediakan operasi S3 primitif di atas.

Operasi upload/download tidak boleh memuat seluruh konten file ke memory; body request/response bersifat streaming.

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

Client boleh di-cache secara in-memory menggunakan key `(connection_id, updated_at)`; `updated_at` berfungsi sebagai revision konfigurasi.

Cache harus diinvalidasi ketika connection diperbarui, credential dirotasi, atau connection dihapus.

**AWS bucket region resolution:** ListBuckets pada AWS mengembalikan bucket dari semua region, sedangkan client dikonfigurasi untuk satu region. Untuk koneksi AWS tanpa custom endpoint, region bucket dideteksi sekali melalui header `x-amz-bucket-region` dari HeadBucket (header ini tetap dikirim pada respons 301/403), di-cache per `(connection_id, bucket)`, dan client untuk region tersebut dibuat dengan credentials yang sama. Pengguna tidak perlu membuat satu koneksi per region.

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

**Wasabi**

- Endpoint region-specific milik Wasabi.
- Region wajib dan harus sesuai endpoint.
- HTTPS enabled.
- Compatibility testing wajib.

**DigitalOcean Spaces**

- Endpoint berbasis region Spaces.
- Region mengikuti datacenter Spaces.
- HTTPS enabled.
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

## 6.0 Type Generation

Seluruh command input/output dan event payload didefinisikan sebagai struct Rust (`serde`, `camelCase`), lalu tipe TypeScript di-generate dengan `tauri-specta` ke `src/services/tauri/bindings.ts`. File hasil generate tidak diedit manual; CI gagal jika bindings tidak sinkron.

Contoh TypeScript di bawah bersifat ilustratif; sumber kebenarannya adalah struct Rust.

Semua command mengembalikan `Result<T, AppError>` (lihat §11).

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

`list_provider_profiles` mengembalikan default dan petunjuk form per provider (label, endpoint wajib/opsional, placeholder, default region, default path-style), sehingga frontend tidak menyimpan salinan konfigurasi provider sendiri.

`delete_connection` mengembalikan `{ secretRemoved: boolean }`; jika `false`, koneksi sudah terhapus tetapi entri credential store gagal dibersihkan dan UI menampilkan peringatan.

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

- `secretAccessKey` kosong/tidak dikirim → secret lama tetap digunakan.
- `secretAccessKey` diisi → jalankan alur credential rotation (§8.2).
- Jika `accessKeyId` berubah, `secretAccessKey` **wajib** diisi.
- Setiap update menginvalidasi client cache untuk koneksi tersebut.

TestConnection harus dapat menggunakan konfigurasi sementara tanpa menyimpan credentials secara permanen. Jika ListBuckets ditolak dan `testBucket` diisi, pengujian menggunakan HeadBucket pada bucket tersebut.

## 6.2 Bucket Commands

```text
list_buckets
head_bucket
list_bucket_bookmarks
add_bucket_bookmark
update_bucket_bookmark
remove_bucket_bookmark
```

`list_buckets` menerima connection ID.

`head_bucket` menerima connection ID dan nama bucket.

Bookmark commands mengelola tabel `connection_buckets`. Jika ListBuckets ditolak, pengguna tetap dapat membuka bucket melalui bookmark; akses diverifikasi dengan `head_bucket` saat dibuka.

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

`preview_object` mengambil isi objek untuk preview di aplikasi: gambar (≤ 15 MB) dan PDF (≤ 25 MB) utuh, teks hanya 512 KB pertama melalui HTTP Range. Isi dikirim sebagai base64 dan ditampilkan melalui `blob:` URL, sehingga CSP cukup ditambah `img-src blob:` dan `frame-src blob:` tanpa mengizinkan origin remote.

`create_share_link` membuat presigned GET URL untuk satu objek (60 detik sampai 7 hari, batas SigV4). Keberadaan objek dicek lebih dulu. URL tidak disimpan maupun dicatat di log karena berfungsi sebagai bearer credential (SEC-004); link hanya bisa dicabut lebih awal dengan merotasi access key.

Rename adalah `move_object` dengan bucket yang sama dan key tujuan baru; tidak ada command rename terpisah.

`copy_object` dan `move_object` menerima `conflictPolicy` (`ASK` | `OVERWRITE`) dan mengembalikan union berlabel:

```typescript
type ObjectOperationOutcome =
  | { kind: "completed" }
  | { kind: "conflict"; existingSize: number | null; existingLastModified: string | null } // tidak ada yang diubah
  | { kind: "sourceKept"; reason: string }; // move: copy berhasil, sumber sengaja dipertahankan
```

Copy menggunakan `x-amz-copy-source-if-match` dengan ETag sumber agar sumber yang berubah di tengah proses tidak tersalin. Move hanya menghapus sumber setelah tujuan terverifikasi (ukuran sama) dan ETag sumber dicek ulang tidak berubah; jika ragu, hasilnya `sourceKept`.

`copy_object` dan `move_object` hanya berlaku untuk objek tunggal pada MVP. Copy/move folder (prefix) termasuk Post-MVP. Objek berukuran di atas 5 GiB ditolak dengan `OPERATION_UNSUPPORTED` karena membutuhkan multipart copy (lihat §9.6).

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

Pencarian nama di server memakai listing yang sama dengan `prefix = folder + query` (query tanpa `/`). S3 hanya mencocokkan awal key dan bersifat case-sensitive, jadi hasilnya adalah item di folder aktif yang namanya diawali query. Field `name` selalu relatif terhadap folder yang memuat prefix (bagian sampai `/` terakhir), sehingga hasil pencarian tetap menampilkan nama lengkap.

### 6.3.1 Bulk Delete Result

```typescript
interface DeleteObjectsResult {
  deleted: string[];
  failed: { key: string; error: AppError }[];
}
```

`delete_objects` menggunakan DeleteObjects API dalam batch maksimal 1.000 key. Jika provider tidak mendukung DeleteObjects, fallback ke DeleteObject per key. Partial failure selalu dilaporkan; UI tidak boleh menampilkan sukses penuh jika `failed` tidak kosong.

### 6.3.2 Folder (Prefix) Delete

Menghapus folder berarti menghapus seluruh objek di bawah prefix, sehingga dilakukan dalam dua tahap:

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

1. `prepare_prefix_delete(connectionId, bucket, prefix)` — backend me-list seluruh objek di bawah prefix (tanpa delimiter), menyimpan snapshot key di memory, dan mengembalikan `PrefixDeletePlan`.
2. UI menampilkan konfirmasi berisi nama bucket, prefix, jumlah objek, dan total ukuran.
3. `execute_prefix_delete(planId)` — backend menghapus **hanya key dalam snapshot**, sehingga objek yang ditambahkan setelah plan dibuat tidak ikut terhapus. Hasil menggunakan `DeleteObjectsResult`.

Ketentuan:

- Plan kedaluwarsa setelah 10 menit dan hanya dapat dieksekusi sekali.
- Plan dengan lebih dari 100.000 objek ditolak pada MVP dengan `OPERATION_UNSUPPORTED`.
- Prefix kosong (root bucket) tidak diizinkan.

## 6.4 Local File Selection

Frontend **tidak pernah** mengirim path filesystem ke backend untuk operasi baca/tulis. File dan folder lokal hanya dapat digunakan setelah dipilih pengguna melalui dialog native yang dibuka dari sisi Rust.

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

Ketentuan:

- Backend membuka dialog menggunakan `tauri-plugin-dialog` dari Rust dan menyimpan path hasil pilihan di `AppState` (in-memory registry).
- Frontend hanya menerima `selectionId` yang opaque.
- Selection berlaku 30 menit dan tidak dipersistenkan.
- Drag & drop (FIL-008) diproses melalui window drag-drop event di Rust, yang mendaftarkan selection dan mengirim `selectionId` ke frontend.
- Plugin `fs` Tauri tidak digunakan.
- Transfer yang sudah dibuat menyimpan path di SQLite; `retry_transfer` menggunakan path tersebut, bukan selection baru.

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

`reveal_transfer(id)` menampilkan file lokal sebuah transfer di Finder / File Explorer. Path hanya dibaca di backend dari baris transfer; frontend cukup mengirim ID. Plugin opener dipakai sebagai library Rust dan tidak didaftarkan, sehingga tidak ada command opener yang terbuka ke frontend.

Operasi berjalan melalui scheduler tanpa menahan IPC request hingga transfer selesai.

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

- `pick_upload_folder` dan drag-and-drop folder mendaftarkan selection jenis `UploadDirectory`. Selection ini hanya bisa dibaca: tidak dapat dipakai sebagai tujuan download.
- `enqueue_upload` mengekspansi folder secara rekursif (symlink dan file metadata OS seperti `.DS_Store` dilewati); setiap file menjadi transfer terpisah dengan key `<prefix><nama folder>/<path relatif>`. Keberadaan objek dicek dengan satu listing prefix tujuan, bukan HEAD per file.
- `enqueue_download` menerima key folder (berakhiran `/`); semua objek di bawahnya diunduh dengan struktur folder dibuat ulang di tujuan. Setiap segmen path disanitasi sehingga key tidak dapat menempatkan file di luar folder tujuan.
- Batas 10.000 file per folder. Item konflik untuk file di dalam folder diberi ID `<folder><U+001F><path/key>` sehingga pengiriman ulang dengan pilihan pengguna hanya memproses file tersebut.

### 6.5.1 Conflict Policy

Upload:

1. Untuk setiap file, backend menjalankan HeadObject pada key tujuan.
2. Jika key sudah ada dan `conflictPolicy = "ASK"`, item **tidak** dimasukkan ke antrean dan dikembalikan sebagai `conflict`.
3. UI menampilkan dialog Overwrite / Skip / Rename (dengan opsi "terapkan ke semua"), lalu memanggil `enqueue_upload` kembali untuk item tersebut dengan policy yang dipilih.
4. `RENAME` menghasilkan key `name (1).ext`, `name (2).ext`, dst. hingga key belum digunakan.

Download:

- Aturan yang sama berlaku terhadap file di direktori tujuan.
- Keberadaan file tujuan diperiksa ulang saat finalisasi (§9.4); jika muncul file baru di antara pengecekan dan finalisasi, transfer gagal dengan `LOCAL_FILE_ERROR` alih-alih menimpa.

Pengecekan konflik upload bersifat best-effort, bukan atomic, karena conditional write tidak didukung seragam oleh semua provider.

### 6.5.2 Retry & History

- `retry_transfer` hanya berlaku untuk status `FAILED`. Transfer yang sama berpindah ke `QUEUED`, `retry_count` bertambah, dan error dibersihkan.
- Untuk multipart upload, retry mencoba `AbortMultipartUpload` pada upload ID lama (best-effort), menghapus `transfer_parts` terkait, lalu memulai upload dari awal.
- `clear_transfer_history` hanya menghapus transfer berstatus terminal (`COMPLETED`, `FAILED`, `CANCELED`). Transfer dengan multipart upload yang belum berhasil dibersihkan tidak ikut dihapus.

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

Settings dikirim sebagai satu objek bertipe dan divalidasi oleh SettingsService; penyimpanan tetap key/value JSON di tabel `settings` (DB design §5). Nilai tersimpan yang tidak valid diabaikan dan diganti default. `update_settings` langsung menerapkan setting transfer ke TransferManager; menurunkan batas concurrency tidak menghentikan transfer yang sedang berjalan.

Hanya setting yang benar-benar diterapkan aplikasi yang dapat diubah. Opsi yang melemahkan keamanan data (misalnya menonaktifkan konfirmasi delete/overwrite, SEC-005) sengaja tidak disediakan.

## 6.7 Events

Event didefinisikan sebagai struct Rust dan di-generate oleh `tauri-specta` (`events.*` di `bindings.ts`); nama event diturunkan dari nama struct.

| Event | Payload | Kapan |
| --- | --- | --- |
| `transfer-update` | `TransferDto` lengkap | Setiap perubahan status dan berkala selama RUNNING |
| `files-dropped` | `{ files: LocalFileSelection[], skipped: number }` | File dari OS di-drop ke jendela; sudah terdaftar di selection registry |
| `file-drag-hover` | `boolean` | File dari OS mulai/berhenti diseret di atas jendela |

Satu event `transfer-update` dengan DTO lengkap menggantikan lima event terpisah (`started`/`progress`/`completed`/`failed`/`canceled`): frontend cukup melakukan upsert berdasarkan `id`, dan tidak ada kemungkinan urutan event yang membuat status kontradiktif.

Progress dikirim setiap 300 ms dan dipersistenkan ke SQLite sekitar setiap 2 detik.

Event bersifat informasi sementara. SQLite dan runtime transfer manager menjadi sumber status otoritatif. Frontend memuat ulang status melalui `list_transfers` saat startup.

# 7. Local Database Design

Database: SQLite via SQLx.

Lokasi database mengikuti application data directory masing-masing OS.

Skema lengkap, constraint, index, migration awal, dan data lifecycle didefinisikan di **[Database Design](database_design.md)**, yang menjadi sumber kebenaran tunggal untuk skema. Bagian ini hanya meringkas.

## 7.1 Tables

| Table                | Purpose                                                      |
| -------------------- | ------------------------------------------------------------ |
| `connections`        | Profil koneksi non-secret + `credential_ref` (UNIQUE, 1:1)   |
| `connection_buckets` | Bookmark bucket per koneksi (cascade saat koneksi dihapus)   |
| `transfers`          | Riwayat dan status transfer                                  |
| `transfer_parts`     | Part multipart upload yang sudah selesai (cascade)           |
| `settings`           | Preferensi aplikasi dalam JSON-encoded value                 |

## 7.2 Connection Deletion Policy

1. Transfer aktif (`QUEUED`/`RUNNING`) milik koneksi harus dibatalkan atau dihentikan terlebih dahulu.
2. Koneksi dihapus; bookmark ikut terhapus (`ON DELETE CASCADE`).
3. Riwayat transfer **dipertahankan**: `transfers.connection_id` menjadi `NULL` (`ON DELETE SET NULL`) dan `connection_name_snapshot` tetap menampilkan nama koneksi.
4. Secret dihapus dari OS credential store.

## 7.3 Migration Strategy

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

Pada Linux tanpa Secret Service (misalnya tanpa GNOME Keyring/KWallet), koneksi tidak dapat disimpan. Aplikasi menampilkan error yang menjelaskan dependency yang dibutuhkan; dokumentasi instalasi Linux mencantumkan dependency ini.

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
- Tidak ada arbitrary filesystem read/write IPC; path lokal hanya melalui selection registry (§6.4).
- Plugin `fs` dan `shell` tidak digunakan.
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

Default concurrency: 3 transfer; di dalam satu multipart upload, maksimal 4 part diunggah paralel.

Concurrency, threshold multipart (default 64 MiB), dan ukuran part (default 16 MiB, dinaikkan otomatis agar ≤ 10.000 part dan tidak di bawah 5 MiB) dibaca dari Settings dan dapat diubah saat aplikasi berjalan.

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

Ketika aplikasi restart, status QUEUED dan RUNNING dari sesi sebelumnya direkonsiliasi menjadi FAILED dengan `error_code = INTERRUPTED` (transfer tidak dimulai otomatis tanpa tindakan pengguna), kecuali tersedia mekanisme resume yang telah diverifikasi.

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

MVP harus memblokir copy/move untuk ukuran atau provider yang belum didukung secara aman, alih-alih mencoba operasi berisiko. Batas MVP: objek di atas 5 GiB (batas CopyObject single-request) ditolak dengan `OPERATION_UNSUPPORTED`; ukuran diperiksa melalui HeadObject sebelum copy.

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
- Server search query (dikosongkan saat pindah folder).
- Sort order (nama, ukuran, tanggal; folder selalu di atas). Diurutkan di client atas item yang sudah dimuat, disimpan di localStorage sebagai preferensi.
- Selection state.

**transferStore**

- Transfer items.
- Progress.
- Queue status.
- Error notifications.

Secrets tidak disimpan di Pinia secara persisten.

Semua teks UI melalui `vue-i18n` sejak awal agar bahasa lain dapat ditambahkan tanpa refactor; MVP hanya menyediakan English.

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

Log ditulis ke application log directory masing-masing OS menggunakan `tracing-appender` dengan rotasi harian dan retensi 7 file. Level mengikuti setting `logging.level`.

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

Provider target dan cara eksekusi:

| Provider         | Eksekusi                                                        |
| ---------------- | --------------------------------------------------------------- |
| S3-compatible lokal (RustFS) | Setiap PR di CI melalui `docker compose` (`BUCKLANE_S3_TEST_ENDPOINT`) |
| MinIO            | Manual terhadap server MinIO; image Docker MinIO tidak lagi dipublikasikan |
| AWS S3           | Nightly atau manual, credentials dari GitHub repository secrets |
| Supabase Storage | Nightly atau manual, credentials dari GitHub repository secrets |

Integration test yang membutuhkan credentials eksternal di-skip secara eksplisit (bukan gagal) jika secrets tidak tersedia, misalnya pada PR dari fork.

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
- Command transfer menolak path mentah; hanya `selectionId` yang valid dan belum kedaluwarsa diterima.
- Prefix delete hanya menghapus key dalam snapshot plan.
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

Upload/download dipindahkan ke M04 karena bergantung pada transfer engine (antrean, progress, cancel, multipart).

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

- Upload/download (dengan conflict policy dan local file selection registry).
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
