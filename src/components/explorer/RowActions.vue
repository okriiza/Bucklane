<script setup lang="ts">
import { Copy, Download, FolderInput, Info, MoreHorizontal, Pencil, Trash2 } from "@lucide/vue";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "reka-ui";

import type { StorageObject } from "@/services/tauri";

defineProps<{ entry: StorageObject }>();
const emit = defineEmits<{
  details: [];
  download: [];
  rename: [];
  copy: [];
  move: [];
  delete: [];
}>();

const itemClass =
  "flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none select-none data-[highlighted]:bg-accent data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:size-4";
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger
      class="hover:bg-accent focus-visible:ring-ring/50 data-[state=open]:bg-accent inline-flex size-7 items-center justify-center rounded-md outline-none focus-visible:ring-2"
      :aria-label="$t('explorer.actions', { name: entry.name })"
      @click.stop
      @keydown.enter.stop
    >
      <MoreHorizontal class="size-4" />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        align="end"
        :side-offset="4"
        class="bg-popover text-popover-foreground z-50 min-w-40 rounded-md border p-1 shadow-md"
      >
        <template v-if="!entry.isPrefix">
          <DropdownMenuItem :class="itemClass" @select="emit('details')">
            <Info /> {{ $t("explorer.details") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('download')">
            <Download /> {{ $t("transfers.downloadAction") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('rename')">
            <Pencil /> {{ $t("ops.rename") }}
            <span class="text-muted-foreground ml-auto pl-4 text-xs">F2</span>
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('copy')">
            <Copy /> {{ $t("ops.copy") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('move')">
            <FolderInput /> {{ $t("ops.move") }}
          </DropdownMenuItem>
          <DropdownMenuSeparator class="bg-border -mx-1 my-1 h-px" />
        </template>
        <template v-else>
          <!-- Shown disabled so users learn why folders can't be renamed yet. -->
          <DropdownMenuItem :class="itemClass" disabled>
            <Pencil /> {{ $t("ops.rename") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" disabled>
            <FolderInput /> {{ $t("ops.move") }}
          </DropdownMenuItem>
          <p class="text-muted-foreground max-w-56 px-2 pt-0.5 pb-1.5 text-xs">
            {{ $t("ops.folderRenameUnsupported") }}
          </p>
          <DropdownMenuSeparator class="bg-border -mx-1 my-1 h-px" />
        </template>
        <DropdownMenuItem
          :class="[itemClass, 'text-destructive data-[highlighted]:text-destructive']"
          @select="emit('delete')"
        >
          <Trash2 /> {{ $t("common.delete") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
