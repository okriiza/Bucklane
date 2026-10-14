import { onScopeDispose, ref } from "vue";

/** Pixels the pointer must travel before a press becomes a drag. */
const DRAG_THRESHOLD = 5;
const DROP_HIGHLIGHT = ["ring-2", "ring-primary", "ring-inset", "bg-accent"];

export interface DragState {
  keys: string[];
  x: number;
  y: number;
  targetPrefix: string | null;
}

/**
 * Pointer-based drag of explorer rows onto folders.
 *
 * HTML5 drag-and-drop is avoided on purpose: Tauri's native file-drop
 * handler (needed for uploads from the OS) disables it on Windows.
 *
 * Markup contract:
 * - draggable elements carry `data-drag-key` (the object key);
 * - drop targets carry `data-drop-prefix` (the destination folder prefix).
 */
export function useObjectDrag(options: {
  /** Keys to move when `key` is dragged (e.g. the whole checked selection). */
  keysFor: (key: string) => string[];
  /** Whether `prefix` is a valid destination for the dragged keys. */
  canDrop: (keys: string[], prefix: string) => boolean;
  onDrop: (keys: string[], prefix: string) => void;
}) {
  const drag = ref<DragState | null>(null);
  let pending: { key: string; x: number; y: number } | null = null;
  let highlighted: Element | null = null;
  let suppressClick = false;

  function setHighlight(el: Element | null) {
    if (el === highlighted) return;
    highlighted?.classList.remove(...DROP_HIGHLIGHT);
    el?.classList.add(...DROP_HIGHLIGHT);
    highlighted = el;
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    const target = event.target as Element;
    // Never start a drag from controls inside a row.
    if (target.closest("input, button, a, [role='menu']")) return;
    const row = target.closest("[data-drag-key]");
    if (!row) return;
    pending = { key: row.getAttribute("data-drag-key")!, x: event.clientX, y: event.clientY };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp, { once: true });
    window.addEventListener("keydown", onKeyDown);
  }

  function onPointerMove(event: PointerEvent) {
    if (pending && !drag.value) {
      const distance = Math.hypot(event.clientX - pending.x, event.clientY - pending.y);
      if (distance < DRAG_THRESHOLD) return;
      drag.value = { keys: options.keysFor(pending.key), x: 0, y: 0, targetPrefix: null };
      document.body.style.cursor = "grabbing";
    }
    if (!drag.value) return;

    drag.value.x = event.clientX;
    drag.value.y = event.clientY;
    const dropEl = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest("[data-drop-prefix]");
    const prefix = dropEl?.getAttribute("data-drop-prefix") ?? null;
    const valid = prefix !== null && options.canDrop(drag.value.keys, prefix);
    drag.value.targetPrefix = valid ? prefix : null;
    setHighlight(valid ? dropEl! : null);
  }

  function onPointerUp() {
    const finished = drag.value;
    cleanup();
    if (!finished) return;
    // The pointerup also produces a click on the row; don't treat it as one.
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
    if (finished.targetPrefix !== null) options.onDrop(finished.keys, finished.targetPrefix);
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") cleanup();
  }

  function cleanup() {
    pending = null;
    drag.value = null;
    setHighlight(null);
    document.body.style.cursor = "";
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("keydown", onKeyDown);
  }

  /** Call from a capture-phase click handler to swallow the post-drag click. */
  function shouldSuppressClick() {
    return suppressClick;
  }

  onScopeDispose(cleanup);

  return { drag, onPointerDown, shouldSuppressClick };
}
