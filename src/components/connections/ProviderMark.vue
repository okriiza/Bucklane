<script setup lang="ts">
import { computed } from "vue";

import type { ProviderType } from "@/services/tauri";

const props = withDefaults(defineProps<{ provider: ProviderType; size?: "sm" | "lg" }>(), {
  size: "sm",
});

const MONOGRAMS: Record<ProviderType, string> = {
  aws: "S3",
  supabase: "SB",
  minio: "MI",
  cloudflare_r2: "R2",
  wasabi: "WA",
  digitalocean_spaces: "DO",
  custom: "S3",
};

const monogram = computed(() => MONOGRAMS[props.provider]);
</script>

<!-- Neutral monogram tile; provider identity without adding more hues. -->
<template>
  <span
    class="bg-background text-muted-foreground ring-border inline-flex shrink-0 items-center justify-center rounded-md font-mono font-semibold tracking-tight ring-1 shadow-[var(--shadow-soft)] ring-inset"
    :class="size === 'lg' ? 'size-11 text-sm' : 'size-7 text-[10px]'"
    aria-hidden="true"
  >
    {{ monogram }}
  </span>
</template>
