export const site = {
  name: 'inertia-axum',
  description:
    'Build server-driven React, Svelte, or Vue applications with Axum and Inertia.js.',
  repository: 'https://github.com/giddyos/inertia-axum',
  crates: 'https://crates.io/crates/inertia-axum',
  rustdoc: 'https://docs.rs/inertia-axum/latest/inertia_axum/',
  branch: 'main',
  docsRoute: '/docs',
  docsImageRoute: '/og/docs',
} as const;

export const siteUrl = new URL(
  process.env.NEXT_PUBLIC_SITE_URL ?? 'http://localhost:3000',
);
