<script setup lang="ts">
import {
  ClipboardCopy,
  Copy,
  Download,
  Eye,
  FolderInput,
  Info,
  Link2,
  Globe,
  MoreHorizontal,
  Pencil,
  SlidersHorizontal,
  Trash2,
} from "@lucide/vue";
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
  preview: [];
  download: [];
  share: [];
  copyKey: [];
  copyUri: [];
  copyPublicUrl: [];
  metadata: [];
  rename: [];
  copy: [];
  move: [];
  delete: [];
}>();

const itemClass =
  "flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-[13px] outline-none select-none data-[highlighted]:bg-accent data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:size-4";
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger
      class="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/40 data-[state=open]:bg-accent data-[state=open]:text-foreground inline-flex size-7 items-center justify-center rounded-md transition-colors outline-none focus-visible:ring-2"
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
        class="bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 z-50 min-w-44 rounded-lg border p-1 shadow-[var(--shadow-float)]"
      >
        <template v-if="!entry.isPrefix">
          <DropdownMenuItem :class="itemClass" @select="emit('preview')">
            <Eye /> {{ $t("preview.action") }}
            <span class="text-muted-foreground ml-auto pl-4 text-xs">{{
              $t("preview.spaceKey")
            }}</span>
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('details')">
            <Info /> {{ $t("explorer.details") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('download')">
            <Download /> {{ $t("transfers.downloadAction") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('share')">
            <Link2 /> {{ $t("share.menu") }}
          </DropdownMenuItem>
          <DropdownMenuItem :class="itemClass" @select="emit('metadata')">
            <SlidersHorizontal /> {{ $t("metadata.action") }}
          </DropdownMenuItem>
          <DropdownMenuSeparator class="bg-border -mx-1 my-1 h-px" />
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
          <DropdownMenuItem :class="itemClass" @select="emit('download')">
            <Download /> {{ $t("transfers.downloadFolder") }}
          </DropdownMenuItem>
          <DropdownMenuSeparator class="bg-border -mx-1 my-1 h-px" />
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
        <DropdownMenuItem :class="itemClass" @select="emit('copyKey')">
          <ClipboardCopy /> {{ $t("share.copyKey") }}
        </DropdownMenuItem>
        <DropdownMenuItem :class="itemClass" @select="emit('copyUri')">
          <ClipboardCopy /> {{ $t("share.copyUri") }}
        </DropdownMenuItem>
        <DropdownMenuItem v-if="!entry.isPrefix" :class="itemClass" @select="emit('copyPublicUrl')">
          <Globe /> {{ $t("share.copyPublicUrl") }}
        </DropdownMenuItem>
        <DropdownMenuSeparator class="bg-border -mx-1 my-1 h-px" />
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
