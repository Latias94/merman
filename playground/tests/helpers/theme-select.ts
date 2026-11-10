import { expect, type Locator, type Page } from "@playwright/test";

export async function chooseThemeOption(page: Page, trigger: Locator, value: string) {
  await trigger.click();
  const listbox = page.getByRole("listbox");
  await expect(listbox).toBeVisible();
  await listbox.locator(`[role="option"][data-value=${JSON.stringify(value)}]`).click();
  await expect(listbox).toBeHidden();
  await expect(trigger).toHaveAttribute("data-value", value);
}
