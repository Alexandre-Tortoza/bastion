<template>
  <div class="flex h-screen flex-col overflow-hidden bg-bg text-text">
    <LayoutAppHeader />
    <main class="flex-1 min-h-0">
      <slot />
    </main>
  </div>
</template>
