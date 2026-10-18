<script setup lang="ts">
import { ChevronDown, File, Folder, Upload } from "@lucide/vue";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";

import { buttonVariants } from "@/components/ui/button";

defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ files: []; folder: [] }>();

const itemClass =
  "flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-[13px] outline-none select-none data-[highlighted]:bg-accent [&_svg]:size-4";
</script>

<!-- Upload files or a whole folder; both keep the current folder as target. -->
<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger
      :disabled="disabled"
      :class="buttonVariants({ size: 'sm' })"
      :aria-label="$t('transfers.uploadAction')"
    >
      <Upload /> {{ $t("transfers.uploadAction") }}
      <ChevronDown class="-mr-0.5 opacity-70" />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        align="start"
        :side-offset="4"
        class="bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 z-50 min-w-40 rounded-lg border p-1 shadow-[var(--shadow-float)]"
      >
        <DropdownMenuItem :class="itemClass" @select="emit('files')">
          <File /> {{ $t("transfers.uploadFiles") }}
        </DropdownMenuItem>
        <DropdownMenuItem :class="itemClass" @select="emit('folder')">
          <Folder /> {{ $t("transfers.uploadFolder") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
