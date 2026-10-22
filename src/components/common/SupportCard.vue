<script setup lang="ts">
import { Heart } from "@lucide/vue";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { commands, toAppError, unwrap } from "@/services/tauri";

async function openSupport() {
  try {
    await unwrap(commands.openSupportPage());
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <section class="bg-card grid gap-3 rounded-xl border p-5 text-[13px] shadow-[var(--shadow-soft)]">
    <h2 class="text-muted-foreground text-[11px] font-medium">{{ $t("support.title") }}</h2>
    <p class="text-muted-foreground">{{ $t("support.body") }}</p>
    <Button size="sm" class="justify-self-start" @click="openSupport">
      <Heart /> {{ $t("support.action") }}
    </Button>
  </section>
</template>
