import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig(({ isSsrBuild }) => ({
  plugins: [svelte({ prebundleSvelteLibraries: true }), inertia()],
  build: {
    outDir: isSsrBuild ? 'dist/ssr' : '../public/build',
    emptyOutDir: true,
    manifest: !isSsrBuild,
    rolldownOptions: isSsrBuild ? {} : {
      input: 'src/app.js',
    },
  },
}))
import inertia from '@inertiajs/vite'
