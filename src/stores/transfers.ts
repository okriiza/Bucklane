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

/** Weight of the newest sample in the smoothed speed (exponential average). */
const SPEED_SMOOTHING = 0.3;
/** Samples closer together than this are merged to avoid noisy spikes. */
const MIN_SAMPLE_MS = 250;

interface SpeedSample {
  bytes: number;
  at: number;
  /** Smoothed bytes per second; null until two samples exist. */
  rate: number | null;
}

/**
 * Transfer queue and history. The list is loaded from SQLite on startup and
 * kept current by `transferUpdate` events, which are informational only: on
 * (re)start the backend list is authoritative (TSD §6.7).
 */
export const useTransferStore = defineStore("transfers", () => {
  const byId = ref(new Map<string, TransferDto>());
  const loadError = ref<AppError | null>(null);
  const completionListeners = new Set<Listener>();
  const speeds = ref(new Map<string, SpeedSample>());
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

  /** Updates the smoothed speed of a running transfer from a progress event. */
  function sampleSpeed(t: TransferDto) {
    const next = new Map(speeds.value);
    if (t.status !== "RUNNING") {
      if (!next.delete(t.id)) return;
      speeds.value = next;
      return;
    }
    const now = Date.now();
    const bytes = t.bytesTransferred ?? 0;
    const prev = next.get(t.id);
    if (!prev || bytes < prev.bytes) {
      // First sample, or progress restarted (e.g. a retry).
      next.set(t.id, { bytes, at: now, rate: null });
    } else {
      const elapsed = now - prev.at;
      if (elapsed < MIN_SAMPLE_MS) return;
      const instant = ((bytes - prev.bytes) / elapsed) * 1000;
      const rate =
        prev.rate === null ? instant : prev.rate + SPEED_SMOOTHING * (instant - prev.rate);
      next.set(t.id, { bytes, at: now, rate });
    }
    speeds.value = next;
  }

  /** Bytes per second, or null when not running or not yet measured. */
  function speedOf(id: string): number | null {
    return speeds.value.get(id)?.rate ?? null;
  }

  /** Seconds remaining, or null when it can't be estimated. */
  function etaOf(t: TransferDto): number | null {
    const rate = speedOf(t.id);
    if (!rate || rate <= 0 || t.bytesTotal == null) return null;
    return Math.max(0, (t.bytesTotal - (t.bytesTransferred ?? 0)) / rate);
  }

  async function reveal(id: string) {
    await unwrap(commands.revealTransfer(id));
  }

  function upsert(t: TransferDto) {
    sampleSpeed(t);
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
    speedOf,
    etaOf,
    reveal,
    onCompleted,
    cancel,
    retry,
    clearFinished,
  };
});
