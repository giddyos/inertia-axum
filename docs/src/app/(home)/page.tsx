import Link from 'next/link';
import { ArrowRight, Braces, CheckCircle2, Server } from 'lucide-react';

const frameworks = [
  ['Svelte', '/docs/frontend/svelte'],
  ['React', '/docs/frontend/react'],
  ['Vue', '/docs/frontend/vue'],
] as const;

export default function HomePage() {
  return (
    <main className="flex flex-1 flex-col">
      <section className="relative overflow-hidden border-b px-6 py-20 sm:py-28">
        <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_top_left,color-mix(in_oklab,var(--ia-rust)_18%,transparent),transparent_48%)]" />
        <div className="mx-auto max-w-5xl">
          <p className="mb-5 font-mono text-sm font-semibold uppercase tracking-[0.18em] text-fd-muted-foreground">
            Axum · Inertia.js v3 · Rust
          </p>
          <h1 className="max-w-4xl text-balance text-5xl font-semibold tracking-tight sm:text-7xl">
            Build modern monoliths with Axum.
          </h1>
          <p className="mt-7 max-w-2xl text-pretty text-lg leading-8 text-fd-muted-foreground sm:text-xl">
            Keep routing, data loading, validation, and middleware in Rust. Render
            Svelte, React, or Vue pages without maintaining a separate API.
          </p>
          <div className="mt-10 flex flex-col gap-3 sm:flex-row">
            <Link
              href="/docs/getting-started/quick-start"
              className="inline-flex min-h-11 items-center justify-center gap-2 rounded-full bg-fd-primary px-6 font-medium text-fd-primary-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-fd-ring"
            >
              Build your first page <ArrowRight className="size-4" aria-hidden="true" />
            </Link>
            <Link
              href="/docs"
              className="inline-flex min-h-11 items-center justify-center rounded-full border bg-fd-background px-6 font-medium hover:bg-fd-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-fd-ring"
            >
              Read the documentation
            </Link>
          </div>
        </div>
      </section>

      <section className="mx-auto grid w-full max-w-5xl gap-5 px-6 py-14 md:grid-cols-3">
        {[
          [Server, 'Axum-native', 'Use normal routes, extractors, state, Tower middleware, and responses.'],
          [Braces, 'Typed where it matters', 'Choose concise dynamic pages or derive typed page and prop contracts.'],
          [CheckCircle2, 'Protocol complete', 'Ship partial reloads, deferred data, validation, SSR, flash, and testing.'],
        ].map(([Icon, title, description]) => (
          <article key={String(title)} className="rounded-2xl border bg-fd-card p-6">
            <Icon className="mb-5 size-6 text-[var(--ia-rust)]" aria-hidden="true" />
            <h2 className="font-semibold">{String(title)}</h2>
            <p className="mt-2 text-sm leading-6 text-fd-muted-foreground">{String(description)}</p>
          </article>
        ))}
      </section>

      <section className="border-y bg-fd-card/40 px-6 py-14">
        <div className="mx-auto max-w-5xl">
          <div className="flex flex-col justify-between gap-6 md:flex-row md:items-end">
            <div>
              <p className="font-mono text-xs font-semibold uppercase tracking-[0.16em] text-fd-muted-foreground">
                Pick your client
              </p>
              <h2 className="mt-2 text-3xl font-semibold tracking-tight">Frontend setup in one click</h2>
            </div>
            <p className="max-w-md text-sm leading-6 text-fd-muted-foreground">
              Each guide includes the adapter packages, Vite entry point, page resolver,
              and production build commands.
            </p>
          </div>
          <div className="mt-8 grid gap-3 sm:grid-cols-3">
            {frameworks.map(([name, href]) => (
              <Link
                key={name}
                href={href}
                className="group flex min-h-16 items-center justify-between rounded-xl border bg-fd-background px-5 font-medium hover:border-[var(--ia-rust)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-fd-ring"
              >
                {name}
                <ArrowRight className="size-4 transition-transform group-hover:translate-x-1" aria-hidden="true" />
              </Link>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
