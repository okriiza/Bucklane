# Product Requirements Document (PRD)

## Bucklane — Desktop S3 Storage Manager

**Version:** 1.0  
**Status:** Draft — Ready for Technical Design  
**Platform:** macOS, Windows, Linux  
**Product Type:** Cross-platform Desktop Application  
**Architecture:** Local-first, without centralized backend  
**Technology:** Tauri 2, Vue 3, TypeScript, Rust, SQLite  
**Initial Release:** MVP v1.0

---

# 1. Executive Summary

Bucklane adalah aplikasi desktop cross-platform untuk mengelola object storage berbasis Amazon S3 API dan berbagai penyedia S3-compatible storage melalui satu antarmuka modern.

Bucklane dirancang sebagai alternatif aplikasi seperti Cyberduck, dengan fokus pada kemudahan manajemen koneksi, eksplorasi bucket, pengelolaan file, transfer data, keamanan kredensial, serta dukungan multi-provider tanpa pembatasan jumlah koneksi.

Aplikasi berjalan secara lokal dan mengakses object storage secara langsung tanpa memerlukan backend cloud terpusat atau pembuatan akun Bucklane.

## 1.1 Product Vision

**One application to manage all your S3-compatible storage.**

Menjadi aplikasi desktop yang ringan, aman, dan mudah digunakan untuk mengelola berbagai layanan object storage dari satu tempat.

## 1.2 Problem Statement

Pengguna object storage sering menghadapi beberapa masalah:

1. Memerlukan beberapa tools untuk mengakses berbagai penyedia storage.
2. Beberapa aplikasi memiliki batasan koneksi atau fitur berdasarkan lisensi.
3. Manajemen file melalui CLI tidak selalu praktis untuk pekerjaan sehari-hari.
4. Interface manajemen object storage sering kali terlalu teknis.
5. Transfer file besar memerlukan monitoring progress dan penanganan kegagalan.
6. Pengelolaan kredensial dari banyak provider menjadi tidak praktis.
7. Fitur dan kompatibilitas S3 berbeda antar-provider.

Bucklane bertujuan menyelesaikan masalah tersebut melalui antarmuka desktop yang konsisten dengan sistem manajemen koneksi dan file terintegrasi.

# 2. Product Goals

## 2.1 Primary Goals

- Menyediakan aplikasi desktop untuk mengelola object storage S3-compatible.
- Mendukung penyimpanan koneksi tanpa batasan jumlah yang ditetapkan aplikasi.
- Menyediakan file explorer yang intuitif.
- Mendukung upload dan download file secara langsung.
- Menyediakan transfer manager dengan progress dan error handling.
- Melindungi access key dan secret key pengguna.
- Menyediakan pengalaman konsisten pada macOS, Windows, dan Linux.

## 2.2 Secondary Goals

- Menjadi aplikasi open-source yang dapat dikembangkan komunitas.
- Mendukung provider S3 baru dengan konfigurasi minimal.
- Memberikan pengalaman penggunaan yang ringan dan responsif.
- Menyediakan fondasi pengembangan untuk sinkronisasi file dan transfer antar-storage.

## 2.3 Non-Goals MVP

MVP tidak mencakup:

- Cloud synchronization dua arah.
- Kolaborasi multi-user.
- Sistem akun SaaS.
- Penyimpanan file melalui server Bucklane.
- Pengelolaan IAM policy tingkat lanjut.
- Manajemen lifecycle dan versioning.
- Object replication.
- Transfer langsung antar-provider.
- Koneksi SFTP, FTP, atau WebDAV.
- Kredensial selain static access key (session token/STS, AWS profiles, SSO).
- Copy/move folder (prefix) secara rekursif.
- Copy/move objek berukuran di atas 5 GiB.

# 3. Target Users

## 3.1 Software Developers

Developer yang menggunakan object storage untuk menyimpan dokumen, gambar, video, backup, atau file aplikasi.

**Kebutuhan:** akses cepat terhadap file, upload/download, debugging object path, dan pemantauan transfer.

## 3.2 DevOps & System Administrators

Pengguna yang mengelola beberapa object storage untuk development, staging, dan production.

