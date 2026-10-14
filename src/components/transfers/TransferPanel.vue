<template>
  <section class="flex flex-col border-t">
    <header class="flex h-9 items-center border-b px-4 text-xs font-medium">
      {{ $t("transfers.title") }}
    </header>
    <div class="text-muted-foreground flex flex-1 items-center justify-center text-sm">
      {{ $t("transfers.empty") }}
    </div>
  </section>
</template>
