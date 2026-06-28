<script setup lang="ts">
import type { Decision } from '~/types/bastion'

defineProps<{
  decision: Decision
  compact?: boolean
}>()

const emit = defineEmits<{
  open: [decision: Decision]
}>()

const statusColor: Record<string, string> = {
  proposed: 'blue',
  accepted: 'green',
  superseded: 'gray',
  rejected: 'red',
}
</script>

<template>
  <UCard
    class="cursor-pointer hover:border-indigo-500 transition-colors"
    @click="emit('open', decision)"
  >
    <div class="flex items-start justify-between gap-3">
      <div class="flex-1 min-w-0">
        <div class="font-medium text-gray-100 truncate">
          {{ decision.title }}
        </div>
        <div v-if="decision.date" class="text-xs text-gray-500 mt-0.5">
          {{ decision.date }}
        </div>
        <p
          v-if="!compact && decision.context_excerpt"
          class="text-sm text-gray-400 mt-2 line-clamp-2"
        >
          {{ decision.context_excerpt }}
        </p>
      </div>
      <UBadge
        :color="(statusColor[decision.decision_status] as any) ?? 'gray'"
        variant="subtle"
        class="flex-shrink-0"
      >
        {{ decision.decision_status }}
      </UBadge>
    </div>
  </UCard>
</template>