**Kebutuhan:** multi-connection, navigasi bucket, operasi massal, keamanan koneksi, dan pengelolaan file berukuran besar.

## 3.3 Technical Teams

Tim teknis yang membutuhkan antarmuka visual tanpa harus selalu menggunakan AWS CLI, rclone, atau tools terminal lainnya.

**Kebutuhan:** kemudahan navigasi dan operasi file standar.

# 4. Supported Storage Providers

| Provider                      | MVP Priority |
| ----------------------------- | ------------ |
| Amazon S3                     | P0           |
| Supabase Storage S3           | P0           |
| MinIO                         | P0           |
| Cloudflare R2                 | P1           |
| Wasabi                        | P1           |
| DigitalOcean Spaces           | P1           |
| Custom S3-compatible endpoint | P0           |

**Ketentuan:**

- Provider preset menggunakan konfigurasi default yang sesuai dengan penyedia masing-masing.
- Custom endpoint dapat diatur secara manual.
- Region dan path-style addressing dapat dikonfigurasi.
- Tidak semua provider mendukung seluruh operasi AWS S3.
- Fitur yang tidak didukung harus dinonaktifkan atau menampilkan informasi yang sesuai.
- Kompatibilitas harus diuji per provider, bukan diasumsikan berdasarkan dukungan S3 API.

# 5. Core Functional Requirements

## 5.1 Connection Management

**Module ID:** CON

### CON-001 — Create Connection

**Priority:** P0

Pengguna dapat menambahkan koneksi object storage baru.

Input:

- Connection Name
- Storage Provider
- Endpoint URL
- Region
- Access Key ID
- Secret Access Key
- Force Path Style
- HTTPS Configuration

**Acceptance Criteria:**

- Field yang wajib harus divalidasi.
- Pengguna dapat memilih preset provider atau custom endpoint.
- Secret key disimpan menggunakan secure credential storage sistem operasi.
- Sistem tidak menyimpan secret key dalam database plaintext.
- Sistem menampilkan pesan validasi yang jelas.

### CON-002 — Test Connection

**Priority:** P0

Pengguna dapat menguji konfigurasi sebelum atau setelah menyimpan koneksi.

**Acceptance Criteria:**

- Sistem melakukan request autentikasi/akses ke endpoint.
- Status sukses ditampilkan dengan jelas.
- Error autentikasi, endpoint, permission, dan jaringan ditangani secara terpisah jika dapat diidentifikasi.
- Koneksi yang tidak memiliki izin untuk listing semua bucket tidak otomatis dinyatakan tidak valid; pengujian dapat menggunakan bucket tertentu jika dikonfigurasi.

### CON-003 — Saved Connections

**Priority:** P0

Pengguna dapat melihat seluruh koneksi yang tersimpan.

**Acceptance Criteria:**

- Tidak ada pembatasan jumlah koneksi secara sengaja.
- Koneksi tetap tersedia setelah aplikasi ditutup.
- Pengguna dapat memilih dan membuka koneksi.
- Daftar koneksi dapat dicari berdasarkan nama.

### CON-004 — Edit & Delete Connection

**Priority:** P0

Pengguna dapat memperbarui atau menghapus konfigurasi koneksi.

**Acceptance Criteria:**

- Perubahan konfigurasi dapat disimpan.
- Penghapusan memerlukan konfirmasi.
- Penghapusan koneksi tidak menghapus bucket maupun objek di storage provider.
- Kredensial terkait dibersihkan dari credential store ketika koneksi dihapus. Setiap koneksi memiliki credential reference sendiri (relasi 1:1); kredensial tidak dibagi antar-koneksi.
- Riwayat transfer koneksi yang dihapus tetap dipertahankan menggunakan snapshot nama koneksi.

## 5.2 Bucket Management

**Module ID:** BKT

### BKT-001 — List Buckets

**Priority:** P0

Sistem menampilkan bucket yang dapat diakses pengguna.

**Acceptance Criteria:**

- Daftar bucket ditampilkan jika kredensial memiliki permission yang diperlukan.
- Nama bucket dapat dipilih untuk membuka file explorer.
- Status loading dan error tersedia.
- Jika listing bucket tidak diizinkan, pengguna dapat membuka bucket tertentu yang telah dikonfigurasi secara manual (lihat BKT-005).

