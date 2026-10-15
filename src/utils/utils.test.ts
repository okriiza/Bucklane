import { describe, expect, it } from "vitest";

import { formatBytes, formatDateTime } from "./format";
import { extensionOf, parentPrefix, prefixSegments } from "./prefix";

describe("formatBytes", () => {
  it.each([
    [null, "—"],
    [-1, "—"],
    [0, "0 B"],
    [1023, "1023 B"],
    [1024, "1.0 KB"],
    [1536, "1.5 KB"],
    [10 * 1024 * 1024, "10 MB"],
    [5 * 1024 ** 4, "5.0 TB"],
  ])("formats %s as %s", (input, expected) => {
    expect(formatBytes(input)).toBe(expected);
  });
});

describe("formatDateTime", () => {
  it("returns a dash for missing or invalid dates", () => {
    expect(formatDateTime(null)).toBe("—");
    expect(formatDateTime("not a date")).toBe("—");
    expect(formatDateTime("2026-10-14T08:30:00Z")).not.toBe("—");
  });
});

describe("prefixSegments", () => {
  it("splits nested folders", () => {
    expect(prefixSegments("photos/2026/")).toEqual([
      { name: "photos", prefix: "photos/" },
      { name: "2026", prefix: "photos/2026/" },
    ]);
  });

  it("returns nothing at the root", () => {
    expect(prefixSegments("")).toEqual([]);
  });

  it("keeps empty segments because S3 keys are opaque", () => {
    expect(prefixSegments("a//b/").map((s) => s.prefix)).toEqual(["a/", "a//", "a//b/"]);
  });
});

describe("parentPrefix", () => {
  it("walks one level up", () => {
    expect(parentPrefix("photos/2026/")).toBe("photos/");
    expect(parentPrefix("photos/")).toBe("");
    expect(parentPrefix("")).toBe("");
  });
});

describe("extensionOf", () => {
  it("handles dotfiles and missing extensions", () => {
    expect(extensionOf("Report.PDF")).toBe("pdf");
    expect(extensionOf(".env")).toBe("");
    expect(extensionOf("Makefile")).toBe("");
  });
});
