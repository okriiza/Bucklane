<script setup lang="ts">
import { ArrowDownCircle, Heart, Info, Settings } from "@lucide/vue";
import { toast } from "vue-sonner";

import ConnectionList from "@/components/connections/ConnectionList.vue";
import { commands, toAppError, unwrap } from "@/services/tauri";
import { useUpdateStore } from "@/stores/updates";

import BackendStatusBadge from "./BackendStatusBadge.vue";
import BrandMark from "./BrandMark.vue";

const links = [
  { to: "/settings", label: "nav.settings", icon: Settings },
  { to: "/about", label: "nav.about", icon: Info },
] as const;

const updates = useUpdateStore();

async function openSupport() {
  try {
    await unwrap(commands.openSupportPage());
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <aside
    class="bg-sidebar text-sidebar-foreground border-sidebar-border flex flex-col border-r"
    :aria-label="$t('nav.connections')"
  >
    <!-- On macOS this row sits under the traffic lights and drags the window. -->
    <div data-tauri-drag-region class="mac:h-11 h-3 shrink-0" />

    <RouterLink
      to="/"
      class="hover:bg-sidebar-accent/60 mx-2 mb-2 flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
    >
      <BrandMark class="size-5" />
      <span class="text-sidebar-accent-foreground text-[15px] font-semibold tracking-tight">
        {{ $t("app.name") }}
      </span>
    </RouterLink>

    <nav class="min-h-0 flex-1 overflow-auto px-2 pb-2">
      <ConnectionList />
    </nav>

    <div class="border-sidebar-border flex flex-col gap-px border-t p-2">
      <RouterLink
        v-if="updates.update && ['available', 'installing', 'ready'].includes(updates.status)"
        to="/about"
        class="bg-primary/10 text-primary hover:bg-primary/15 mb-1 flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] font-medium transition-colors"
      >
        <ArrowDownCircle class="size-4" />
        {{ $t("updates.sidebar", { version: updates.update.version }) }}
      </RouterLink>
      <RouterLink
        v-for="link in links"
        :key="link.to"
        :to="link.to"
        class="hover:bg-sidebar-accent hover:text-sidebar-accent-foreground flex items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] transition-colors"
        active-class="bg-sidebar-accent text-sidebar-accent-foreground font-medium"
      >
        <component :is="link.icon" class="size-4 opacity-70" />
        {{ $t(link.label) }}
      </RouterLink>
      <button
        type="button"
        class="hover:bg-sidebar-accent hover:text-sidebar-accent-foreground flex items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-[13px] transition-colors"
        @click="openSupport"
      >
        <Heart class="size-4 text-rose-500 opacity-80" />
        {{ $t("nav.support") }}
      </button>
      <BackendStatusBadge class="mt-1.5 px-2" />
    </div>
  </aside>
</template>
