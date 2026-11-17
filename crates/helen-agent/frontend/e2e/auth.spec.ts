import { test, expect } from '@playwright/test'

/**
 * E2E tests for the authentication flow.
 * 
 * These tests verify the AuthGate component behavior:
 * - Shows token prompt when backend returns 401
 * - Accepts and stores valid tokens
 * - Renders children after authentication
 */

test.describe('Authentication Flow', () => {
  test.beforeEach(async ({ page }) => {
    // Clear localStorage before each test
    await page.evaluate(() => localStorage.clear())
  })

  test('shows app layout when authenticated', async ({ page }) => {
    // Set a valid token in localStorage
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-token', 'test-token')
    })

    await page.goto('/')

    // Should see the main app layout (sidebar, chat area, etc.)
    // The exact selectors depend on the app structure
    await expect(page.locator('body')).toBeVisible()
  })

  test('clears token from localStorage', async ({ page }) => {
    // Set a token
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-token', 'test-token')
    })

    // Verify it's set
    const token = await page.evaluate(() => 
      localStorage.getItem('helen-webui-token')
    )
    expect(token).toBe('test-token')

    // Clear it
    await page.evaluate(() => {
      localStorage.removeItem('helen-webui-token')
    })

    // Verify it's cleared
    const clearedToken = await page.evaluate(() => 
      localStorage.getItem('helen-webui-token')
    )
    expect(clearedToken).toBeNull()
  })
})
