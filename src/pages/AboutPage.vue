<script setup lang="ts">
import {
  ArrowLeftRight,
  Check,
  FolderTree,
  Layers,
  MousePointerClick,
  ShieldCheck,
  Star,
  X,
} from "@lucide/vue";
import { computed } from "vue";

import appIcon from "../../src-tauri/icons/128x128@2x.png";
import ProviderMark from "@/components/connections/ProviderMark.vue";
import { useAppStore } from "@/stores/app";
import { useConnectionStore } from "@/stores/connections";

const app = useAppStore();
const connections = useConnectionStore();

type Platform = "mac" | "windows" | "linux";
const platform = computed<Platform>(() => {
  const ua = navigator.userAgent;
  if (ua.includes("Mac")) return "mac";
  if (ua.includes("Windows")) return "windows";
  return "linux";
});

const stores = [
  "about.privacy.stores.connections",
  "about.privacy.stores.bookmarks",
  "about.privacy.stores.history",
  "about.privacy.stores.settings",
  "about.privacy.stores.logs",
] as const;
const never = [
  "about.privacy.never.secrets",
  "about.privacy.never.files",
  "about.privacy.never.telemetry",
] as const;

const shortcuts = [
  { keys: ["Enter"], label: "about.shortcuts.open" },
  { keys: ["Space"], label: "about.shortcuts.preview" },
  { keys: ["F2"], label: "about.shortcuts.rename" },
  { keys: ["Delete"], label: "about.shortcuts.delete" },
  { keys: ["Esc"], label: "about.shortcuts.cancelDrag" },
] as const;

const features = [
  { icon: Layers, key: "providers" },
  { icon: FolderTree, key: "explorer" },
  { icon: ArrowLeftRight, key: "transfers" },
  { icon: MousePointerClick, key: "dragDrop" },
  { icon: ShieldCheck, key: "safeOps" },
  { icon: Star, key: "bookmarks" },
] as const;
</script>

<template>
  <div class="mx-auto max-w-4xl px-10 pt-8 pb-14">
    <header class="flex items-center gap-5">
      <img
        :src="appIcon"
        alt=""
        class="size-20 rounded-[22%] shadow-[var(--shadow-float)]"
        draggable="false"
      />
      <div>
        <h1 class="text-3xl font-semibold tracking-[-0.02em]">{{ $t("app.name") }}</h1>
        <p class="text-muted-foreground mt-1 text-[15px]">{{ $t("about.tagline") }}</p>
        <p class="mt-2 flex items-center gap-2 text-[13px]">
          <span
            class="bg-primary/10 text-primary tabular rounded-md px-2 py-0.5 font-mono text-xs font-medium"
          >
            v{{ app.health?.appVersion ?? "—" }}
          </span>
          <span class="text-muted-foreground">{{ $t(`about.platforms.${platform}`) }}</span>
        </p>
      </div>
    </header>

    <p class="text-muted-foreground mt-8 max-w-[62ch] text-[15px] leading-relaxed">
      {{ $t("about.description") }}
    </p>

    <section class="mt-10">
      <h2 class="text-[15px] font-semibold tracking-tight">{{ $t("about.features.title") }}</h2>
      <ul class="mt-4 grid gap-x-8 gap-y-6 sm:grid-cols-2">
        <li v-for="f in features" :key="f.key" class="flex gap-3.5">
          <span
            class="bg-primary/10 text-primary inline-flex size-8 shrink-0 items-center justify-center rounded-lg"
          >
            <component :is="f.icon" class="size-4" />
          </span>
          <div>
            <h3 class="text-[13px] font-medium">{{ $t(`about.features.${f.key}.title`) }}</h3>
            <p class="text-muted-foreground mt-1 text-[13px] leading-relaxed">
              {{ $t(`about.features.${f.key}.body`) }}
            </p>
          </div>
        </li>
      </ul>
    </section>

    <div class="mt-12 grid gap-10 border-t pt-10 lg:grid-cols-[minmax(0,1fr)_17rem]">
      <div class="grid content-start gap-10">
        <section>
          <h2 class="text-[15px] font-semibold tracking-tight">{{ $t("about.privacy.title") }}</h2>
          <div class="mt-4 grid gap-6 sm:grid-cols-2">
            <div>
              <h3 class="text-muted-foreground text-[11px] font-medium">
                {{ $t("about.privacy.storesTitle") }}
              </h3>
              <ul class="mt-2.5 grid gap-2">
                <li v-for="item in stores" :key="item" class="flex gap-2.5 text-[13px]">
                  <Check class="text-success mt-0.5 size-3.5 shrink-0" />
                  {{ $t(item) }}
                </li>
              </ul>
            </div>
            <div>
              <h3 class="text-muted-foreground text-[11px] font-medium">
                {{ $t("about.privacy.neverTitle") }}
              </h3>
              <ul class="mt-2.5 grid gap-2">
                <li v-for="item in never" :key="item" class="flex gap-2.5 text-[13px]">
                  <X class="text-muted-foreground mt-0.5 size-3.5 shrink-0" />
                  {{ $t(item) }}
                </li>
              </ul>
            </div>
          </div>
        </section>

        <section>
          <h2 class="text-[15px] font-semibold tracking-tight">
            {{ $t("about.shortcuts.title") }}
          </h2>
          <dl class="mt-3 divide-y rounded-xl border">
            <div
              v-for="s in shortcuts"
              :key="s.label"
              class="flex items-center justify-between px-4 py-2.5 text-[13px]"
            >
              <dt>{{ $t(s.label) }}</dt>
              <dd class="flex gap-1">
                <kbd
                  v-for="key in s.keys"
                  :key="key"
                  class="bg-muted text-muted-foreground min-w-6 rounded-md border px-1.5 py-0.5 text-center font-mono text-[11px] shadow-[0_1px_0] shadow-border"
                >
                  {{ key }}
                </kbd>
              </dd>
            </div>
          </dl>
        </section>
      </div>

      <aside class="grid content-start gap-8">
        <dl
          class="bg-card grid gap-3.5 rounded-xl border p-5 text-[13px] shadow-[var(--shadow-soft)]"
        >
          <div>
            <dt class="text-muted-foreground text-[11px]">{{ $t("about.details.credentials") }}</dt>
            <dd class="mt-0.5 font-medium">{{ $t(`about.keychains.${platform}`) }}</dd>
          </div>
          <div>
            <dt class="text-muted-foreground text-[11px]">{{ $t("about.details.telemetry") }}</dt>
            <dd class="mt-0.5 font-medium">{{ $t("about.details.telemetryValue") }}</dd>
          </div>
          <div>
            <dt class="text-muted-foreground text-[11px]">{{ $t("about.details.account") }}</dt>
            <dd class="mt-0.5 font-medium">{{ $t("about.details.accountValue") }}</dd>
          </div>
        </dl>

        <section>
          <h2 class="text-muted-foreground text-[11px] font-medium">{{ $t("about.providers") }}</h2>
          <ul class="mt-2.5 grid gap-1.5">
            <li
              v-for="p in connections.profiles"
              :key="p.provider"
              class="flex items-center gap-2.5 text-[13px]"
            >
              <ProviderMark :provider="p.provider" />
              {{ p.label }}
            </li>
          </ul>
        </section>
      </aside>
    </div>
  </div>
</template>
