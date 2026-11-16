import { expect, test } from '@playwright/test';

const countries = Array.from({ length: 80 }, (_, index) => ({
  name: `Country ${String(index + 1).padStart(2, '0')}`,
  anchor: `country-${index + 1}`,
  figures:
    index === 41
      ? [
          {
            map: 'country-42.svg',
            document: 'country-42.json',
            country_id: 'CC',
            alt: 'Country 42 map',
          },
        ]
      : [],
  missing: false,
}));
const world = [{ map: 'world.svg', document: 'world.json', country_id: null, alt: 'World map' }];

test.beforeEach(async ({ page }) => {
  await page.route('**/gallery-data.json', (route) =>
    route.fulfill({ json: { world, countries } }),
  );
  await page.route('**/world.svg', (route) =>
    route.fulfill({
      contentType: 'image/svg+xml',
      body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 6" />',
    }),
  );
  await page.route('**/country-42.svg', (route) =>
    route.fulfill({
      contentType: 'image/svg+xml',
      body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 6" />',
    }),
  );
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();
});

test('searches country navigation and selects a country', async ({ page }) => {
  const navigation = page.getByRole('navigation', { name: 'Map navigation' });
  await navigation.getByRole('searchbox', { name: 'Search maps' }).fill('country 42');
  await expect(navigation.getByRole('link')).toHaveCount(2);
  await page
    .getByRole('navigation', { name: 'Map navigation' })
    .getByRole('link', { name: 'Country 42' })
    .click();

  await expect(page).toHaveURL(/#country-42$/);
  await expect(page.locator('#country-42')).toBeVisible();
  await expect(page.locator('#country-41')).toHaveCount(0);
  const mapLink = page.getByRole('link', { name: 'Country 42 map' });
  await expect(mapLink).toHaveAttribute('href', '#country-42');
  await mapLink.click();
  await expect(page).toHaveURL(/#country-42$/);
});

test('results scroll inside the viewport without scrolling the document', async ({ page }) => {
  const metrics = await page.getByRole('region', { name: 'Map results' }).evaluate((results) => {
    results.scrollTop = results.scrollHeight;
    return {
      scrollTop: results.scrollTop,
      bodyHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
    };
  });

  expect(metrics.scrollTop).toBeGreaterThan(0);
  expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight);
});

test('country navigation is keyboard operable and returns focus after Escape', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const trigger = page.getByRole('button', { name: 'Open map navigation' });
  await trigger.focus();
  await page.keyboard.press('Enter');

  const dialog = page.getByRole('dialog', { name: 'Browse maps' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('searchbox', { name: 'Search maps' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('button', { name: 'Open map navigation' })).toBeFocused();
});

test('interactive controls have accessible names', async ({ page }) => {
  const [buttons, namedButtons, links, namedLinks] = await Promise.all([
    page.locator('button:visible').count(),
    page.getByRole('button', { name: /.+/ }).count(),
    page.locator('a[href]:visible').count(),
    page.getByRole('link', { name: /.+/ }).count(),
  ]);

  expect(namedButtons).toBe(buttons);
  expect(namedLinks).toBe(links);
  await expect(page.getByRole('searchbox', { name: 'Search maps' })).toBeVisible();
});

test('animated logo keeps its brand text and selects valid sprite symbols', async ({ page }) => {
  const logo = page.getByRole('link', { name: 'PopCircles home' });
  const logoData = await logo.evaluate(async (link) => {
    const brandText = link.querySelector('.brand-text');
    const uses = [...link.querySelectorAll('use')];
    const urls = uses.map((use) => new URL(use.getAttribute('href'), location.href));
    const spriteUrl = new URL(urls[0]);
    spriteUrl.hash = '';
    const document = new DOMParser().parseFromString(
      await fetch(spriteUrl).then((r) => r.text()),
      'image/svg+xml',
    );
    return {
      text: brandText?.textContent.replace(/\s/g, ''),
      fallbackVisibility: getComputedStyle(link.querySelector('.globe-fallback')).visibility,
      label: link.getAttribute('aria-label'),
      symbolIds: [...document.querySelectorAll('symbol')].map((symbol) => symbol.id),
      references: urls.map((url) => ({
        id: url.hash.slice(1),
        valid: Boolean(document.getElementById(url.hash.slice(1))),
      })),
    };
  });
  expect(logoData.text).toBe('PopCircles');
  expect(logoData.fallbackVisibility).toBe('hidden');
  expect(logoData.label).toBe('PopCircles home');
  expect(logoData.symbolIds).toEqual(
    Array.from({ length: 72 }, (_, index) => `frame-${String(index + 1).padStart(2, '0')}`),
  );
  expect(logoData.references).toEqual([
    { id: 'frame-01', valid: true },
    { id: 'frame-01', valid: true },
  ]);
  await expect(logo.locator('svg[aria-hidden="true"]')).toHaveCount(2);

  const results = page.getByRole('region', { name: 'Map results' });
  for (const [scrollTop, symbol] of [
    [4, 'frame-02'],
    [239, 'frame-72'],
    [240, 'frame-01'],
  ]) {
    await results.evaluate((region, top) => {
      region.scrollTop = top;
      region.dispatchEvent(new Event('scroll'));
    }, scrollTop);
    await expect
      .poll(() => logo.locator('use').first().getAttribute('href'))
      .toMatch(new RegExp(`#${symbol}$`));
  }

  for (const viewport of [
    { width: 1280, height: 800 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    const alignment = await logo.evaluate((link) => {
      const brandText = link.querySelector('.brand-text');
      const slots = [...brandText.querySelectorAll('.letter-slot')].map((letter) =>
        letter.getBoundingClientRect(),
      );
      const slot = brandText.querySelector('.globe-slot').getBoundingClientRect();
      const globe = brandText.querySelector('.globe-letter').getBoundingClientRect();
      const fontSize = Number.parseFloat(getComputedStyle(brandText).fontSize);
      const slotGap = Number.parseFloat(getComputedStyle(brandText).columnGap);
      return {
        slotCenter: slot.left + slot.width / 2,
        slotMiddle: slot.top + slot.height / 2,
        globeCenter: globe.left + globe.width / 2,
        globeMiddle: globe.top + globe.height / 2,
        globeScale: globe.width / slot.width,
        inlineScale: globe.style.scale,
        slotWidths: slots.map(({ width }) => width),
        slotGaps: slots.slice(1).map((letter, index) => letter.left - slots[index].right),
        expectedSlotGaps: slots
          .slice(1)
          .map((_, index) => slotGap + fontSize * (index === 0 ? -0.08 : index === 1 ? 0.1 : 0)),
      };
    });
    expect(Math.abs(alignment.slotCenter - alignment.globeCenter)).toBeLessThan(1);
    expect(Math.abs(alignment.slotMiddle - alignment.globeMiddle)).toBeLessThan(1);
    expect(alignment.inlineScale).toBe('1.07');
    expect(alignment.globeScale).toBeCloseTo(1.07, 2);
    expect(
      alignment.slotWidths.every((width) => Math.abs(width - alignment.slotWidths[0]) < 1),
    ).toBe(true);
    expect(
      alignment.slotGaps.every(
        (gap, index) => Math.abs(gap - alignment.expectedSlotGaps[index]) < 1,
      ),
    ).toBe(true);
  }
});

test('responsive layout, map ratio, and theme styles keep their expected values', async ({
  page,
}) => {
  const visualProperties = () =>
    page.evaluate(() => ({
      background: getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
      colorScheme: getComputedStyle(document.documentElement).colorScheme,
      documentHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
      columns: getComputedStyle(document.querySelector('.explorer')).gridTemplateColumns,
      mapAspectRatio: Number.parseFloat(
        getComputedStyle(document.querySelector('.map-frame')).aspectRatio,
      ),
    }));

  let visual = await visualProperties();
  expect(visual.background).toBe('#0b1020');
  expect(visual.colorScheme).toBe('dark');
  expect(visual.documentHeight).toBeLessThanOrEqual(visual.viewportHeight);
  expect(await page.getByRole('region', { name: 'Map results' }).isVisible()).toBe(true);
  expect(visual.columns.split(' ')).toHaveLength(2);
  expect(visual.mapAspectRatio).toBeCloseTo(11 / 6);
  await expect(page).toHaveScreenshot('gallery-desktop-dark.png');

  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  visual = await visualProperties();
  expect(visual.background).toBe('#f5f7fb');
  expect(visual.colorScheme).toBe('light');
  await expect(page).toHaveScreenshot('gallery-desktop-light.png');

  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  visual = await visualProperties();
  expect(visual.background).toBe('#0b1020');
  expect(visual.colorScheme).toBe('dark');
  expect(visual.documentHeight).toBeLessThanOrEqual(visual.viewportHeight);
  expect(visual.columns.split(' ')).toHaveLength(1);
  await expect(page.getByRole('navigation', { name: 'Map navigation' })).toBeHidden();
  expect(await page.getByRole('button', { name: 'Open map navigation' }).isVisible()).toBe(true);
  await expect(page).toHaveScreenshot('gallery-mobile-dark.png');

  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  visual = await visualProperties();
  expect(visual.background).toBe('#f5f7fb');
  expect(visual.colorScheme).toBe('light');
  await expect(page).toHaveScreenshot('gallery-mobile-light.png');
});
