/**
 * Development-only demo mode for screenshots (`pnpm screenshots`).
 *
 * Enabled with `?demo` on the dev server. It replaces Tauri IPC with canned,
 * realistic data so every page can be rendered in a plain browser. This
 * module is only imported behind `import.meta.env.DEV`, so it never ships in
 * production builds.
 *
 * Query parameters:
 * - `theme=light|dark`: theme returned by settings (default: dark)
 * - `select=<object key>`: object to open in the explorer details panel
 */
import type { App } from "vue";
import type { Router } from "vue-router";

const MINUTE = 60_000;
const ago = (minutes: number) => new Date(Date.now() - minutes * MINUTE).toISOString();

const params = new URLSearchParams(location.search);

const profiles = (
  [
    ["aws", "Amazon S3"],
    ["supabase", "Supabase Storage"],
    ["minio", "MinIO"],
    ["cloudflare_r2", "Cloudflare R2"],
    ["wasabi", "Wasabi"],
    ["digitalocean_spaces", "DigitalOcean Spaces"],
    ["custom", "Custom S3-compatible"],
  ] as const
).map(([provider, label]) => ({
  provider,
  label,
  endpointRequired:
    provider !== "aws" && provider !== "wasabi" && provider !== "digitalocean_spaces",
  endpointPlaceholder: null,
  defaultRegion: null,
  regionPlaceholder: "us-east-1",
  defaultForcePathStyle: provider !== "aws",
}));

const connections = [
  {
    id: "c-aws",
    name: "Production assets",
    provider: "aws",
    endpoint: null,
    region: "ap-southeast-1",
    accessKeyId: "AKIA4T7XQ2MPLRV3",
    forcePathStyle: false,
    allowInsecureHttp: false,
    createdAt: ago(60 * 24 * 40),
    updatedAt: ago(60 * 24 * 3),
  },
  {
    id: "c-supabase",
    name: "Supabase · app uploads",
    provider: "supabase",
    endpoint: "https://vqzkfjrm.supabase.co/storage/v1/s3",
    region: "ap-southeast-1",
    accessKeyId: "8f3c1e0a52d4",
    forcePathStyle: true,
    allowInsecureHttp: false,
    createdAt: ago(60 * 24 * 21),
    updatedAt: ago(60 * 24 * 21),
  },
  {
    id: "c-r2",
    name: "Website CDN",
    provider: "cloudflare_r2",
    endpoint: "https://4b8e1f0c9a.r2.cloudflarestorage.com",
    region: "auto",
    accessKeyId: "e91a07c4d3",
    forcePathStyle: true,
    allowInsecureHttp: false,
    createdAt: ago(60 * 24 * 12),
    updatedAt: ago(60 * 24 * 12),
  },
  {
    id: "c-minio",
    name: "Local MinIO",
    provider: "minio",
    endpoint: "http://localhost:9000",
    region: "us-east-1",
    accessKeyId: "minio-dev",
    forcePathStyle: true,
    allowInsecureHttp: true,
    createdAt: ago(60 * 24 * 5),
    updatedAt: ago(60 * 24 * 5),
  },
];

const buckets = [
  { name: "backups-cold", createdAt: ago(60 * 24 * 400), region: "eu-central-1" },
  { name: "invoices-2026", createdAt: ago(60 * 24 * 280), region: "ap-southeast-1" },
  { name: "media-originals", createdAt: ago(60 * 24 * 500), region: "ap-southeast-1" },
  { name: "static-site", createdAt: ago(60 * 24 * 90), region: "us-east-1" },
  { name: "user-avatars", createdAt: ago(60 * 24 * 60), region: "ap-southeast-1" },
];

const bookmarks: Record<string, object[]> = {
  "c-aws": [
    {
      id: "b1",
      connectionId: "c-aws",
      bucketName: "media-originals",
      displayName: "Media",
      createdAt: ago(9000),
    },
    {
      id: "b2",
      connectionId: "c-aws",
      bucketName: "invoices-2026",
      displayName: null,
      createdAt: ago(8000),
    },
  ],
  "c-r2": [
    {
      id: "b3",
      connectionId: "c-r2",
      bucketName: "site-assets",
      displayName: null,
      createdAt: ago(7000),
    },
  ],
};

