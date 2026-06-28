<script setup lang="ts">
const route = useRoute()

const links = [
  { label: 'Dashboard', to: '/' },
  { label: 'Escrever', to: '/write' },
  { label: 'Wiki', to: '/wiki' },
  { label: 'Arquivos', to: '/files' },
  { label: 'Chat', to: '/chat' }
]

function isActive(to: string) {
  return route.path === to || route.path.startsWith(`${to}/`)
}
</script>

<template>
  <UHeader class="border-b border-primary-800/80 bg-bg/95 backdrop-blur">
    <template #left>
      <NuxtLink
        to="/write"
        class="text-lg font-semibold tracking-tight text-text hover:text-primary-300 transition-colors"
      >
        Bastion
      </NuxtLink>
      <nav class="hidden md:flex items-center gap-4 ml-6 text-sm">
        <NuxtLink
          v-for="link in links"
          :key="link.to"
          :to="link.to"
          class="transition-colors hover:text-primary-300"
          :class="isActive(link.to) ? 'text-primary-300 underline decoration-primary-500 underline-offset-8' : 'text-text/70'"
        >{{ link.label }}</NuxtLink>
      </nav>
    </template>
    <template #right>
      <AppButton
        to="/settings"
        icon="i-lucide-settings"
        variant="ghost"
        color="neutral"
        aria-label="Configurações"
        :class="isActive('/settings') ? 'text-primary-300' : 'text-text/70 hover:text-primary-300'"
      />
    </template>
  </UHeader>
</template>
