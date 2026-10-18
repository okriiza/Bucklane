/**
 * Copies text to the clipboard. Uses the async Clipboard API, falling back to
 * a temporary textarea for webviews that restrict it.
 */
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // Fall through to the legacy path.
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  if (!ok) throw new Error("Copy to clipboard failed.");
}

/** `s3://bucket/key`, the form most S3 tools accept. */
export function s3Uri(bucket: string, key: string): string {
  return `s3://${bucket}/${key}`;
}
