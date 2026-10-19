# Product Requirements Document (PRD)

## Bucklane — Desktop S3 Storage Manager

**Version:** 1.0  
**Status:** Original plan (not updated for every change; the code and README are authoritative)  
**Platform:** macOS, Windows, Linux  
**Product Type:** Cross-platform Desktop Application  
**Architecture:** Local-first, without centralized backend  
**Technology:** Tauri 2, Vue 3, TypeScript, Rust, SQLite  
**Initial Release:** MVP v1.0

---

# 1. Executive Summary

Bucklane is a cross-platform desktop application for managing object storage based on the Amazon S3 API and various S3-compatible storage providers through a single modern interface.

Bucklane is designed as an alternative to applications such as Cyberduck, focusing on easy connection management, bucket exploration, file management, data transfer, credential security, and multi-provider support with no limit on the number of connections.

The application runs locally and accesses object storage directly, without requiring a centralized cloud backend or a Bucklane account.

## 1.1 Product Vision

**One application to manage all your S3-compatible storage.**

To be a lightweight, secure, and easy-to-use desktop application for managing multiple object storage services from one place.

## 1.2 Problem Statement

Object storage users often face several problems:

1. They need several tools to access different storage providers.
2. Some applications limit connections or features based on licensing.
3. Managing files through a CLI is not always practical for day-to-day work.
4. Object storage management interfaces are often too technical.
5. Transferring large files requires progress monitoring and failure handling.
6. Managing credentials from many providers becomes impractical.
7. S3 features and compatibility differ between providers.

Bucklane aims to solve these problems through a consistent desktop interface with an integrated connection and file management system.

# 2. Product Goals

## 2.1 Primary Goals

- Provide a desktop application for managing S3-compatible object storage.
- Support saving connections without an application-imposed limit on their number.
- Provide an intuitive file explorer.
- Support direct file upload and download.
- Provide a transfer manager with progress and error handling.
- Protect the user's access key and secret key.
- Provide a consistent experience on macOS, Windows, and Linux.

## 2.2 Secondary Goals

- Be an open-source application that the community can develop.
- Support new S3 providers with minimal configuration.
- Deliver a lightweight and responsive user experience.
- Provide a development foundation for file synchronization and cross-storage transfer.

## 2.3 Non-Goals MVP

The MVP does not include:

- Two-way cloud synchronization.
- Multi-user collaboration.
- A SaaS account system.
- Storing files through a Bucklane server.
- Advanced IAM policy management.
- Lifecycle and versioning management.
- Object replication.
- Direct transfer between providers.
- SFTP, FTP, or WebDAV connections.
- Credentials other than static access keys (session token/STS, AWS profiles, SSO).
- Recursive copy/move of folders (prefixes).
- Copy/move of objects larger than 5 GiB.

# 3. Target Users

## 3.1 Software Developers

Developers who use object storage to store documents, images, videos, backups, or application files.

**Needs:** fast access to files, upload/download, object path debugging, and transfer monitoring.

## 3.2 DevOps & System Administrators

Users who manage multiple object storages for development, staging, and production.

**Needs:** multi-connection, bucket navigation, bulk operations, connection security, and handling of large files.

## 3.3 Technical Teams

Technical teams that need a visual interface without always having to use the AWS CLI, rclone, or other terminal tools.

**Needs:** easy navigation and standard file operations.

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

**Rules:**

- Provider presets use default configuration appropriate to each provider.
- A custom endpoint can be configured manually.
- Region and path-style addressing are configurable.
- Not every provider supports all AWS S3 operations.
- Unsupported features must be disabled or show appropriate information.
- Compatibility must be tested per provider, not assumed from S3 API support.

# 5. Core Functional Requirements

## 5.1 Connection Management

**Module ID:** CON

### CON-001 — Create Connection

**Priority:** P0

Users can add a new object storage connection.

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

- Required fields must be validated.
- Users can choose a provider preset or a custom endpoint.
- The secret key is stored using the operating system's secure credential storage.
- The system does not store the secret key in a plaintext database.
- The system shows clear validation messages.

### CON-002 — Test Connection

**Priority:** P0

Users can test the configuration before or after saving the connection.

**Acceptance Criteria:**

- The system makes an authentication/access request to the endpoint.
- A success status is shown clearly.
- Authentication, endpoint, permission, and network errors are handled separately when they can be identified.
- A connection without permission to list all buckets is not automatically deemed invalid; the test can use a specific bucket if one is configured.

### CON-003 — Saved Connections

**Priority:** P0

Users can see all saved connections.

**Acceptance Criteria:**

- There is no deliberate limit on the number of connections.
- Connections remain available after the application is closed.
- Users can select and open a connection.
- The connection list can be searched by name.

### CON-004 — Edit & Delete Connection