type Entry = [name: string, size: number | null, minutesAgo: number];
const folders: Record<string, Entry[]> = {
  "": [
    ["brand/", null, 0],
    ["campaigns/", null, 0],
    ["marketing/", null, 0],
    ["product-shots/", null, 0],
    ["README.txt", 2_148, 60 * 24 * 30],
  ],
  "marketing/": [
    ["exports/", null, 0],
    ["reports/", null, 0],
    ["thumbnails/", null, 0],
    ["Q3-board-deck.pdf", 4_823_114, 20],
    ["hero-banner@2x.png", 1_288_412, 140],
    ["customer-export-2026-10.csv", 73_821, 410],
    ["launch-video-final.mp4", 184_220_341, 980],
    ["press-kit-2026.zip", 926_114_720, 1_730],
    ["release-notes.md", 4_210, 2_880],
    ["brand-guidelines-v4.pdf", 12_904_337, 4_320],
    ["social-schedule.xlsx", 58_204, 6_010],
  ],
};

function listObjects(prefix: string) {
  // A prefix that does not end in "/" is a name search within its folder.
  const folder = prefix.slice(0, prefix.lastIndexOf("/") + 1);
  const search = prefix.slice(folder.length);
  const entries = (folders[folder] ?? []).filter(([name]) => name.startsWith(search));
  return {
    objects: entries.map(([name, size, minutes]) => ({
      key: folder + name,
      name,
      size,
      lastModified: size === null ? null : ago(minutes),
      etag: null,
      storageClass: null,
      isPrefix: name.endsWith("/"),
    })),
    nextContinuationToken: null,
    isTruncated: false,
  };
}

function transfer(
  id: string,
  direction: "UPLOAD" | "DOWNLOAD" | "COPY",
  key: string,
  bucket: string,
  total: number,
  done: number,
  status: string,
  minutes: number,
  error: string | null = null,
) {
  return {
    id,
    connectionId: "c-aws",
    connectionNameSnapshot: "Production assets",
    direction,
    bucket,
    objectKey: key,
    localFileName: direction === "COPY" ? "" : key.split("/").pop(),
    source: null as Record<string, string | boolean> | null,
    bytesTotal: total,
    bytesTransferred: done,
    status,
    retryCount: 0,
    error: error
      ? { code: "ENDPOINT_UNREACHABLE", message: error, details: null, retryable: true }
      : null,
    // An interrupted download keeps its partial file and can resume.
    resumableBytes: direction === "DOWNLOAD" && status === "FAILED" && done > 0 ? done : null,
    paused: false,
    sourceKept: false,
    createdAt: ago(minutes),
    startedAt: ago(minutes),
    completedAt: status === "RUNNING" || status === "QUEUED" ? null : ago(minutes - 1),
  };
}