### BKT-005 — Bucket Bookmarks

**Priority:** P0

Pengguna dapat menyimpan nama bucket secara manual per koneksi agar dapat dibuka langsung tanpa permission ListBuckets.

**Acceptance Criteria:**

- Pengguna dapat menambah, memberi label, dan menghapus bookmark bucket.
- Nama bucket yang sama tidak dapat di-bookmark dua kali pada koneksi yang sama.
- Bookmark tidak dianggap sebagai bukti bahwa bucket ada atau dapat diakses; status akses diverifikasi saat dibuka.
- Bookmark ikut terhapus ketika koneksi dihapus.

### BKT-002 — Bucket Navigation

**Priority:** P0

Pengguna dapat membuka bucket dan menavigasi daftar objek.

**Acceptance Criteria:**

- Menampilkan prefix/folder dan objek.
- Mendukung pagination.
- Mendukung navigasi breadcrumb.
- Mendukung refresh.
- Tidak mengunduh seluruh isi bucket saat pertama kali dibuka.

### BKT-003 — Create Bucket

**Priority:** P2 — Post-MVP

Pembuatan bucket akan ditambahkan setelah kompatibilitas dan permission handling dasar stabil.

### BKT-004 — Delete Bucket

**Priority:** P2 — Post-MVP

Penghapusan bucket tidak termasuk MVP karena berisiko tinggi dan memiliki ketentuan berbeda antar-provider.

## 5.3 File Explorer

**Module ID:** EXP

### EXP-001 — Browse Objects

**Priority:** P0

Pengguna dapat melihat objek dalam suatu bucket.

Informasi yang ditampilkan:

- File name/object key
- File type
- File size
- Last modified
- Folder/prefix

### EXP-002 — Folder Navigation

**Priority:** P0

Pengguna dapat menavigasi struktur folder virtual berdasarkan object prefix.

**Acceptance Criteria:**

- Breadcrumb dapat digunakan.
- Folder dapat dibuka.
- Navigasi kembali tersedia.
- Folder kosong yang direpresentasikan menggunakan marker dapat ditampilkan.

### EXP-003 — Create Folder

**Priority:** P0

Pengguna dapat membuat folder virtual.

**Acceptance Criteria:**

- Sistem membuat folder marker object.
- Nama folder divalidasi.
- Prefix baru dapat dinavigasi.
- Folder marker tidak dianggap sebagai filesystem directory sesungguhnya.

### EXP-004 — Object Search

**Priority:** P1

Pencarian objek berdasarkan nama atau prefix.

**Ketentuan MVP:**

- Filter pada objek yang sudah dimuat.
- Pencarian prefix server-side jika didukung.
- Tidak menjanjikan pencarian substring menyeluruh untuk seluruh isi bucket.

### EXP-005 — Object Metadata

**Priority:** P1

Menampilkan metadata dasar seperti key, size, last modified, content type, dan ETag jika tersedia.

## 5.4 File Operations

**Module ID:** FIL

### FIL-001 — Upload File

**Priority:** P0

Pengguna dapat mengunggah satu atau beberapa file lokal ke bucket.

**Acceptance Criteria:**

- Pengguna dapat memilih file.
- Tujuan bucket dan prefix dapat dipilih.
- Transfer progress ditampilkan.
- Kegagalan upload dapat diketahui.
- Jika key tujuan sudah ada, pengguna memilih Overwrite, Skip, atau Rename (`name (1).ext`), dengan opsi "terapkan ke semua". Default: tanya pengguna; tidak ada overwrite tanpa pilihan eksplisit.
- UI tetap responsif selama transfer.

### FIL-002 — Download File

**Priority:** P0

Pengguna dapat mengunduh objek ke filesystem lokal.

**Acceptance Criteria:**

- Pengguna memilih lokasi penyimpanan.
- Progress transfer ditampilkan.
- File lokal yang sudah ada tidak ditimpa diam-diam; pengguna memilih Overwrite, Skip, atau Rename seperti pada upload.
- Kegagalan download ditangani dengan aman.
- File parsial tidak diperlakukan sebagai download berhasil.