**Priority:** P0

Users can update or delete a connection's configuration.

**Acceptance Criteria:**

- Configuration changes can be saved.
- Deletion requires confirmation.
- Deleting a connection does not delete buckets or objects at the storage provider.
- The related credentials are removed from the credential store when the connection is deleted. Each connection has its own credential reference (1:1 relation); credentials are not shared between connections.
- Transfer history of a deleted connection is retained using a snapshot of the connection name.

## 5.2 Bucket Management

**Module ID:** BKT

### BKT-001 — List Buckets

**Priority:** P0

The system displays the buckets the user can access.

**Acceptance Criteria:**

- The bucket list is shown if the credentials have the required permissions.
- A bucket name can be selected to open the file explorer.
- Loading and error states are available.
- If bucket listing is not allowed, the user can open a specific, manually configured bucket (see BKT-005).

### BKT-005 — Bucket Bookmarks

**Priority:** P0

Users can manually save bucket names per connection so they can be opened directly without the ListBuckets permission.

**Acceptance Criteria:**

- Users can add, label, and delete bucket bookmarks.
- The same bucket name cannot be bookmarked twice on the same connection.
- A bookmark is not treated as proof that the bucket exists or is accessible; access is verified when it is opened.
- Bookmarks are deleted along with the connection.

### BKT-002 — Bucket Navigation

**Priority:** P0

Users can open a bucket and navigate the object list.

**Acceptance Criteria:**

- Shows prefixes/folders and objects.
- Supports pagination.
- Supports breadcrumb navigation.
- Supports refresh.
- Does not download the entire bucket contents when first opened.

### BKT-003 — Create Bucket

**Priority:** P2 — Post-MVP

Bucket creation will be added once compatibility and basic permission handling are stable.

### BKT-004 — Delete Bucket

**Priority:** P2 — Post-MVP

Bucket deletion is excluded from the MVP because it is high-risk and its rules differ between providers.

## 5.3 File Explorer

**Module ID:** EXP

### EXP-001 — Browse Objects

**Priority:** P0

Users can view the objects in a bucket.

Information displayed:

- File name/object key
- File type
- File size
- Last modified
- Folder/prefix

### EXP-002 — Folder Navigation

**Priority:** P0

Users can navigate the virtual folder structure based on object prefixes.

**Acceptance Criteria:**

- The breadcrumb can be used.
- Folders can be opened.
- Back navigation is available.
- Empty folders represented by a marker can be displayed.

### EXP-003 — Create Folder

**Priority:** P0

Users can create virtual folders.

**Acceptance Criteria:**

- The system creates a folder marker object.
- The folder name is validated.
- The new prefix can be navigated.
- A folder marker is not treated as a real filesystem directory.

### EXP-004 — Object Search

**Priority:** P1

Search for objects by name or prefix.

**MVP Rules:**

- Filtering over objects already loaded.
- Server-side prefix search where supported.
- No promise of full substring search across the entire bucket contents.

### EXP-005 — Object Metadata

**Priority:** P1

Displays basic metadata such as key, size, last modified, content type, and ETag when available.

## 5.4 File Operations

**Module ID:** FIL

### FIL-001 — Upload File

**Priority:** P0

Users can upload one or more local files to a bucket.

**Acceptance Criteria:**

- Users can select files.
- The destination bucket and prefix can be selected.
- Transfer progress is shown.
- Upload failures are reported.
- If the destination key already exists, the user chooses Overwrite, Skip, or Rename (`name (1).ext`), with an "apply to all" option. Default: ask the user; no overwrite without an explicit choice.
- The UI stays responsive during transfer.

### FIL-002 — Download File

**Priority:** P0

Users can download objects to the local filesystem.

**Acceptance Criteria:**

- The user chooses the save location.
- Transfer progress is shown.
- Existing local files are not silently overwritten; the user chooses Overwrite, Skip, or Rename as with upload.
- Download failures are handled safely.
- Partial files are not treated as successful downloads.

### FIL-003 — Rename Object

**Priority:** P0

Users can rename an object.

**Technical Rules:**

Rename is done by copying the object to the new key, then deleting the old key after the copy is validated.

**Acceptance Criteria:**

- The system does not delete the source object before the copy succeeds.
- A failed copy does not cause the source object to be deleted.
- Errors can be shown and recovered manually if needed.

### FIL-004 — Copy Object

**Priority:** P0

Users can copy objects within the same storage.

**MVP Limitation:** single objects only; objects larger than 5 GiB are rejected with a clear message.

### FIL-005 — Move Object

**Priority:** P0

Users can move objects to another prefix using a copy-then-delete process.

**MVP Limitation:** same as FIL-004.

### FIL-006 — Delete Object

**Priority:** P0

Users can delete one or more objects.

