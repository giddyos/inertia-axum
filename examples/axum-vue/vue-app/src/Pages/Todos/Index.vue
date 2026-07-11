<script setup>
import { Deferred, router } from '@inertiajs/vue3'
import { ref } from 'vue'

const props = defineProps({
  todos: {
    type: Array,
    default: () => [],
  },
  stats: {
    type: Object,
    default: undefined,
  },
  errors: {
    type: Object,
    default: () => ({}),
  },
})

const title = ref('')

function submit() {
  router.post(
    '/todos',
    { title: title.value },
    {
      onSuccess: () => {
        title.value = ''
      },
    },
  )
}

function refreshSummary() {
  router.reload({ only: ['stats'] })
}
</script>

<template>
  <main>
    <header class="masthead">
      <p class="eyebrow">Inertia Axum</p>
      <h1>Todos</h1>
      <p>Axum state, typed props, redirect validation, and deferred data.</p>
    </header>

    <form @submit.prevent="submit">
      <label for="title">Todo title</label>
      <div class="form-row">
        <input
          id="title"
          v-model="title"
          :aria-invalid="props.errors.title ? 'true' : undefined"
        >
        <button type="submit">Add todo</button>
      </div>

      <p v-if="props.errors.title" class="error">
        {{ props.errors.title }}
      </p>
    </form>

    <ul>
      <li v-for="todo in props.todos" :key="todo.id">
        {{ todo.title }}
      </li>
    </ul>

    <section class="summary" aria-label="Todo summary">
      <Deferred data="stats">
        <template #fallback>
          <p>Loading summary…</p>
        </template>

        <strong>{{ props.stats.remaining }} remaining</strong>
        <span>{{ props.stats.total }} total</span>
      </Deferred>

      <button type="button" @click="refreshSummary">
        Refresh summary
      </button>
    </section>
  </main>
</template>
