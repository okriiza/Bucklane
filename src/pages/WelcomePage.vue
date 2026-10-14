<template>
  <div class="flex h-full items-center justify-center p-8">
    <div class="max-w-md text-center">
      <h1 class="text-xl font-semibold tracking-tight">{{ $t("welcome.title") }}</h1>
      <p class="text-fg-muted mt-2 text-sm leading-relaxed">{{ $t("welcome.description") }}</p>
      <button
        type="button"
        disabled
        class="bg-accent mt-6 rounded-md px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
        :title="$t('welcome.comingSoon')"
      >
        {{ $t("welcome.addConnection") }}
      </button>
    </div>
  </div>
</template>
