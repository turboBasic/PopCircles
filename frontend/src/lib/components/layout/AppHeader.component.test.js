import { render } from 'vitest-browser-svelte';
import { expect, test, vi } from 'vitest';
import AppHeader from './AppHeader.svelte';

test('[themes] theme control reports its action and follows the updated value', async () => {
  const onTheme = vi.fn();
  const screen = await render(AppHeader, {
    scrollPosition: 0,
    headerCollapseProgress: 0,
    data: { world: [], countries: [] },
    themeLight: false,
    drawerOpen: false,
    headerElement: null,
    onHome: vi.fn(),
    onTheme,
    onOpenDrawer: vi.fn(),
  });
  const darkThemeButton = screen.getByRole('button', { name: 'Switch to light theme' });

  await expect.element(darkThemeButton).toHaveAttribute('aria-pressed', 'false');
  await darkThemeButton.click();

  expect(onTheme).toHaveBeenCalledOnce();
  await screen.rerender({ themeLight: true });

  const lightThemeButton = screen.getByRole('button', { name: 'Switch to dark theme' });
  await expect.element(lightThemeButton).toHaveAttribute('aria-pressed', 'true');
});