const transfers = [
  {
    ...transfer(
      "t0",
      "COPY",
      "brand/brand-guidelines-v4.pdf",
      "site-assets",
      12_582_912,
      12_582_912,
      "COMPLETED",
      3,
    ),
    connectionId: "c-r2",
    connectionNameSnapshot: "Website CDN",
    source: {
      connectionId: "c-aws",
      connectionNameSnapshot: "Production assets",
      bucket: "media-originals",
      key: "marketing/brand-guidelines-v4.pdf",
      deleteAfterCopy: false,
    },
  },
  transfer(
    "t1",
    "UPLOAD",
    "marketing/launch-video-final.mp4",
    "media-originals",
    184_220_341,
    121_734_000,
    "RUNNING",
    1,
  ),
  transfer(
    "t2",
    "UPLOAD",
    "marketing/press-kit-2026.zip",
    "media-originals",
    926_114_720,
    0,
    "QUEUED",
    1,
  ),
  transfer(
    "t3",
    "DOWNLOAD",
    "2026/10/INV-20418.pdf",
    "invoices-2026",
    238_114,
    238_114,
    "COMPLETED",
    6,
  ),
  transfer(
    "t4",
    "UPLOAD",
    "marketing/Q3-board-deck.pdf",
    "media-originals",
    4_823_114,
    4_823_114,
    "COMPLETED",
    21,
  ),
  {
    ...transfer(
      "t-paused",
      "DOWNLOAD",
      "media-originals/archive/raw-footage-day2.zip",
      "media-originals",
      2_147_483_648,
      912_261_120,
      "FAILED",
      12,
      "Paused.",
    ),
    paused: true,
  },
  transfer(
    "t-resume",
    "DOWNLOAD",
    "media-originals/marketing/launch-video-final-4k.mov",
    "media-originals",
    4_831_838_208,
    3_221_225_472,
    "FAILED",
    30,
    "Bucklane was closed before this download finished. Resume to continue where it stopped.",
  ),
  transfer(
    "t5",
    "UPLOAD",
    "dumps/db-2026-10-14.sql.gz",
    "backups-cold",
    52_114_003,
    8_421_000,
    "FAILED",
    48,
    "Could not reach the storage endpoint.",
  ),
  transfer(
    "t6",
    "UPLOAD",
    "marketing/hero-banner@2x.png",
    "media-originals",
    1_288_412,
    1_288_412,
    "COMPLETED",
    141,
  ),
  transfer(
    "t7",
    "DOWNLOAD",
    "exports/customer-export-2026-10.csv",
    "media-originals",
    73_821,
    73_821,
    "COMPLETED",
    205,
  ),
];

const theme = params.get("theme") === "light" ? "light" : "dark";

