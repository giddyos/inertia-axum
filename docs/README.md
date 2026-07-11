# inertia-axum documentation application

This directory contains the canonical Fumadocs site. User-facing content lives in
`content/docs`; `internal` contains maintainer-only inventory and archived records.

## Local development

Use Node.js 22 or newer and pnpm 11.9.0:

```bash
corepack pnpm install --frozen-lockfile
corepack pnpm dev
```

The site runs at `http://localhost:3000`. Set `NEXT_PUBLIC_SITE_URL` to the deployed
origin when building production metadata, canonical links, and the sitemap.

## Required checks

```bash
pnpm lint
pnpm typecheck
pnpm docs:check-links
pnpm build
```

Every content directory has an explicit `meta.json`; add new pages there rather than
relying on alphabetical ordering. `docs:check-links` also verifies frontmatter,
metadata coverage, duplicate routes, internal links, and heading anchors.
