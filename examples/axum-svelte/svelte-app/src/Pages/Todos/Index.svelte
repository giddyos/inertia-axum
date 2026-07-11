<script>
  import { Deferred, router } from '@inertiajs/svelte'

  let { todos = [], stats, errors = {} } = $props()
  let title = $state('')

  function submit(event) {
    event.preventDefault()
    router.post('/todos', { title }, {
      onSuccess: () => {
        title = ''
      },
    })
  }

  function refreshSummary() {
    router.reload({ only: ['stats'] })
  }
</script>

<main>
  <header class="masthead">
    <p class="eyebrow">Inertia Axum</p>
    <h1>Todos</h1>
    <p>Axum state, typed props, redirect validation, and deferred data.</p>
  </header>

  <form onsubmit={submit}>
    <label for="title">Todo title</label>
    <div class="form-row">
      <input id="title" bind:value={title} aria-invalid={errors.title ? 'true' : undefined}>
      <button type="submit">Add todo</button>
    </div>
    {#if errors.title}
      <p class="error">{errors.title}</p>
    {/if}
  </form>

  <ul>
    {#each todos as todo}
      <li>{todo.title}</li>
    {/each}
  </ul>

  <section class="summary" aria-label="Todo summary">
    <Deferred data="stats">
      {#snippet fallback()}
        <p>Loading summary…</p>
      {/snippet}

      <strong>{stats.remaining} remaining</strong>
      <span>{stats.total} total</span>
    </Deferred>
    <button type="button" onclick={refreshSummary}>Refresh summary</button>
  </section>
</main>
