<script setup lang="ts">
import { KeyRound, Plus, ShieldCheck, Zap } from "@lucide/vue";

import BrandMark from "@/components/common/BrandMark.vue";
import ProviderMark from "@/components/connections/ProviderMark.vue";
import { Button } from "@/components/ui/button";
import { useConnectionStore } from "@/stores/connections";

const store = useConnectionStore();

const points = [
  { icon: KeyRound, title: "welcome.points.keychain.title", body: "welcome.points.keychain.body" },
  { icon: Zap, title: "welcome.points.transfers.title", body: "welcome.points.transfers.body" },
  { icon: ShieldCheck, title: "welcome.points.safe.title", body: "welcome.points.safe.body" },
] as const;
</script>

<template>
  <div class="flex min-h-full items-center px-12 py-10">
    <div class="w-full max-w-2xl">
      <BrandMark class="size-10" />
      <h1 class="mt-6 text-3xl leading-[1.15] font-semibold tracking-[-0.02em]">
        {{ $t("welcome.title") }}
      </h1>
      <p class="text-muted-foreground mt-3 max-w-[52ch] text-[15px] leading-relaxed">
        {{ $t("welcome.description") }}
      </p>

      <div class="mt-7 flex items-center gap-4">
        <Button size="lg" @click="store.openCreateForm()">
          <Plus /> {{ $t("welcome.addConnection") }}
        </Button>
        <div class="flex -space-x-1.5" :aria-label="$t('welcome.providers')">
          <ProviderMark
            v-for="p in store.profiles"
            :key="p.provider"
            :provider="p.provider"
            class="ring-background ring-2"
            :title="p.label"
          />
        </div>
      </div>

      <dl class="mt-12 grid gap-x-8 gap-y-6 border-t pt-8 sm:grid-cols-3">
        <div v-for="point in points" :key="point.title">
          <dt class="flex items-center gap-2 text-[13px] font-medium">
            <component :is="point.icon" class="text-primary size-4" />
            {{ $t(point.title) }}
          </dt>
          <dd class="text-muted-foreground mt-1.5 text-[13px] leading-relaxed">
            {{ $t(point.body) }}
          </dd>
        </div>
      </dl>
    </div>
  </div>
</template>
