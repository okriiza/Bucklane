import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";

import { useWhatsNewStore } from "./whatsNew";

describe("useWhatsNewStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
  });

  it("stays quiet on a fresh install", () => {
    const store = useWhatsNewStore();
    store.checkAfterUpdate("0.8.0");
    expect(store.open).toBe(false);
  });

  it("shows the notes once after an update", () => {
    localStorage.setItem("bucklane.lastSeenVersion", "0.7.0");
    const store = useWhatsNewStore();

    store.checkAfterUpdate("0.8.0");
    expect([store.open, store.version]).toEqual([true, "0.8.0"]);

    store.open = false;
    store.checkAfterUpdate("0.8.0");
    expect(store.open).toBe(false);
  });

  it("treats saved connections without a record as an update from 0.8.0 or earlier", () => {
    const store = useWhatsNewStore();
    store.checkAfterUpdate("0.8.0", true);
    expect(store.open).toBe(true);
  });

  it("skips versions without bundled notes", () => {
    localStorage.setItem("bucklane.lastSeenVersion", "0.7.0");
    const store = useWhatsNewStore();
    store.checkAfterUpdate("0.0.1");
    expect(store.open).toBe(false);
  });
});
