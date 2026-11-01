import { describe, expect, it } from "vitest";

import type { TransferDto } from "@/services/tauri";

import { transferKind, transferName } from "./transfer";

const base = {
  direction: "COPY",
  objectKey: "archive/brand/logo.svg",
  localFileName: "",
  source: null,
} as unknown as TransferDto;

describe("transfer helpers", () => {
  it("tells moves apart from copies", () => {
    expect(transferKind(base)).toBe("COPY");
    const move = {
      ...base,
      source: {
        connectionId: "c1",
        connectionNameSnapshot: "S3",
        bucket: "b",
        key: "k",
        deleteAfterCopy: true,
      },
    };
    expect(transferKind(move)).toBe("MOVE");
  });

  it("names files and folder markers", () => {
    expect(transferName(base)).toBe("logo.svg");
    expect(transferName({ ...base, objectKey: "archive/brand/" })).toBe("brand/");
    expect(transferName({ ...base, direction: "UPLOAD", localFileName: "a.txt" })).toBe("a.txt");
  });
});