function respond(cmd: string, args: Record<string, unknown> = {}): unknown {
  switch (cmd) {
    case "health":
      return { appVersion: "0.5.0", databaseOk: true, databaseError: null };
    case "list_connections":
      return connections;
    case "list_provider_profiles":
      return profiles;
    case "get_settings":
      return {
        theme,
        maxConcurrentTransfers: 3,
        multipartThresholdMb: 64,
        multipartPartSizeMb: 16,
        explorerPageSize: 100,
        checkForUpdates: true,
        resumeDownloadsAutomatically: false,
      };
    case "pause_transfer":
    case "cancel_transfer":
      return null;
    case "list_transfers":
      return transfers;
    case "list_buckets":
      return buckets;
    case "list_bucket_bookmarks":
      return bookmarks[args.connectionId as string] ?? [];
    case "list_objects": {
      const request = args.request as { prefix?: string | null };
      return listObjects(request.prefix ?? "");
    }
    case "prepare_prefix_relocate": {
      const input = args.input as { sourcePrefix: string; destinationPrefix: string; mode: string };
      return {
        planId: "demo-relocate",
        mode: input.mode,
        sourcePrefix: input.sourcePrefix,
        destinationPrefix: input.destinationPrefix,
        objectCount: 128,
        totalBytes: 734_003_200,
        conflictCount: 2,
        conflictSamples: [
          `${input.destinationPrefix}cover.png`,
          `${input.destinationPrefix}notes.md`,
        ],
        expiresAt: new Date(Date.now() + 600_000).toISOString(),
      };
    }
    case "execute_prefix_relocate":
      return { completed: 126, skipped: 2, canceled: 0, failed: [], sourcesKept: [] };
    case "cancel_prefix_relocate":
      return null;
    case "diagnostics":
      return {
        appVersion: "0.5.0",
        tauriVersion: "2.12.0",
        webviewVersion: "26.0",
        os: "macos",
        arch: "aarch64",
        databaseOk: true,
      };
    case "log_frontend_error":
    case "open_logs_folder":
    case "open_support_page":
      return null;
    case "check_for_update":
      return {
        version: "0.6.0",
        currentVersion: "0.5.0",
        notes: "### Added\n\n- **Copy to another bucket or provider.**\n- Public URLs.",
      };
    case "install_update":
    case "open_release_page":
    case "restart_app":
      return null;
    case "head_object":
      return {
        key: args.key,
        size: 4_823_114,
        lastModified: ago(20),
        contentType: "application/pdf",
        cacheControl: "public, max-age=3600",
        contentDisposition: null,
        contentEncoding: null,
        contentLanguage: null,
        etag: '"9b2cf535f27731c974343645a3985328"',
        storageClass: "STANDARD",
        metadata: { "uploaded-by": "dewi.anggraini", department: "marketing" },
      };
    case "update_object_metadata": {
      const input = args.input as { key: string; etag: string; headers: Record<string, unknown> };
      return {
        key: input.key,
        size: 4_823_114,
        lastModified: new Date().toISOString(),
        etag: input.etag,
        storageClass: "STANDARD",
        ...input.headers,
      };
    }
    case "get_public_url":
      return {
        url: `https://${String(args.bucket)}.s3.ap-southeast-1.amazonaws.com/${String(args.key)}`,
        baseUrl: null,
      };
    case "enqueue_copy": {
      const input = args.input as { keys: string[]; destinationPrefix: string };
      return input.keys.map((key, i) => ({
        kind: "queued",
        transferId: `copy-${i}`,
        key: `${input.destinationPrefix}${key.split("/").filter(Boolean).pop()}`,
      }));
    }
    case "set_bucket_public_url":
      return (args.input as { baseUrl: string | null }).baseUrl;
    case "create_share_link": {
      const input = args.input as { bucket: string; key: string; expiresInSeconds: number };
      return {
        url: `https://${input.bucket}.s3.ap-southeast-1.amazonaws.com/${input.key}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires=${input.expiresInSeconds}&X-Amz-Signature=3f9a1c…`,
        expiresAt: new Date(Date.now() + input.expiresInSeconds * 1000).toISOString(),
      };
    }
    case "preview_object": {
      const key = String(args.key);
      const encode = (text: string) => btoa(unescape(encodeURIComponent(text)));
      if (/\.(png|jpe?g|webp|svg)$/i.test(key)) {
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#0e7490"/><stop offset="1" stop-color="#0f172a"/></linearGradient></defs><rect width="1200" height="630" fill="url(#g)"/><text x="80" y="300" font-family="Helvetica,Arial" font-size="72" font-weight="700" fill="#fff">Launch week 2026</text><text x="80" y="380" font-family="Helvetica,Arial" font-size="32" fill="#a5f3fc">Faster uploads, shareable links, folder transfers</text></svg>`;
        return {
          kind: "IMAGE",
          contentType: "image/svg+xml",
          dataBase64: encode(svg),
          truncated: false,
          size: 1_288_412,
        };
      }
      if (/\.(md|txt|csv|json)$/i.test(key)) {
        const body = key.endsWith(".json")
          ? JSON.stringify({
              release: "0.2.0",
              date: "2026-10-15",
              highlights: ["UI refresh", "Overview dashboard", "Share links"],
            })
          : "# Release notes\n\n## 0.2.0\n\n- Refreshed interface with light and dark themes\n- Overview dashboard with pinned buckets\n- Share links for any file\n- Upload and download whole folders\n";
        return {
          kind: "TEXT",
          contentType: "text/markdown",
          dataBase64: encode(body),
          truncated: false,
          size: 4_210,
        };
      }
      throw {
        code: "OPERATION_UNSUPPORTED",
        message: "This type of file can't be previewed. Download it to open it.",
        details: null,
        retryable: false,
      };
    }
    default:
      throw {
        code: "OPERATION_UNSUPPORTED",
        message: `Not available in demo mode: ${cmd}`,
        details: null,
        retryable: false,
      };
  }
}

/** Replaces Tauri IPC; must run before the app mounts. */
export function install() {
  // Screenshots should not show an empty macOS title-bar area.
  document.documentElement.classList.remove("platform-mac");
  let nextCallback = 1;
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd.startsWith("plugin:event|")) return nextCallback++;
      return structuredClone(respond(cmd, args));
    },
    transformCallback: () => nextCallback++,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  };
}

/** Optional per-screenshot actions once routing and stores are ready. */
export async function afterMount(app: App) {
  const select = params.get("select");
  if (!select) return;
  const router = app.config.globalProperties.$router as Router;
  await router.isReady();
  // Let the explorer load its listing before selecting.
  await new Promise((resolve) => setTimeout(resolve, 300));
  const pinia = app.config.globalProperties.$pinia as unknown as {
    _s: Map<string, { select?: (key: string) => Promise<void> }>;
  };
  await pinia._s.get("explorer")?.select?.(select);
}