**Acceptance Criteria:**

- Deletion requires confirmation.
- The bucket name and number of objects are shown.
- Partial failures are reported.
- The system does not claim the whole operation succeeded if part of it failed.

**Delete folder (prefix):**

- The system first counts all objects under the prefix, then shows the object count and total size before confirmation.
- Only objects counted at confirmation time are deleted; new objects that appear afterwards are not deleted.
- Deleting the bucket root (empty prefix) is not allowed.
- Folders with more than 100,000 objects cannot be deleted in the MVP.

### FIL-007 — Bulk Selection

**Priority:** P1

Supports selecting multiple objects for download and delete.

### FIL-008 — Drag & Drop Upload

**Priority:** P1

Users can drag files from the filesystem into the application window to upload them.

# 6. Transfer Management

**Module ID:** TRF

## TRF-001 — Transfer Queue

**Priority:** P0

Every upload/download is added to the transfer queue.

Status:

- QUEUED
- RUNNING
- COMPLETED
- FAILED
- CANCELED

## TRF-002 — Transfer Progress

**Priority:** P0

Displays:

- File name
- Transfer direction
- Total bytes
- Transferred bytes
- Percentage
- Current status
- Error message if a failure occurs

Transfer speed and ETA can be added when they can be measured accurately.

## TRF-003 — Retry Transfer

**Priority:** P0

Users can rerun a failed transfer.

## TRF-004 — Cancel Transfer

**Priority:** P0

Users can cancel an active transfer.

Cancellation must stop cancelable tasks and safely handle temporary files or multipart uploads.

## TRF-005 — Multipart Upload

**Priority:** P0

Large files use multipart upload when supported by the provider and within the defined size limits.

**Acceptance Criteria:**

- File parts can be uploaded incrementally.
- Total progress can be calculated.
- The multipart upload is completed after all parts succeed.
- Failed multipart uploads can be cleaned up or marked for recovery.
- Resume after an application restart is not promised in the MVP.

## TRF-006 — Concurrent Transfer

**Priority:** P0

The application supports multiple simultaneous active transfers with a configurable concurrency limit.

# 7. Desktop User Experience

## 7.1 Main Layout

The application uses three main areas:

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

- Light mode and dark mode.
- Responsive desktop layout.
- Loading indicator.
- Empty state.
- Error notifications.
- Confirmation dialogs.
- Basic keyboard navigation.
- Native file picker.
- Clear progress information.
- UI is not blocked by network operations.

# 8. Security Requirements

**SEC-001 — Credential Protection**

The Secret Access Key must be stored using the OS's secure credential storage.

**SEC-002 — No Plaintext Secret Database**

SQLite stores only the credential reference and non-secret metadata.

**SEC-003 — Secure Communication**

HTTPS is the default. Using HTTP for a custom endpoint must require explicit configuration and a security warning.

**SEC-004 — Sensitive Logging**

The application must not log secrets, authorization headers, tokens, or full signed URLs.

**SEC-005 — Destructive Operations**

Delete and overwrite must be protected by appropriate confirmation.

**SEC-006 — Permission Handling**

The application must keep working for read-only accounts, to the extent that the required operations are permitted.

**SEC-007 — Local-First**

Credentials and files are not sent to any Bucklane-owned backend.

**SEC-008 — Desktop Permissions**

Filesystem, network, and Tauri command access must be restricted following the principle of least privilege.

# 9. Non-Functional Requirements

## 9.1 Performance Targets

Initial targets to be verified through testing, not proven performance claims:

| Metric               | MVP Target                                                   |
| -------------------- | ------------------------------------------------------------ |
| UI startup           | Under 3 seconds on the benchmark device                      |
| Explorer navigation  | Does not block the UI during requests                        |
| File listing         | Pagination, maximum 1,000 objects per request if API allows  |
| UI responsiveness    | Stays interactive while transfers run                        |
| Connection storage   | No application-level connection limit                        |
| Transfer concurrency | Default 3, configurable                                      |
| Memory utilization   | Profiled at idle and during large transfers                  |
| Large file handling  | Streaming, not full-file buffering                           |

## 9.2 Reliability

- Network timeouts must be handled.
- Temporary errors can be retried with backoff.
- Destructive operations must not run before their prerequisites complete.
- Transfer status must reflect the actual state.
- Partial failures must be reported.
- The application must not crash because a single transfer fails.

## 9.3 Compatibility

- macOS on Apple Silicon and Intel still supported by the runtime.
- Windows 10/11 as supported by Tauri/WebView.
- Linux distributions that meet the runtime dependencies.
- S3-compatible APIs using endpoint and region configuration.

# 10. Local Data Management

The entity summary below is conceptual. The detailed schema, constraints, and migrations are the responsibility of [Database Design](database_design.md).

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
- connection_id (nullable after the connection is deleted)
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

