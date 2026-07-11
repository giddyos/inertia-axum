import { defineConfig, devices } from '@playwright/test'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(here, '../../..')

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  use: {
    baseURL: 'http://127.0.0.1:3014',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'cargo run --locked -p axum-vue',
    cwd: repositoryRoot,
    env: {
      ...process.env,
      ADDR: '127.0.0.1:3014',
    },
    url: 'http://127.0.0.1:3014/todos',
    reuseExistingServer: false,
    timeout: 120_000,
  },
})

