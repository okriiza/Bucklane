<script setup lang="ts">
import { inlineTokens, type NotesSection } from "@/utils/changelog";

defineProps<{ sections: NotesSection[] }>();
</script>

<!-- CHANGELOG notes rendered as text nodes; nothing is interpreted as HTML. -->
<template>
  <div class="grid gap-4 text-[13px] leading-relaxed">
    <section v-for="(section, i) in sections" :key="i">
      <h3
        v-if="section.title"
        class="text-muted-foreground mb-1.5 text-[11px] font-medium tracking-wide uppercase"
      >
        {{ section.title }}
      </h3>
      <ul class="grid list-disc gap-1.5 pl-4">
        <li v-for="(item, j) in section.items" :key="j">
          <template v-for="(token, k) in inlineTokens(item)" :key="k">
            <strong v-if="token.kind === 'strong'" class="font-semibold" v-text="token.text" />
            <code
              v-else-if="token.kind === 'code'"
              class="bg-muted rounded px-1 font-mono text-xs"
              v-text="token.text"
            />
            <template v-else>{{ token.text }}</template>
          </template>
        </li>
      </ul>
    </section>
  </div>
</template>
