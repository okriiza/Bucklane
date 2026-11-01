import changelogSource from "../../CHANGELOG.md?raw";

export interface NotesSection {
  /** "Added", "Changed", …; empty for notes without headings. */
  title: string;
  items: string[];
}

export interface Release {
  version: string;
  date: string | null;
  sections: NotesSection[];
}

export interface InlineToken {
  kind: "text" | "strong" | "code";
  text: string;
}

/**
 * Parses the body of one release: `### Heading` lines and `- item` bullets.
 * Other non-empty lines continue the previous item, or become items of
 * their own when there is none (e.g. "See https://…").
 */
export function parseNotes(markdown: string): NotesSection[] {
  const sections: NotesSection[] = [];
  let current: NotesSection | null = null;
  let item = false;
  for (const raw of markdown.split("\n")) {
    const line = raw.trim();
    if (!line) {
      item = false;
      continue;
    }
    if (line.startsWith("### ")) {
      current = { title: line.slice(4).trim(), items: [] };
      sections.push(current);
      item = false;
      continue;
    }
    if (!current) {
      current = { title: "", items: [] };
      sections.push(current);
    }
    if (line.startsWith("- ")) {
      current.items.push(line.slice(2).trim());
      item = true;
    } else if (item && current.items.length) {
      current.items[current.items.length - 1] += ` ${line}`;
    } else {
      current.items.push(line);
    }
  }
  return sections.filter((s) => s.items.length);
}

/** Releases in CHANGELOG order (newest first), without "Unreleased". */
export function parseChangelog(markdown: string): Release[] {
  const releases: Release[] = [];
  for (const block of markdown.split(/^## /m).slice(1)) {
    const newline = block.indexOf("\n");
    const heading = (newline === -1 ? block : block.slice(0, newline)).trim();
    const [version = "", date = null] = heading.split(/\s+—\s+/);
    if (!/^\d+\.\d+\.\d+/.test(version)) continue;
    releases.push({
      version,
      date,
      sections: parseNotes(newline === -1 ? "" : block.slice(newline + 1)),
    });
  }
  return releases;
}

const releases = parseChangelog(changelogSource);

/** Notes of `version` from the CHANGELOG bundled with this build. */
export function releaseNotes(version: string): Release | null {
  return releases.find((r) => r.version === version) ?? null;
}

/** Splits `**bold**`, `` `code` ``, and `[text](url)` (kept as text only). */
export function inlineTokens(text: string): InlineToken[] {
  const tokens: InlineToken[] = [];
  for (const part of text.split(/(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/)) {
    if (!part) continue;
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
      tokens.push({ kind: "strong", text: part.slice(2, -2) });
    } else if (part.startsWith("`") && part.endsWith("`") && part.length > 2) {
      tokens.push({ kind: "code", text: part.slice(1, -1) });
    } else if (part.startsWith("[")) {
      tokens.push({ kind: "text", text: part.slice(1, part.indexOf("]")) });
    } else {
      tokens.push({ kind: "text", text: part });
    }
  }
  return tokens;
}
