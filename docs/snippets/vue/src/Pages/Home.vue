<script setup lang="ts">
import { Deferred } from '@inertiajs/vue3'

const props = withDefaults(
  defineProps<{
    greeting?: string
    stats?: {
      projects: number
      tasks: number
    }
  }>(),
  {
    greeting: 'Hello',
  },
)
</script>

<template>
  <main>
    <h1>{{ props.greeting }} from inertia-axum</h1>

    <Deferred data="stats">
      <template #fallback>
        <p>Loading stats…</p>
      </template>

      <p v-if="props.stats">
        {{ props.stats.projects }} projects ·
        {{ props.stats.tasks }} tasks
      </p>
    </Deferred>
  </main>
</template>