### FIL-003 — Rename Object

**Priority:** P0

Pengguna dapat mengganti nama objek.

**Ketentuan Teknis:**

Rename dilakukan dengan copy object ke key baru, lalu menghapus key lama setelah hasil copy tervalidasi.

**Acceptance Criteria:**

- Sistem tidak menghapus objek asal sebelum copy sukses.
- Kegagalan copy tidak menyebabkan penghapusan objek asal.
- Error dapat ditampilkan dan dipulihkan secara manual jika perlu.

### FIL-004 — Copy Object

**Priority:** P0

Pengguna dapat menyalin objek dalam storage yang sama.

**Batasan MVP:** hanya objek tunggal; objek di atas 5 GiB ditolak dengan pesan yang jelas.

### FIL-005 — Move Object

**Priority:** P0

Pengguna dapat memindahkan objek ke prefix lain menggunakan proses copy lalu delete.

**Batasan MVP:** sama dengan FIL-004.

### FIL-006 — Delete Object

**Priority:** P0

Pengguna dapat menghapus satu atau beberapa objek.

**Acceptance Criteria:**

- Penghapusan memerlukan konfirmasi.
- Nama bucket dan jumlah objek ditampilkan.
- Operasi partial failure dilaporkan.
- Sistem tidak mengklaim seluruh operasi berhasil jika sebagian gagal.

**Hapus folder (prefix):**

- Sistem terlebih dahulu menghitung seluruh objek di bawah prefix, lalu menampilkan jumlah objek dan total ukuran sebelum konfirmasi.
- Hanya objek yang terhitung saat konfirmasi yang dihapus; objek baru yang muncul setelahnya tidak ikut terhapus.
- Penghapusan root bucket (prefix kosong) tidak diizinkan.
- Folder dengan lebih dari 100.000 objek tidak dapat dihapus melalui MVP.

### FIL-007 — Bulk Selection

**Priority:** P1

Mendukung pemilihan beberapa objek untuk download dan delete.

### FIL-008 — Drag & Drop Upload

**Priority:** P1

Pengguna dapat menyeret file dari filesystem ke jendela aplikasi untuk melakukan upload.

# 6. Transfer Management

**Module ID:** TRF

## TRF-001 — Transfer Queue

**Priority:** P0

Setiap upload/download dimasukkan ke transfer queue.

Status:

- QUEUED
- RUNNING
- COMPLETED
- FAILED
- CANCELED

## TRF-002 — Transfer Progress

**Priority:** P0

Menampilkan:

- File name
- Transfer direction
- Total bytes
- Transferred bytes
- Percentage
- Current status
- Error message jika terjadi kegagalan

Kecepatan transfer dan ETA dapat ditambahkan apabila pengukurannya tersedia dengan akurat.

## TRF-003 — Retry Transfer

**Priority:** P0

Pengguna dapat menjalankan ulang transfer yang gagal.

## TRF-004 — Cancel Transfer

**Priority:** P0

Pengguna dapat membatalkan transfer aktif.

Cancellation harus menghentikan task yang dapat dibatalkan dan menangani file sementara atau multipart upload secara aman.

## TRF-005 — Multipart Upload

**Priority:** P0

File besar menggunakan multipart upload apabila didukung oleh provider dan batas ukuran yang ditentukan.

**Acceptance Criteria:**

- Bagian file dapat diunggah secara bertahap.
- Progress total dapat dihitung.
- Multipart upload diselesaikan setelah seluruh part berhasil.
- Multipart upload yang gagal dapat dibersihkan atau ditandai untuk pemulihan.
- Resume setelah aplikasi restart tidak dijanjikan pada MVP.

## TRF-006 — Concurrent Transfer

**Priority:** P0

Aplikasi mendukung beberapa transfer aktif secara bersamaan dengan batas concurrency yang dapat dikonfigurasi.

# 7. Desktop User Experience

## 7.1 Main Layout

Aplikasi menggunakan tiga area utama:

**Left Sidebar**

- Saved connections
- Storage providers
- Bucket navigation
- Settings access

**Main Explorer**

- Breadcrumb
- Toolbar
- Files and folders
- Metadata
- Object actions

**Bottom Transfer Panel**

