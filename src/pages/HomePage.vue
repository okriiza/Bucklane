<script setup lang="ts">
import { ArrowDown, ArrowRight, ArrowUp, Database, Plus, Star } from "@lucide/vue";
import { computed, ref, watch } from "vue";

import ProviderMark from "@/components/connections/ProviderMark.vue";
import OnboardingHero from "@/components/home/OnboardingHero.vue";
import { Button } from "@/components/ui/button";
import { commands, unwrap, type BucketBookmark, type TransferDto } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";
import { useTransferStore } from "@/stores/transfers";
import { endpointHost, formatBytes, formatRelative } from "@/utils/format";
import { TRANSFER_ICONS, transferKind, transferName } from "@/utils/transfer";

const connections = useConnectionStore();
const transfers = useTransferStore();

const RECENT_LIMIT = 8;

/** Bookmarks from every connection, loaded locally (no network calls). */
const pinned = ref<(BucketBookmark & { connectionName: string })[]>([]);

watch(
  () => connections.connections.map((c) => c.id).join(),
  async () => {
    const results = await Promise.allSettled(
      connections.connections.map(async (c) =>
        (await unwrap(commands.listBucketBookmarks(c.id))).map((b) => ({
          ...b,
          connectionName: c.name,
        })),
      ),
    );
    pinned.value = results.flatMap((r) => (r.status === "fulfilled" ? r.value : []));
  },
  { immediate: true },
);

const recent = computed(() => transfers.transfers.slice(0, RECENT_LIMIT));

/** Completed transfers since local midnight, split by direction. */
const today = computed(() => {
  const midnight = new Date();
  midnight.setHours(0, 0, 0, 0);
  const done = transfers.transfers.filter(
    (t) => t.status === "COMPLETED" && t.completedAt && new Date(t.completedAt) >= midnight,
  );
  const sum = (dir: TransferDto["direction"]) => {
    const items = done.filter((t) => t.direction === dir);
    return { count: items.length, bytes: items.reduce((n, t) => n + (t.bytesTotal ?? 0), 0) };
  };
  return { up: sum("UPLOAD"), down: sum("DOWNLOAD") };
});

const running = computed(() => transfers.transfers.filter((t) => t.status === "RUNNING"));
const runningProgress = computed(() => {
  const total = running.value.reduce((n, t) => n + (t.bytesTotal ?? 0), 0);
  const done = running.value.reduce((n, t) => n + (t.bytesTransferred ?? 0), 0);
  return total ? Math.floor((done / total) * 100) : null;
});

const STATUS_DOT: Record<TransferDto["status"], string> = {
  QUEUED: "bg-muted-foreground/50",
  RUNNING: "bg-primary animate-pulse",
  COMPLETED: "bg-success",
  FAILED: "bg-destructive",
  CANCELED: "bg-muted-foreground/50",
};
</script>

