export interface PrefixSegment {
  /** Folder name without the trailing delimiter. */
  name: string;
  /** Full prefix up to and including this folder. */
  prefix: string;
}

/**
 * Splits an S3 prefix into breadcrumb segments: `"a/b/"` → `a` (`a/`), `b` (`a/b/`).
 * Keys are opaque identifiers, so empty segments (`a//b/`) are kept rather
 * than normalized away; they are real, distinct prefixes in S3.
 */
export function prefixSegments(prefix: string, delimiter = "/"): PrefixSegment[] {
  if (!prefix) return [];
  const parts = prefix.split(delimiter);
  // A folder prefix ends with the delimiter, leaving one trailing empty part.
  if (parts.at(-1) === "") parts.pop();

  let acc = "";
  return parts.map((name) => {
    acc += name + delimiter;
    return { name, prefix: acc };
  });
}

/** Prefix of the containing folder, or `""` at the bucket root. */
export function parentPrefix(prefix: string, delimiter = "/"): string {
  const segments = prefixSegments(prefix, delimiter);
  return segments.at(-2)?.prefix ?? "";
}

/** File extension in lowercase, without the dot; `""` if none. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/**
 * Whether an object uploaded to `key` changes the listing of the folder at
 * `prefix`: a file directly inside it always does; a file deeper down only
 * does when its top-level subfolder isn't listed yet. This lets a folder
 * upload of thousands of files refresh the view once, not once per file.
 */
export function uploadChangesListing(
  prefix: string,
  key: string,
  listedKeys: ReadonlySet<string>,
): boolean {
  if (!key.startsWith(prefix)) return false;
  const rest = key.slice(prefix.length);
  const slash = rest.indexOf("/");
  if (slash === -1) return true;
  return !listedKeys.has(prefix + rest.slice(0, slash + 1));
}
