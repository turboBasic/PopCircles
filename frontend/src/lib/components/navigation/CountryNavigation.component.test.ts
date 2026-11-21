import { render } from 'vitest-browser-svelte';
import { expect, test, vi } from 'vitest';
import CountryNavigation from './CountryNavigation.svelte';
import { Provider as TooltipProvider } from '#lib/components/ui/tooltip';

import type { GalleryEntry } from '#lib/types/gallery';

const countries: GalleryEntry[] = [
  { name: 'Alpine', anchor: 'country-alpine', figures: [], missing: false },
  { name: 'Brazil', anchor: 'country-brazil', figures: [], missing: false },
];
const worldEntry: GalleryEntry = {
  name: 'World',
  anchor: 'world-result',
  figures: [],
  missing: false,
};

test('[navigation-and-history] search filters countries and updates available letter jumps', async () => {
  const screen = await render(
    CountryNavigation,
    {
      worldEntry,
      countries,
      selectedEntry: null,
      mobileMode: false,
      drawerOpen: false,
      desktopNavigationCollapsed: false,
      onSelect: vi.fn(),
      onCloseDrawer: vi.fn(),
      onDrawerClosed: vi.fn(),
      onDesktopCollapsedChange: vi.fn(),
    },
    { wrapper: TooltipProvider, wrapperProps: { delayDuration: 300 } },
  );
  const navigation = screen.getByRole('navigation', { name: 'Country navigation' });
  const search = navigation.getByRole('searchbox', { name: 'Search countries' });
  const alpineJump = navigation.getByRole('button', {
    name: 'Jump to countries starting with A',
  });
  const brazilJump = navigation.getByRole('button', {
    name: 'Jump to countries starting with B',
  });

  await search.fill('brazil');

  await expect.element(navigation.getByRole('link', { name: 'Brazil' })).toBeVisible();
  await expect.element(alpineJump).toBeDisabled();
  await expect.element(brazilJump).toBeEnabled();

  await search.fill('no match');

  await expect.element(navigation.getByText('No matching countries.')).toBeVisible();
  await expect.element(brazilJump).toBeDisabled();
});

test('[responsive-navigation] mobile drawer country links select without closing the dialog', async () => {
  const onSelect = vi.fn();
  const screen = await render(
    CountryNavigation,
    {
      worldEntry,
      countries,
      selectedEntry: null,
      mobileMode: true,
      drawerOpen: true,
      desktopNavigationCollapsed: false,
      onSelect,
      onCloseDrawer: vi.fn(),
      onDrawerClosed: vi.fn(),
      onDesktopCollapsedChange: vi.fn(),
    },
    { wrapper: TooltipProvider, wrapperProps: { delayDuration: 300 } },
  );
  const dialog = screen.getByRole('dialog', { name: 'Country navigation' });

  await expect.element(dialog).toBeVisible();
  await dialog.getByRole('link', { name: 'Brazil' }).click();

  expect(onSelect).toHaveBeenCalledWith(countries[1]);
  await expect.element(dialog).toBeVisible();
});
