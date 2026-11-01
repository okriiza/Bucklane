<script setup lang="ts">
import { ExternalLink, Sparkles } from "@lucide/vue";
import { computed } from "vue";
import { toast } from "vue-sonner";

import ReleaseNotes from "@/components/common/ReleaseNotes.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { commands, toAppError, unwrap } from "@/services/tauri";
import { useWhatsNewStore } from "@/stores/whatsNew";
import { formatDate } from "@/utils/format";
import { releaseNotes } from "@/utils/changelog";

const whatsNew = useWhatsNewStore();
const release = computed(() => (whatsNew.version ? releaseNotes(whatsNew.version) : null));

async function openReleases() {
  if (!whatsNew.version) return;
  try {
    await unwrap(commands.openReleasePage(whatsNew.version));
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <Dialog v-model:open="whatsNew.open">
    <DialogContent class="max-w-lg">
      <div class="space-y-1">
        <DialogTitle class="flex items-center gap-2 text-lg font-semibold">
          <Sparkles class="text-primary size-5" />
          {{ $t("whatsNew.title", { version: whatsNew.version ?? "" }) }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-[13px]">
          <template v-if="release?.date">{{ formatDate(release.date) }}</template>
        </DialogDescription>
      </div>

      <div class="max-h-[55vh] overflow-auto pr-1">
        <ReleaseNotes v-if="release" :sections="release.sections" />
        <p v-else class="text-muted-foreground text-[13px]">{{ $t("whatsNew.missing") }}</p>
      </div>

      <div class="flex justify-end gap-2">
        <Button variant="ghost" @click="openReleases">
          <ExternalLink /> {{ $t("whatsNew.allReleases") }}
        </Button>
        <Button @click="whatsNew.open = false">{{ $t("whatsNew.close") }}</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
