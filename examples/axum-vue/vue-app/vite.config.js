import inertia from '@inertiajs/vite'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

export default defineConfig(({ isSsrBuild }) => ({
  plugins: [vue(), inertia()],
  build: {
    outDir: isSsrBuild ? 'dist/ssr' : '../public/build',
    emptyOutDir: true,
    manifest: !isSsrBuild,
    rolldownOptions: isSsrBuild
      ? {}
      : {
          input: 'src/app.js',
        },
  },
}))