- Transfer queue
- Upload/download progress
- Retry and cancel
- Transfer history

## 7.2 Application Pages

1. Welcome / Empty State
2. Connection Manager
3. Add/Edit Connection
4. Bucket & File Explorer
5. Transfer Manager
6. Settings
7. About

## 7.3 UI Requirements

- Light mode dan dark mode.
- Responsive desktop layout.
- Loading indicator.
- Empty state.
- Error notifications.
- Confirmation dialogs.
- Keyboard navigation dasar.
- Native file picker.
- Informasi progres yang jelas.
- UI tidak terblokir oleh operasi jaringan.

# 8. Security Requirements

**SEC-001 — Credential Protection**

Secret Access Key wajib disimpan menggunakan secure credential storage OS.

**SEC-002 — No Plaintext Secret Database**

SQLite hanya menyimpan credential reference dan metadata non-secret.

**SEC-003 — Secure Communication**

HTTPS menjadi default. Penggunaan HTTP custom endpoint harus memerlukan konfigurasi eksplisit dan peringatan keamanan.

**SEC-004 — Sensitive Logging**

Aplikasi tidak boleh mencatat secret, authorization header, token, atau signed URL lengkap ke log.

**SEC-005 — Destructive Operations**

Delete dan overwrite harus dilindungi oleh konfirmasi yang sesuai.

**SEC-006 — Permission Handling**

Aplikasi harus tetap berfungsi untuk akun read-only, sejauh operasi yang dibutuhkan mendapatkan izin.

**SEC-007 — Local-First**

Kredensial dan file tidak dikirimkan ke backend milik Bucklane.

**SEC-008 — Desktop Permissions**

Akses filesystem, network, dan Tauri commands harus dibatasi mengikuti prinsip least privilege.

# 9. Non-Functional Requirements

## 9.1 Performance Targets

Target awal yang akan diverifikasi melalui pengujian, bukan klaim kinerja yang telah terbukti:

| Metric               | MVP Target                                                        |
| -------------------- | ----------------------------------------------------------------- |
| UI startup           | Kurang dari 3 detik pada perangkat benchmark                      |
| Explorer navigation  | Tidak memblokir UI selama request                                 |
| File listing         | Pagination, maksimum 1.000 objek per request jika API mengizinkan |
| UI responsiveness    | Tetap interaktif saat transfer berjalan                           |
| Connection storage   | Tidak ada application-level connection limit                      |
| Transfer concurrency | Default 3, dapat dikonfigurasi                                    |
| Memory utilization   | Diprofilkan saat idle dan transfer besar                          |
| Large file handling  | Streaming, bukan full-file buffering                              |

## 9.2 Reliability

- Network timeout harus ditangani.
- Temporary errors dapat dicoba kembali dengan backoff.
- Operasi destruktif tidak boleh dilakukan sebelum prasyarat selesai.
- Status transfer harus mencerminkan kondisi sebenarnya.
- Partial failure harus dilaporkan.
- Aplikasi tidak boleh crash akibat satu transfer gagal.

## 9.3 Compatibility

- macOS Apple Silicon dan Intel yang masih didukung runtime.
- Windows 10/11 sesuai dukungan Tauri/WebView.
- Distribusi Linux yang memenuhi runtime dependencies.
- S3-compatible API menggunakan konfigurasi endpoint dan region.

# 10. Local Data Management

Ringkasan entitas di bawah bersifat konseptual. Skema detail, constraint, dan migration menjadi tanggung jawab [Database Design](database_design.md).

## 10.1 Connection Entity

- id
- name
- provider
- endpoint
- region
- access_key_id
- credential_ref
- force_path_style
- allow_insecure_http
- created_at
- updated_at

## 10.2 Bucket Bookmark Entity

- id
- connection_id
- bucket_name
- display_name
- created_at

## 10.3 Transfer Entity

- id
- connection_id (nullable setelah koneksi dihapus)
- connection_name_snapshot
- direction
- bucket
- object_key
- local_path
- bytes_total
- bytes_transferred
- status
- retry_count
- error_code
- error_message
- multipart_upload_id
- created_at
- started_at
- completed_at

## 10.4 Transfer Part Entity

