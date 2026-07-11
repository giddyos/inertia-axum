import { Deferred, router } from '@inertiajs/react'
import { useState } from 'react'

export default function TodosIndex({ todos = [], stats, errors = {} }) {
  const [title, setTitle] = useState('')

  function submit(event) {
    event.preventDefault()

    router.post(
      '/todos',
      { title },
      {
        onSuccess: () => setTitle(''),
      },
    )
  }

  function refreshSummary() {
    router.reload({ only: ['stats'] })
  }

  return (
    <main>
      <header className="masthead">
        <p className="eyebrow">Inertia Axum</p>
        <h1>Todos</h1>
        <p>Axum state, typed props, redirect validation, and deferred data.</p>
      </header>

      <form onSubmit={submit}>
        <label htmlFor="title">Todo title</label>
        <div className="form-row">
          <input
            id="title"
            value={title}
            aria-invalid={errors.title ? 'true' : undefined}
            onChange={(event) => setTitle(event.target.value)}
          />
          <button type="submit">Add todo</button>
        </div>

        {errors.title && <p className="error">{errors.title}</p>}
      </form>

      <ul>
        {todos.map((todo) => (
          <li key={todo.id}>{todo.title}</li>
        ))}
      </ul>

      <section className="summary" aria-label="Todo summary">
        <Deferred data="stats" fallback={<p>Loading summary…</p>}>
          {stats && (
            <>
              <strong>{stats.remaining} remaining</strong>
              <span>{stats.total} total</span>
            </>
          )}
        </Deferred>

        <button type="button" onClick={refreshSummary}>
          Refresh summary
        </button>
      </section>
    </main>
  )
}