<template>
  <OnboardingHero v-if="!connections.loading && !connections.connections.length" />

  <div v-else class="mx-auto max-w-5xl px-10 pt-6 pb-12">
    <header class="flex items-end justify-between gap-6">
      <div>
        <h1 class="text-2xl font-semibold tracking-[-0.015em]">{{ $t("home.title") }}</h1>
        <p class="text-muted-foreground mt-1 text-[13px]">
          {{
            $t(
              "home.subtitle",
              { count: connections.connections.length },
              connections.connections.length,
            )
          }}
        </p>
      </div>
      <Button variant="outline" @click="connections.openCreateForm()">
        <Plus /> {{ $t("nav.addConnection") }}
      </Button>
    </header>

    <!-- Activity summary: real numbers from transfer history. -->
    <dl class="mt-7 grid grid-cols-3 divide-x rounded-xl border">
      <div class="px-5 py-4">
        <dt class="text-muted-foreground text-[11px]">{{ $t("home.active") }}</dt>
        <dd class="mt-1 flex items-baseline gap-2">
          <span class="tabular text-2xl font-semibold tracking-tight">{{ running.length }}</span>
          <span v-if="runningProgress != null" class="text-primary tabular text-xs font-medium">
            {{ runningProgress }}%
          </span>
        </dd>
        <div
          v-if="runningProgress != null"
          class="bg-muted mt-2.5 h-1 overflow-hidden rounded-full"
        >
          <div
            class="bg-primary h-full rounded-full transition-[width] duration-500"
            :style="{ width: `${runningProgress}%` }"
          />
        </div>
      </div>
      <div class="px-5 py-4">
        <dt class="text-muted-foreground flex items-center gap-1.5 text-[11px]">
          <ArrowUp class="size-3" /> {{ $t("home.uploadedToday") }}
        </dt>
        <dd class="mt-1">
          <span class="tabular text-2xl font-semibold tracking-tight">
            {{ formatBytes(today.up.bytes) }}
          </span>
          <span class="text-muted-foreground ml-2 text-xs">
            {{ $t("home.files", { count: today.up.count }, today.up.count) }}
          </span>
        </dd>
      </div>
      <div class="px-5 py-4">
        <dt class="text-muted-foreground flex items-center gap-1.5 text-[11px]">
          <ArrowDown class="size-3" /> {{ $t("home.downloadedToday") }}
        </dt>
        <dd class="mt-1">
          <span class="tabular text-2xl font-semibold tracking-tight">
            {{ formatBytes(today.down.bytes) }}
          </span>
          <span class="text-muted-foreground ml-2 text-xs">
            {{ $t("home.files", { count: today.down.count }, today.down.count) }}
          </span>
        </dd>
      </div>
    </dl>

    <div class="mt-10 grid gap-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
      <div class="grid content-start gap-10">
        <section>
          <h2 class="mb-3 text-[15px] font-semibold tracking-tight">
            {{ $t("nav.connections") }}
          </h2>
          <div class="grid gap-3 sm:grid-cols-2">
            <RouterLink
              v-for="c in connections.connections"
              :key="c.id"
              :to="{ name: 'connection', params: { id: c.id } }"
              class="group bg-card hover:border-primary/40 flex items-start gap-3 rounded-xl border p-4 shadow-[var(--shadow-soft)] transition-[border-color,box-shadow,transform] hover:shadow-[var(--shadow-float)] active:scale-[0.99]"
            >
              <ProviderMark :provider="c.provider" />
              <div class="min-w-0 flex-1">
                <p class="truncate text-[13px] font-medium">{{ c.name }}</p>
                <p class="text-muted-foreground mt-0.5 truncate font-mono text-[11px]">
                  {{ endpointHost(c.endpoint, c.region) }}
                </p>
                <p
                  class="text-muted-foreground mt-2.5 flex items-center gap-1.5 text-[11px] whitespace-nowrap"
                >
                  <span class="truncate">
                    {{ connections.profileFor(c.provider)?.label ?? c.provider }}
                  </span>
                  <span class="opacity-40">·</span>
                  <span class="font-mono">{{ c.region }}</span>
                  <span
                    v-if="c.allowInsecureHttp"
                    class="bg-warning/15 text-warning ml-auto rounded px-1.5 py-px font-medium"
                  >
                    HTTP
                  </span>
                </p>
              </div>
              <ArrowRight
                class="text-muted-foreground size-4 shrink-0 -translate-x-1 opacity-0 transition-[opacity,transform] group-hover:translate-x-0 group-hover:opacity-100"
              />
            </RouterLink>
            <button
              type="button"
              class="text-muted-foreground hover:text-foreground hover:border-primary/40 flex min-h-[5.5rem] items-center justify-center gap-2 rounded-xl border border-dashed text-[13px] transition-colors"
              @click="connections.openCreateForm()"
            >
              <Plus class="size-4" /> {{ $t("nav.addConnection") }}
            </button>
          </div>
        </section>

        <section>
          <h2 class="mb-3 flex items-center gap-2 text-[15px] font-semibold tracking-tight">
            {{ $t("home.pinned") }}
          </h2>
          <div v-if="pinned.length" class="flex flex-wrap gap-2">
            <RouterLink
              v-for="b in pinned"
              :key="b.id"
              :to="{ name: 'explorer', params: { id: b.connectionId, bucket: b.bucketName } }"
              class="bg-card hover:border-primary/40 hover:bg-accent/40 flex items-center gap-2.5 rounded-lg border py-1.5 pr-3 pl-1.5 shadow-[var(--shadow-soft)] transition-colors"
            >
              <span
                class="bg-primary/10 text-primary inline-flex size-6 items-center justify-center rounded-md"
              >
                <Database class="size-3.5" />
              </span>
              <span class="text-[13px] font-medium">{{ b.displayName ?? b.bucketName }}</span>
              <span class="text-muted-foreground text-[11px]">{{ b.connectionName }}</span>
            </RouterLink>
          </div>
          <p
            v-else
            class="text-muted-foreground flex items-center gap-2 rounded-xl border border-dashed px-4 py-4 text-[13px]"
          >
            <Star class="size-4 shrink-0" />
            {{ $t("home.pinnedEmpty") }}
          </p>
        </section>
      </div>

      <aside>
        <h2 class="mb-3 text-[15px] font-semibold tracking-tight">{{ $t("home.recent") }}</h2>
        <ol v-if="recent.length" class="relative grid gap-3.5">
          <li v-for="t in recent" :key="t.id" class="flex items-start gap-3">
            <span
              class="mt-1.5 size-1.5 shrink-0 rounded-full"
              :class="t.paused ? 'bg-warning' : STATUS_DOT[t.status]"
            />
            <div class="min-w-0 flex-1">
              <p class="flex items-center gap-1.5 text-[13px]">
                <component
                  :is="TRANSFER_ICONS[transferKind(t)]"
                  class="text-muted-foreground size-3 shrink-0"
                />
                <span class="truncate font-medium" :title="t.objectKey">{{ transferName(t) }}</span>
              </p>
              <p class="text-muted-foreground mt-0.5 truncate text-[11px]">
                {{ formatRelative(t.completedAt ?? t.startedAt ?? t.createdAt) }}
                <span class="opacity-40">·</span>
                {{ t.bucket }}
                <template v-if="t.bytesTotal != null">
                  <span class="opacity-40">·</span>
                  <span class="font-mono">{{ formatBytes(t.bytesTotal) }}</span>
                </template>
              </p>
              <p
                v-if="t.status === 'FAILED' && t.error && !t.paused"
                class="text-destructive mt-0.5 text-[11px]"
              >
                {{ t.error.message }}
              </p>
            </div>
          </li>
        </ol>
        <p v-else class="text-muted-foreground text-[13px]">{{ $t("home.recentEmpty") }}</p>
      </aside>
    </div>
  </div>
</template>