- id
- transfer_id
- part_number
- etag
- size_bytes

## 10.5 Settings Entity

- key
- value
- updated_at

Database harus mempunyai versioned migration untuk perubahan skema aplikasi di masa mendatang.

# 11. Technical Architecture

## 11.1 Frontend

Vue 3 bertanggung jawab untuk:

- Menampilkan UI.
- Mengelola state tampilan.
- Navigasi aplikasi.
- Input pengguna.
- Menerima transfer progress event.
- Memanggil Tauri commands.

Frontend tidak menangani penyimpanan atau penggunaan langsung Secret Access Key.

## 11.2 Rust Backend

Rust bertanggung jawab untuk:

- Credential management.
- Storage connection lifecycle.
- S3 client creation.
- Bucket and object operations.
- Transfer management.
- SQLite persistence.
- Error classification.
- Secure filesystem operations.

## 11.3 Architecture Pattern

Menggunakan pemisahan:

**Tauri Commands → Application Services → Storage Provider Adapter → S3 SDK**

Sedangkan data lokal menggunakan:

**Application Services → Repository → SQLite / OS Credential Store**

## 11.4 Provider Adapter

Operasi dasar provider:

- TestConnection
- ListBuckets
- ListObjects
- HeadObject
- PutObject
- GetObject
- CopyObject
- DeleteObject
- CreateMultipartUpload / UploadPart / CompleteMultipartUpload / AbortMultipartUpload

Provider-specific capabilities tidak boleh diasumsikan identik untuk semua layanan.

# 12. Core User Flows

## 12.1 Add New Storage Connection

1. Pengguna membuka Bucklane.
2. Memilih Add Connection.
3. Memilih provider.
4. Mengisi konfigurasi.
5. Menekan Test Connection.
6. Sistem menampilkan hasil pengujian.
7. Pengguna menyimpan koneksi.
8. Profil muncul pada sidebar.

## 12.2 Browse Storage

1. Pengguna memilih koneksi.
2. Sistem mengakses provider.
3. Bucket yang diizinkan ditampilkan.
4. Pengguna memilih bucket.
5. Sistem memuat objek secara bertahap.
6. Pengguna membuka folder atau memilih file.

## 12.3 Upload File

1. Pengguna membuka bucket tujuan.
2. Memilih Upload.
3. Memilih file lokal.
4. Sistem memvalidasi tujuan dan potensi overwrite.
5. Transfer masuk antrean.
6. Sistem melakukan upload.
7. Progress ditampilkan.
8. Hasil transfer ditampilkan.
9. Explorer diperbarui setelah upload sukses.

## 12.4 Download File

1. Pengguna memilih objek.
2. Memilih Download.
3. Memilih lokasi lokal.
4. Transfer dimulai.
5. Progress ditampilkan.
6. Sistem memvalidasi completion.
7. File tersedia di lokasi tujuan.

# 13. Error Handling

Aplikasi harus dapat membedakan kategori error:

- Invalid credentials
- Access denied
- Bucket not found
- Object not found
- Unsupported operation
- Invalid region/endpoint
- Network unavailable
- TLS/certificate error
- Connection timeout
- Request throttling
- Insufficient local disk space
- File permission error
- Transfer interrupted
- Partial bulk-operation failure

Pesan error harus mudah dipahami pengguna dan menyediakan detail teknis untuk troubleshooting tanpa membocorkan credentials.

# 14. MVP Release Criteria

MVP dinyatakan siap apabila:

- Aplikasi dapat dibuka dan digunakan pada platform target yang diuji.
- Pengguna dapat menyimpan dan mengedit koneksi S3.
- Koneksi Supabase, AWS S3, dan MinIO lulus integration testing.
- Pengguna dapat mengakses bucket dengan izin yang sesuai.
- Pengguna dapat browsing file menggunakan pagination.
- Upload dan download berjalan dengan progress.
- Rename, copy, move, dan delete memiliki pengamanan data.
- Transfer dapat dibatalkan dan dicoba kembali.
- Secret key tidak disimpan dalam plaintext.
- Kegagalan jaringan tidak menyebabkan aplikasi crash.
- Tidak ditemukan bug kritis atau masalah keamanan tingkat tinggi yang belum ditangani.

