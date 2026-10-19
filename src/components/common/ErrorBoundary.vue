<script setup lang="ts">
import { CircleAlert, Home, RotateCw } from "@lucide/vue";
import { onErrorCaptured, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { reportError } from "@/utils/errorReporting";

/**
 * Shows a recovery screen instead of a blank page when a view throws while
 * rendering. Navigating elsewhere clears it.
 */
const failed = ref(false);
const route = useRoute();
const router = useRouter();

onErrorCaptured((err, _instance, info) => {
  reportError(err, `render: ${info}`);
  failed.value = true;
  return false;
});

watch(
  () => route.fullPath,
  () => (failed.value = false),
);

function reload() {
  window.location.reload();
}
</script>

<template>
  <div v-if="failed" class="grid h-full place-items-center p-8 text-center" role="alert">
    <div class="max-w-sm">
      <CircleAlert class="text-destructive mx-auto size-8" />
      <h1 class="mt-3 text-[15px] font-semibold">{{ $t("errorBoundary.title") }}</h1>
      <p class="text-muted-foreground mt-1.5 text-[13px]">{{ $t("errorBoundary.body") }}</p>
      <div class="mt-5 flex justify-center gap-2">
        <Button variant="outline" size="sm" @click="router.push('/')">
          <Home /> {{ $t("errorBoundary.home") }}
        </Button>
        <Button size="sm" @click="reload"><RotateCw /> {{ $t("errorBoundary.reload") }}</Button>
      </div>
    </div>
  </div>
  <slot v-else />
</template>
