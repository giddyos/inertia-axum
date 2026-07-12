<script lang="ts">
  import { Deferred } from '@inertiajs/svelte'

  type Stats = {
    projects: number
    tasks: number
  }

  let {
    greeting = 'Hello',
    stats,
  }: {
    greeting?: string
    stats?: Stats
  } = $props()
</script>

<main>
  <h1>{greeting} from inertia-axum</h1>

  <Deferred data="stats">
    {#snippet fallback()}
      <p>Loading stats…</p>
    {/snippet}

    {#if stats}
      <p>{stats.projects} projects · {stats.tasks} tasks</p>
    {/if}
  </Deferred>
</main>
