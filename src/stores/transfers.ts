import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  commands,
  events,
  toAppError,
  unwrap,
  type AppError,
  type TransferDto,
} from "@/services/tauri";

type Listener = (t: TransferDto) => void;

/**
 * Transfer queue and history. The list is loaded from SQLite on startup and
 * kept current by `transferUpdate` events, which are informational only: on
 * (re)start the backend list is authoritative (TSD §6.7).
 */
export const useTransferStore = defineStore("transfers", () => {
  const byId = ref(new Map<string, TransferDto>());
  const loadError = ref<AppError | null>(null);
  const completionListeners = new Set<Listener>();
  let started = false;

  const transfers = computed(() =>
    [...byId.value.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
  );
  const activeCount = computed(
    () => transfers.value.filter((t) => t.status === "QUEUED" || t.status === "RUNNING").length,
  );
  const failedCount = computed(() => transfers.value.filter((t) => t.status === "FAILED").length);
  const hasFinished = computed(() =>
    transfers.value.some((t) => ["COMPLETED", "FAILED", "CANCELED"].includes(t.status)),
  );

  function upsert(t: TransferDto) {
    const previous = byId.value.get(t.id);
    const next = new Map(byId.value);
    next.set(t.id, t);
    byId.value = next;
    if (t.status === "COMPLETED" && previous?.status !== "COMPLETED") {
      completionListeners.forEach((listener) => listener(t));
    }
  }

  async function load() {
    try {
      const list = await unwrap(commands.listTransfers());
      byId.value = new Map(list.map((t) => [t.id, t]));
      loadError.value = null;
    } catch (err) {
      loadError.value = toAppError(err);
    }
  }

  /** Subscribes to backend events once, then loads the authoritative list. */
  async function start() {
    if (started) return;
    started = true;
    await events.transferUpdate.listen((event) => upsert(event.payload));
    await load();
  }

  /** Called whenever a transfer reaches COMPLETED; returns an unsubscribe fn. */
  function onCompleted(listener: Listener) {
    completionListeners.add(listener);
    return () => completionListeners.delete(listener);
  }

  async function cancel(id: string) {
    await unwrap(commands.cancelTransfer(id));
  }

  async function retry(id: string) {
    upsert(await unwrap(commands.retryTransfer(id)));
  }

  async function clearFinished() {
    await unwrap(commands.clearTransferHistory());
    await load();
  }

  return {
    transfers,
    activeCount,
    failedCount,
    hasFinished,
    loadError,
    start,
    load,
    upsert,
    onCompleted,
    cancel,
    retry,
    clearFinished,
  };
});