The database must have versioned migrations for future application schema changes.

# 11. Technical Architecture

## 11.1 Frontend

Vue 3 is responsible for:

- Rendering the UI.
- Managing view state.
- Application navigation.
- User input.
- Receiving transfer progress events.
- Calling Tauri commands.

The frontend does not handle storage or direct use of the Secret Access Key.

## 11.2 Rust Backend

Rust is responsible for:

- Credential management.
- Storage connection lifecycle.
- S3 client creation.
- Bucket and object operations.
- Transfer management.
- SQLite persistence.
- Error classification.
- Secure filesystem operations.

## 11.3 Architecture Pattern

Uses the following separation:

**Tauri Commands → Application Services → Storage Provider Adapter → S3 SDK**

While local data uses:

**Application Services → Repository → SQLite / OS Credential Store**

## 11.4 Provider Adapter

Basic provider operations:

- TestConnection
- ListBuckets
- ListObjects
- HeadObject
- PutObject
- GetObject
- CopyObject
- DeleteObject
- CreateMultipartUpload / UploadPart / CompleteMultipartUpload / AbortMultipartUpload

Provider-specific capabilities must not be assumed to be identical across all services.

# 12. Core User Flows

## 12.1 Add New Storage Connection

1. The user opens Bucklane.
2. Selects Add Connection.
3. Selects a provider.
4. Fills in the configuration.
5. Clicks Test Connection.
6. The system shows the test result.
7. The user saves the connection.
8. The profile appears in the sidebar.

## 12.2 Browse Storage

1. The user selects a connection.
2. The system accesses the provider.
3. Permitted buckets are displayed.
4. The user selects a bucket.
5. The system loads objects incrementally.
6. The user opens a folder or selects a file.

## 12.3 Upload File

1. The user opens the destination bucket.
2. Selects Upload.
3. Selects local files.
4. The system validates the destination and potential overwrites.
5. The transfer enters the queue.
6. The system performs the upload.
7. Progress is shown.
8. The transfer result is shown.
9. The explorer refreshes after a successful upload.

## 12.4 Download File

1. The user selects an object.
2. Selects Download.
3. Selects a local location.
4. The transfer starts.
5. Progress is shown.
6. The system validates completion.
7. The file is available at the destination.

# 13. Error Handling

The application must be able to distinguish error categories:

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

Error messages must be easy for users to understand and provide technical details for troubleshooting without leaking credentials.

# 14. MVP Release Criteria

The MVP is considered ready when:

- The application can be opened and used on the tested target platforms.
- Users can save and edit S3 connections.
- Supabase, AWS S3, and MinIO connections pass integration testing.
- Users can access buckets with the appropriate permissions.
- Users can browse files using pagination.
- Upload and download work with progress.
- Rename, copy, move, and delete have data safeguards.
- Transfers can be canceled and retried.
- The secret key is not stored in plaintext.
- Network failures do not crash the application.
- No unresolved critical bugs or high-severity security issues are found.

# 15. Development Roadmap

## M00 — Foundation

- Initialize Tauri + Vue project.
- Configure TypeScript, Tailwind, Pinia.
- Establish UI architecture.
- Rust backend structure.
- Logging and error handling.
- SQLite migration setup.
- CI pipeline.

**Deliverable:** the basic application can be opened.

## M01 — Storage Connections

- Connection model.
- SQLite repository.
- Credential store.
- Provider presets.
- Add/Edit/Delete connection.
- Test connection.
- Connection sidebar.

**Deliverable:** users can save and test connections.

## M02 — Bucket Explorer

- List buckets.
- Bucket bookmarks / open configured bucket.
- List objects.
- Prefix navigation.
- Pagination.
- Breadcrumb.
- Refresh.
- Basic metadata.

**Deliverable:** users can explore object storage.

## M03 — Object Operations

- Upload file.
- Download file.
- Create folder marker.
- Rename object.
- Copy object.
- Move object.
- Delete object.
- Overwrite confirmation.

**Deliverable:** basic file management works.

## M04 — Transfer Engine

- Transfer queue.
- Transfer progress.
- Retry.
- Cancel.
- Concurrent transfer.
- Multipart upload.
- Transfer history.

**Deliverable:** stable transfer management.

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
- Copy/move folder and multipart copy for objects larger than 5 GiB.
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

A requirement is considered done when:

1. Its implementation meets the acceptance criteria.
2. Input validation and error handling are in place.
3. The relevant security requirements are met.
4. Automated tests are added for critical parts.
5. It has passed review.
6. It causes no regression in other storage operations.
7. Documentation is updated if behavior changes.
8. It has been tested with the relevant providers.

---

**End of PRD v1.0 — Bucklane Desktop S3 Storage Manager**
