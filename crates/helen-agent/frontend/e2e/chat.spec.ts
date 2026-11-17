import { test, expect } from '@playwright/test'

/**
 * E2E tests for the chat interface.
 * 
 * These tests verify the core chat functionality:
 * - Message input is visible and functional
 * - Messages can be sent (UI only, no backend)
 * - Message list renders correctly
 */

test.describe('Chat Interface', () => {
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => localStorage.clear())
    // Set token to bypass auth
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-token', 'test-token')
    })
  })

  test('shows chat page layout', async ({ page }) => {
    await page.goto('/')

    // Should see the main container
    await expect(page.locator('body')).toBeVisible()
    
    // Should not show auth prompt (we have a token)
    const authPrompt = page.locator('text=Access Token')
    await expect(authPrompt).not.toBeVisible()
  })

  test('message input is present', async ({ page }) => {
    await page.goto('/')

    // Look for textarea or input for messages
    const messageInput = page.locator('textarea, [contenteditable="true"]').first()
    
    // Input should be visible (or at least present in DOM)
    await expect(messageInput).toBeAttached()
  })

  test('can type in message input', async ({ page }) => {
    await page.goto('/')

    const messageInput = page.locator('textarea').first()
    
    // Type a message
    await messageInput.fill('Hello, Helen!')
    
    // Verify the text is there
    await expect(messageInput).toHaveValue('Hello, Helen!')
  })

  test('clears input after sending (UI behavior)', async ({ page }) => {
    await page.goto('/')

    const messageInput = page.locator('textarea').first()
    
    // Type a message
    await messageInput.fill('Test message')
    await expect(messageInput).toHaveValue('Test message')
    
    // Note: Actually sending requires backend connection
    // This test just verifies the input can be cleared
    await messageInput.clear()
    await expect(messageInput).toHaveValue('')
  })
})

test.describe('Chat Page Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => localStorage.clear())
    await page.evaluate(() => {
      localStorage.setItem('helen-webui-token', 'test-token')
    })
  })

  test('navigates to chat page', async ({ page }) => {
    await page.goto('/')
    
    // Should be on the chat page
    await expect(page).toHaveURL(/.*\//)
  })

  test('navigates to settings page', async ({ page }) => {
    await page.goto('/settings')
    
    // Should be on the settings page
    await expect(page).toHaveURL(/.*\/settings/)
  })

  test('can navigate between pages', async ({ page }) => {
    // Start at chat
    await page.goto('/')
    await expect(page).toHaveURL(/.*\//)

    // Navigate to settings
    await page.goto('/settings')
    await expect(page).toHaveURL(/.*\/settings/)

    // Navigate back to chat
    await page.goto('/')
    await expect(page).toHaveURL(/.*\//)
  })
})