# 15. Development Roadmap

## M00 — Foundation

- Initialize Tauri + Vue project.
- Configure TypeScript, Tailwind, Pinia.
- Establish UI architecture.
- Rust backend structure.
- Logging and error handling.
- SQLite migration setup.
- CI pipeline.

**Deliverable:** aplikasi dasar dapat dibuka.

## M01 — Storage Connections

- Connection model.
- SQLite repository.
- Credential store.
- Provider presets.
- Add/Edit/Delete connection.
- Test connection.
- Connection sidebar.

**Deliverable:** pengguna dapat menyimpan dan menguji koneksi.

## M02 — Bucket Explorer

- List buckets.
- Bucket bookmarks / open configured bucket.
- List objects.
- Prefix navigation.
- Pagination.
- Breadcrumb.
- Refresh.
- Basic metadata.

**Deliverable:** pengguna dapat menjelajahi object storage.

## M03 — Object Operations

- Upload file.
- Download file.
- Create folder marker.
- Rename object.
- Copy object.
- Move object.
- Delete object.
- Overwrite confirmation.

**Deliverable:** file management dasar berfungsi.

## M04 — Transfer Engine

- Transfer queue.
- Transfer progress.
- Retry.
- Cancel.
- Concurrent transfer.
- Multipart upload.
- Transfer history.

**Deliverable:** pengelolaan transfer stabil.

## M05 — QA & Release

- Integration testing.
- Cross-platform testing.
- Security checks.
- Application packaging.
- Code signing/release setup.
- Documentation.
- Release v1.0.

**Deliverable:** Bucklane MVP v1.0.

# 16. Post-MVP Roadmap

## Version 1.1

- Drag & drop enhancements.
- Image/PDF preview.
- Presigned URL generator.
- Advanced sorting/filtering.
- Object metadata editor.
- Bulk download.
- Transfer speed and ETA.

## Version 1.2

- Transfer between providers.
- Folder upload/download.
- Copy/move folder dan multipart copy untuk objek di atas 5 GiB.
- Session token / temporary credentials.
- Connection grouping.
- Local configuration backup/export.
- Keyboard shortcuts.
- Enhanced provider compatibility.

## Version 2.0

- Local-to-S3 synchronization.
- Scheduled backups.
- Optional resumable transfer across application restarts.
- SFTP/WebDAV support.
- Advanced storage analytics.

# 17. Success Metrics

## Product Quality

- 100% completion of P0 acceptance criteria.
- No unresolved critical security vulnerabilities at release.
- No known reproducible data-loss bugs at release.
- All required provider smoke tests pass.

## Usability

- First connection can be created without command-line interaction.
- Users can complete basic upload/download from the graphical interface.
- Errors provide actionable troubleshooting information.

## Performance

- App remains responsive during transfer.
- Large file uploads do not require loading the complete file into RAM.
- Pagination prevents uncontrolled listing of large buckets.

# 18. Open Decisions

| Decision                   | Proposed Direction                  |
| -------------------------- | ----------------------------------- |
| Product name               | Bucklane (working name)             |
| License                    | MIT or Apache-2.0                   |
| First development platform | macOS                               |
| MVP providers              | AWS, Supabase, MinIO                |
| UI layout                  | Sidebar + Explorer + Transfer Panel |
| Backend framework          | Tauri/Rust                          |
| Cloud backend              | None                                |
| Account registration       | None                                |
| Credential storage         | OS Keychain                         |
| Configuration DB           | SQLite                              |
| Public repository          | To be decided                       |

# 19. Definition of Done

Sebuah requirement dinyatakan selesai apabila:

1. Implementasinya memenuhi acceptance criteria.
2. Validasi input dan error handling tersedia.
3. Security requirements yang relevan terpenuhi.
4. Automated test ditambahkan untuk bagian yang kritis.
5. Telah melewati review.
6. Tidak menimbulkan regression pada operasi storage lainnya.
7. Dokumentasi diperbarui jika terdapat perubahan perilaku.
8. Telah diuji dengan provider yang sesuai.

---

**End of PRD v1.0 — Bucklane Desktop S3 Storage Manager**
