import { describe, expect, it } from "vitest";

import type { StorageObject } from "@/services/tauri";

import { nextSort, sortEntries } from "./sort";

function file(name: string, size: number, lastModified: string): StorageObject {
  return {
    key: name,
    name,
    size,
    lastModified,
    etag: null,
    storageClass: null,
    isPrefix: false,
  };
}
function folder(name: string): StorageObject {
  return {
    key: name,
    name,
    size: null,
    lastModified: null,
    etag: null,
    storageClass: null,
    isPrefix: true,
  };
}

const entries = [
  file("img10.png", 300, "2026-10-03T00:00:00Z"),
  folder("b/"),
  file("img2.png", 100, "2026-10-01T00:00:00Z"),
  folder("a/"),
  file("Notes.txt", 200, "2026-10-02T00:00:00Z"),
];
const names = (list: StorageObject[]) => list.map((e) => e.name);

describe("sortEntries", () => {
  it("sorts names naturally with folders first", () => {
    expect(names(sortEntries(entries, { key: "name", direction: "asc" }))).toEqual([
      "a/",
      "b/",
      "img2.png",
      "img10.png",
      "Notes.txt",
    ]);
    expect(names(sortEntries(entries, { key: "name", direction: "desc" }))).toEqual([
      "b/",
      "a/",
      "Notes.txt",
      "img10.png",
      "img2.png",
    ]);
  });

  it("sorts files by size and date, keeping folders by name", () => {
    expect(names(sortEntries(entries, { key: "size", direction: "desc" }))).toEqual([
      "a/",
      "b/",
      "img10.png",
      "Notes.txt",
      "img2.png",
    ]);
    expect(names(sortEntries(entries, { key: "lastModified", direction: "asc" }))).toEqual([
      "a/",
      "b/",
      "img2.png",
      "Notes.txt",
      "img10.png",
    ]);
  });

  it("does not mutate its input", () => {
    const copy = [...entries];
    sortEntries(entries, { key: "size", direction: "asc" });
    expect(entries).toEqual(copy);
  });
});

describe("nextSort", () => {
  it("flips the active column and starts new ones sensibly", () => {
    expect(nextSort({ key: "name", direction: "asc" }, "name").direction).toBe("desc");
    expect(nextSort({ key: "name", direction: "asc" }, "size")).toEqual({
      key: "size",
      direction: "desc",
    });
    expect(nextSort({ key: "size", direction: "desc" }, "name").direction).toBe("asc");
  });
});
