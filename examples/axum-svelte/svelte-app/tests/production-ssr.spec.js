import { expect, test } from '@playwright/test'

test('production SSR hydrates and remains interactive', async ({ page }) => {
  const pageErrors = []
  const consoleErrors = []

  page.on('pageerror', error => pageErrors.push(error.message))
  page.on('console', message => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text())
    }
  })

  await page.goto('/todos')

  await expect(page.locator('#app')).toHaveAttribute(
    'data-server-rendered',
    'true',
  )
  await expect(page.getByText('Try automatic deferred props')).toBeVisible()
  await expect(page.getByText('1 remaining')).toBeVisible()
  await expect(page.getByText('1 total')).toBeVisible()

  await page.getByLabel('Todo title').fill('Verify SSR hydration')
  await page.getByRole('button', { name: 'Add todo' }).click()

  await expect(page.getByText('Verify SSR hydration')).toBeVisible()
  await expect(page.getByText('2 remaining')).toBeVisible()
  await expect(page.getByText('2 total')).toBeVisible()

  expect(pageErrors).toEqual([])
  expect(consoleErrors).toEqual([])
})
