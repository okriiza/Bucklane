import { describe, expect, it } from "vitest";

import { inlineTokens, parseChangelog, parseNotes, releaseNotes } from "./changelog";

const SAMPLE = `# Changelog

Intro text.

## Unreleased

### Added

- Not released yet.

## 1.2.0 — 2026-11-01

### Added

- **Copy** to another bucket.
- Second item
  continued here.

### Fixed

- A bug.

## 1.1.0 — 2026-10-01

- Plain item.
`;

describe("changelog", () => {
  it("parses releases newest first and skips Unreleased", () => {
    const releases = parseChangelog(SAMPLE);

    expect(releases.map((r) => [r.version, r.date])).toEqual([
      ["1.2.0", "2026-11-01"],
      ["1.1.0", "2026-10-01"],
    ]);
    expect(releases[0]?.sections).toEqual([
      { title: "Added", items: ["**Copy** to another bucket.", "Second item continued here."] },
      { title: "Fixed", items: ["A bug."] },
    ]);
    expect(releases[1]?.sections).toEqual([{ title: "", items: ["Plain item."] }]);
  });

  it("keeps notes without headings, such as an old updater manifest", () => {
    expect(parseNotes("See https://github.com/okriiza/Bucklane/releases/tag/v0.8.0")).toEqual([
      { title: "", items: ["See https://github.com/okriiza/Bucklane/releases/tag/v0.8.0"] },
    ]);
  });

  it("splits inline formatting without rendering HTML", () => {
    expect(inlineTokens("Use **Copy**, `s3://b/k`, see [docs](https://x) <b>")).toEqual([
      { kind: "text", text: "Use " },
      { kind: "strong", text: "Copy" },
      { kind: "text", text: ", " },
      { kind: "code", text: "s3://b/k" },
      { kind: "text", text: ", see " },
      { kind: "text", text: "docs" },
      { kind: "text", text: " <b>" },
    ]);
  });

  it("bundles the project's CHANGELOG", () => {
    expect(releaseNotes("0.8.0")?.sections.length).toBeGreaterThan(0);
    expect(releaseNotes("0.0.0")).toBeNull();
  });
});
