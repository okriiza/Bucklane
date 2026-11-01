const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** Human-readable size using binary multiples (1 KB = 1024 B). */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;

  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value < 10 ? 1 : 0;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

/** Localized date and time, or an em dash when unknown or unparseable. */
export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "—" : dateTimeFormat.format(date);
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "long" });

/** A calendar date like `2026-11-01`, localized; independent of time zone. */
export function formatDate(day: string): string {
  const date = new Date(`${day}T12:00:00`);
  return Number.isNaN(date.getTime()) ? day : dateFormat.format(date);
}

const relativeFormat = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const RELATIVE_STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["second", 60],
  ["minute", 60],
  ["hour", 24],
  ["day", 7],
  ["week", 4.35],
  ["month", 12],
  ["year", Infinity],
];

/** "3 minutes ago", "yesterday"; falls back to a dash for invalid input. */
export function formatRelative(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return "—";
  const time = new Date(iso).getTime();
  if (Number.isNaN(time)) return "—";
  let value = (time - now) / 1000;
  for (const [unit, size] of RELATIVE_STEPS) {
    if (Math.abs(value) < size) return relativeFormat.format(Math.round(value), unit);
    value /= size;
  }
  return "—";
}

/** Host of an endpoint URL, or the AWS regional host when none is set. */
export function endpointHost(endpoint: string | null, region: string): string {
  if (!endpoint) return `s3.${region}.amazonaws.com`;
  try {
    return new URL(endpoint).host;
  } catch {
    return endpoint;
  }
}

/** "12.4 MB/s"; a dash when unknown. */
export function formatSpeed(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null || !Number.isFinite(bytesPerSecond)) return "—";
  return `${formatBytes(Math.round(bytesPerSecond))}/s`;
}

/** Compact remaining time: "45 s", "3 min", "1 h 5 min". */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const s = Math.ceil(seconds);
  if (s < 60) return `${s} s`;
  const minutes = Math.ceil(s / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}
