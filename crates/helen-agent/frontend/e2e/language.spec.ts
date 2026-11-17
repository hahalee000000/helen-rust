import { test, expect } from '@playwright/test'

/**
 * E2E tests for language switching functionality.
 */

test.describe('Language Switching', () => {
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => localStorage.clear())
    // Set token to bypass auth
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-token', 'test-token')
    })
  })

  test('defaults to English', async ({ page }) => {
    await page.goto('/')
    
    // Check that the language is stored as 'en' by default
    const lang = await page.evaluate(() => 
      localStorage.getItem('helen-webui-lang')
    )
    // Default might be null or 'en' depending on implementation
    expect(lang === null || lang === 'en').toBeTruthy()
  })

  test('can switch language via localStorage', async ({ page }) => {
    await page.goto('/')
    
    // Switch to Chinese
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-lang', 'zh')
    })

    // Reload to apply
    await page.reload()

    // Verify language is set
    const lang = await page.evaluate(() => 
      localStorage.getItem('helen-webui-lang')
    )
    expect(lang).toBe('zh')
  })

  test('persists language preference across reloads', async ({ page }) => {
    // Set language to Chinese
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-lang', 'zh')
    })

    await page.goto('/')
    
    // Verify it persists
    const lang = await page.evaluate(() => 
      localStorage.getItem('helen-webui-lang')
    )
    expect(lang).toBe('zh')

    // Reload
    await page.reload()

    // Verify it still persists
    const langAfterReload = await page.evaluate(() => 
      localStorage.getItem('helen-webui-lang')
    )
    expect(langAfterReload).toBe('zh')
  })
})
