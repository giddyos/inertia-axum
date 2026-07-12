import inertia from '@inertiajs/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig(({ isSsrBuild }) => ({
  plugins: [react(), inertia()],
  build: {
    outDir: isSsrBuild ? 'dist/ssr' : '../public/build',
    emptyOutDir: true,
    manifest: !isSsrBuild,
    rolldownOptions: isSsrBuild
      ? {}
      : {
          input: 'src/app.jsx',
        },
  },
}))
