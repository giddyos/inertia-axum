# Svelte Frontend

Vite frontend for the Axum Inertia example.

From the repository root:

```sh
npm ci --prefix examples/axum-svelte/svelte-app
npm run build --prefix examples/axum-svelte/svelte-app
```

The production build writes assets and a Vite manifest to `../public/build`.
The Axum server reads that manifest to choose the script path and asset
version.
