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
  await expect(navigation.getByRole('link', { name: 'All countries' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expect(navigation.getByRole('link', { name: 'World' })).toHaveAttribute(
    'href',
    '#world-result',
  );
  await expect(navigation.getByText('Matching: 80')).toBeVisible();
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
  await expect(navigation.getByRole('link')).toHaveCount(3);
  await expect(navigation.getByText('Matching: 1')).toBeVisible();
  await expect(navigation.getByRole('heading', { name: 'C', exact: true })).toHaveCount(0);
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('no result');
  await expect(navigation.getByText('No matching countries.')).toBeVisible();
  await navigation.getByRole('button', { name: 'Clear search' }).click();
  await expect(navigation.getByRole('searchbox', { name: 'Search countries' })).toHaveValue('');
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
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
  await navigation.getByRole('link', { name: 'All countries' }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(navigation.getByRole('link', { name: 'All countries' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expect(navigation.getByRole('link', { name: 'Country 42' })).not.toHaveAttribute(
    'aria-current',
    'page',
  );
});

test('scrolls the selected country into view in the navigation list', async ({ page }) => {
  await page.goto('/#country-80');
  const navigation = page.getByRole('navigation', { name: 'Map navigation' });
  const selectedLink = navigation.getByRole('link', { name: 'Country 80' });
  await expect(selectedLink).toHaveAttribute('aria-current', 'page');
  await expect(selectedLink).toBeInViewport();
  await expect
    .poll(() => navigation.locator('.map-links').evaluate((list) => list.scrollTop))
    .toBeGreaterThan(0);
});

test('desktop navigation collapses to a rail without overlapping results', async ({ page }) => {
  for (const width of [1024, 1440]) {
    await page.setViewportSize({ width, height: 768 });
    const drawer = page.getByRole('dialog', { name: 'Browse maps' });
    await expect(drawer).toBeVisible();
    await expect(page.getByRole('navigation', { name: 'Map navigation' })).toHaveCount(1);
    const workspaceBefore = await page.getByRole('region', { name: 'Map results' }).boundingBox();
    const drawerBounds = await drawer.boundingBox();
    expect(drawerBounds.x).toBeGreaterThanOrEqual(0);
    expect(drawerBounds.width).toBeGreaterThanOrEqual(300);
    expect(workspaceBefore.x).toBeGreaterThan(drawerBounds.x + drawerBounds.width);
    await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
    const rail = await drawer.boundingBox();
    const workspaceExpanded = await page.getByRole('region', { name: 'Map results' }).boundingBox();
    expect(rail.width).toBeGreaterThanOrEqual(56);
    expect(rail.width).toBeLessThanOrEqual(64);
    expect(workspaceExpanded.width).toBeGreaterThan(workspaceBefore.width);
    expect(workspaceExpanded.x).toBeGreaterThan(rail.x + rail.width);
    await page.getByRole('button', { name: 'Search countries' }).click();
    await expect(drawer.getByRole('searchbox', { name: 'Search countries' })).toBeFocused();
    const workspaceAfterExpand = await page
      .getByRole('region', { name: 'Map results' })
      .boundingBox();
    expect(workspaceAfterExpand.width).toBeLessThan(workspaceExpanded.width);
    const list = drawer.locator('.map-links');
    await list.evaluate((element) => (element.scrollTop = element.scrollHeight));
    const listScrollTop = await list.evaluate((element) => element.scrollTop);
    await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
    await page.getByRole('button', { name: 'Expand maps navigation' }).click();
    expect(await list.evaluate((element) => element.scrollTop)).toBe(listScrollTop);
    await page.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
    await drawer
      .getByRole('navigation', { name: 'Map navigation' })
      .getByRole('link', { name: 'Country 42', exact: true })
      .click();
    await expect(page).toHaveURL(/#country-42$/);
    await page.getByRole('button', { name: 'Expand maps navigation' }).click();
    await expect(
      drawer
        .getByRole('navigation', { name: 'Map navigation' })
        .getByRole('link', { name: 'Country 42', exact: true }),
    ).toHaveAttribute('aria-current', 'page');
    await expect(page.getByRole('searchbox', { name: 'Search countries' })).toHaveValue(
      'country 42',
    );
    if (width === 1024) await page.goto('/');
  }
});

test('navigation-only transitions do not request map assets', async ({ page }) => {
  const mapRequests = () =>
    page.evaluate(() =>
      performance
        .getEntriesByType('resource')
        .map((resource) => resource.name)
        .filter((url) => /\.(svg|json)(?:$|\?)/.test(url)),
    );
  const initialRequests = await mapRequests();

  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await page.getByRole('button', { name: 'Search countries' }).click();
  await page.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await page.getByRole('button', { name: 'Expand maps navigation' }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Open maps navigation' }).click();
  await page.getByRole('button', { name: 'Close map navigation' }).click();

  expect(await mapRequests()).toEqual(initialRequests);
});

test('mobile navigation remains modal and closes on resize', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();
  const drawer = page.getByRole('dialog', { name: 'Browse maps' });
  await expect(drawer).toBeHidden();
  await page.getByRole('button', { name: 'Open maps navigation' }).click();
  await expect(drawer).toBeVisible();
  await expect.poll(() => drawer.evaluate((element) => element.matches(':modal'))).toBe(true);
  expect(await page.getByRole('region', { name: 'Map results' }).boundingBox()).not.toBeNull();
  const mobileWorkspace = await page.getByRole('region', { name: 'Map results' }).boundingBox();
  await drawer.getByRole('button', { name: 'Close map navigation' }).click();
  await expect(drawer).toBeHidden();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  expect(await page.getByRole('region', { name: 'Map results' }).boundingBox()).toEqual(
    mobileWorkspace,
  );
  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(drawer).toBeVisible();
  await expect.poll(() => drawer.evaluate((element) => element.matches(':modal'))).toBe(false);
  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  const mobileResults = await page.getByRole('region', { name: 'Map results' }).boundingBox();
  expect(mobileResults.x).toBeLessThan(4);
  expect(mobileResults.width).toBeGreaterThan(350);
  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(drawer).toBeVisible();
  await expect.poll(() => drawer.evaluate((element) => element.matches(':modal'))).toBe(false);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(drawer).toBeHidden();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
});

test('direct links, Back and Forward restore country selection and reset results', async ({
  page,
}) => {
  await page.goto('/#country-42');
  const drawer = page.getByRole('dialog', { name: 'Browse maps' });
  const navigation = page.getByRole('navigation', { name: 'Map navigation' });
  await expect(navigation.getByRole('link', { name: 'Country 42' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  const results = page.getByRole('region', { name: 'Map results' });
  await results.evaluate((region) => (region.scrollTop = 200));
  await navigation.getByRole('link', { name: 'Country 01' }).click();
  await expect(page).toHaveURL(/#country-1$/);
  await expect.poll(() => results.evaluate((region) => region.scrollTop)).toBe(0);
  await expect(navigation.getByRole('link', { name: 'Country 01' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.goBack();
  await expect(page).toHaveURL(/#country-42$/);
  await expect(navigation.getByRole('link', { name: 'Country 42' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.goForward();
  await expect(page).toHaveURL(/#country-1$/);
  await expect(navigation.getByRole('link', { name: 'Country 01' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expect(drawer).toBeVisible();
});

test('drawer presentation fits each viewport, theme, and motion preference', async ({ page }) => {
  const viewports = [
    { width: 1440, height: 900 },
    { width: 1024, height: 768 },
    { width: 390, height: 844 },
    { width: 320, height: 844 },
  ];
  const drawer = page.getByRole('dialog', { name: 'Browse maps' });

  for (const viewport of viewports) {
    await page.setViewportSize(viewport);
    await page.reload();
    await expect(
      page.getByRole('heading', { name: 'One circle, half the population.' }),
    ).toBeVisible();
    const workspace = page.getByRole('region', { name: 'Map results' });
    const workspaceBounds = await workspace.boundingBox();
    if (viewport.width > 1023) {
      const navBounds = await drawer.boundingBox();
      expect(navBounds.x + navBounds.width).toBeLessThanOrEqual(workspaceBounds.x);
    } else {
      await page.getByRole('button', { name: 'Open maps navigation' }).click();
    }
    await expect(drawer).toBeVisible();
    if (viewport.width <= 1023) {
      await expect
        .poll(() => drawer.evaluate((element) => getComputedStyle(element).transform))
        .toBe('matrix(1, 0, 0, 1, 0, 0)');
    }
    const bounds = await drawer.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.y).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width);
    expect(bounds.y + bounds.height).toBeLessThanOrEqual(viewport.height);
    expect(await workspace.boundingBox()).toEqual(workspaceBounds);
    const transitionDurations = await drawer.evaluate((element) =>
      getComputedStyle(element)
        .transitionDuration.split(',')
        .map((duration) => Number.parseFloat(duration)),
    );
    if (viewport.width <= 1023)
      expect(transitionDurations.some((duration) => duration > 0)).toBe(true);
    if (viewport.width <= 1023) {
      const backdrop = await drawer.evaluate(
        (element) => getComputedStyle(element, '::backdrop').backgroundColor,
      );
      expect(backdrop).not.toBe('rgba(0, 0, 0, 0)');
      await page.keyboard.press('Shift+Tab');
      await expect(drawer.locator(':focus-visible')).toHaveCSS('outline-style', 'solid');
      await drawer.getByRole('searchbox', { name: 'Search countries' }).focus();
    }

    const scroll = await drawer.locator('.map-links').evaluate((links) => {
      links.scrollTop = links.scrollHeight;
      return links.scrollTop;
    });
    expect(scroll).toBeGreaterThan(0);
    await drawer.locator('.map-links').evaluate((links) => (links.scrollTop = 0));

    for (const [theme, surface, text] of [
      ['dark', 'rgb(18, 26, 43)', 'rgb(238, 243, 255)'],
      ['light', 'rgb(255, 255, 255)', 'rgb(23, 32, 51)'],
    ]) {
      const currentTheme = await page.evaluate(() => document.documentElement.dataset.theme);
      if ((currentTheme === 'light') !== (theme === 'light')) {
        const themeButton = page.getByRole('button', {
          name: currentTheme === 'light' ? 'Switch to dark theme' : 'Switch to light theme',
        });
        if (viewport.width <= 1023) {
          await drawer.getByRole('button', { name: 'Close map navigation' }).click();
          await themeButton.click();
          await page.getByRole('button', { name: 'Open maps navigation' }).click();
        } else await themeButton.click();
      }
      await expect(drawer).toHaveCSS('background-color', surface);
      await expect(drawer).toHaveCSS('color', text);
    }

    await page.emulateMedia({ reducedMotion: 'reduce' });
    await expect(drawer).toHaveCSS('transition-duration', '0s');
    await page.emulateMedia({ reducedMotion: 'no-preference' });
  }
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

test('header follows results scroll progress and restores at the top', async ({ page }) => {
  const results = page.getByRole('region', { name: 'Map results' });
  const header = page.locator('header');
  const logo = page.getByRole('link', { name: 'PopCircles home' });
  const intro = page.locator('.workspace-intro');

  async function scrollResultsTo(top) {
    await results.evaluate((region, position) => {
      region.scrollTop = position;
      region.dispatchEvent(new Event('scroll'));
    }, top);
  }

  for (const viewport of [
    { width: 1280, height: 800 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    await scrollResultsTo(80);
    await expect(header).not.toHaveClass(/collapsed/);
    await expect(intro).toHaveAttribute('aria-hidden', 'false');
    expect(
      Number(await intro.evaluate((element) => getComputedStyle(element).opacity)),
    ).toBeCloseTo(0.5);
    await scrollResultsTo(160);
    await expect(header).toHaveClass(/collapsed/);
    await expect(intro).toHaveAttribute('aria-hidden', 'true');
    await expect(intro).toHaveCSS('opacity', '0');
    await expect(logo).toBeVisible();
    await expect(page.getByRole('link', { name: 'PopCircles on GitHub' })).toBeVisible();
    await expect(page.getByRole('button', { name: /Switch to .* theme/ })).toBeVisible();
    if (viewport.width <= 1023) {
      await page.getByRole('button', { name: 'Open maps navigation' }).click();
      await expect(page.getByRole('dialog', { name: 'Browse maps' })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.getByRole('dialog', { name: 'Browse maps' })).toBeHidden();
    } else {
      const navigationButton = page.getByRole('button', { name: 'Collapse maps navigation' });
      await expect(navigationButton).toBeVisible();
      await navigationButton.click();
      const openMaps = page.getByRole('button', { name: 'Expand maps navigation' });
      await expect(openMaps).toBeVisible();
      await openMaps.click();
    }
    await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await expect(page.getByRole('button', { name: 'Switch to dark theme' })).toBeVisible();
    await page.getByRole('button', { name: 'Switch to dark theme' }).click();

    await scrollResultsTo(80);
    await expect(header).not.toHaveClass(/collapsed/);
    await scrollResultsTo(0);
    await expect(header).not.toHaveClass(/collapsed/);
    await expect(intro).toHaveAttribute('aria-hidden', 'false');
    await expect(intro).toHaveCSS('opacity', '1');
    const dimensions = await page.evaluate(() => ({
      documentHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
    }));
    expect(dimensions.documentHeight).toBeLessThanOrEqual(dimensions.viewportHeight);
  }
});

test('header collapse respects reduced motion', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const results = page.getByRole('region', { name: 'Map results' });
  await results.evaluate((region) => {
    region.scrollTop = 160;
    region.dispatchEvent(new Event('scroll'));
  });
  await expect(page.locator('header')).toHaveClass(/collapsed/);
  await expect(page.locator('header')).toHaveCSS('transition-duration', '0s');
  await expect(page.getByRole('link', { name: 'PopCircles home' })).toHaveCSS(
    'transition-duration',
    '0s',
  );
});

test('country navigation is keyboard operable and returns focus after Escape', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  const trigger = page.getByRole('button', { name: 'Open maps navigation' });
  await expect(trigger).toHaveAttribute('aria-controls', 'country-browser-panel');
  const triggerBounds = await trigger.boundingBox();
  expect(triggerBounds.width).toBeGreaterThanOrEqual(44);
  expect(triggerBounds.height).toBeGreaterThanOrEqual(44);
  await trigger.focus();
  await page.keyboard.press('Enter');

  const dialog = page.getByRole('dialog', { name: 'Browse maps' });
  await expect(dialog).toBeVisible();
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
  await expect(dialog.getByRole('button', { name: 'Close map navigation' })).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Tab');
  expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toBeFocused();

  await page.setViewportSize({ width: 1280, height: 800 });
  await page.reload();
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Expand maps navigation' })).toBeVisible();
});

test('mobile selection and viewport changes preserve query and independent state', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();
  const trigger = page.getByRole('button', { name: 'Open maps navigation' });
  const dialog = page.getByRole('dialog', { name: 'Browse maps' });
  const search = dialog.getByRole('searchbox', { name: 'Search countries' });

  await trigger.click();
  await expect(dialog.getByRole('button', { name: 'Close map navigation' })).toBeFocused();
  await search.fill('country 42');
  const startUrl = page.url();
  await page.mouse.click(380, 500);
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await expect(search).toHaveValue('country 42');
  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(dialog).toBeVisible();
  await expect.poll(() => dialog.evaluate((element) => element.matches(':modal'))).toBe(false);
  await expect(search).toHaveValue('country 42');
  expect(page.url()).toBe(startUrl);

  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(dialog).toBeHidden();
  expect(page.url()).toBe(startUrl);
  await trigger.click();
  await expect(search).toHaveValue('country 42');
  await dialog.getByRole('link', { name: 'Country 42', exact: true }).click();
  await expect(page).toHaveURL(/#country-42$/);
  await expect(dialog).toBeHidden();
  await expect(page.locator('#map-heading-country-42')).toBeFocused();
  await trigger.click();
  await expect(search).toHaveValue('');
  await dialog.getByRole('button', { name: 'Close map navigation' }).click();
  await expect(trigger).toBeFocused();
});

test('short narrow viewports keep navigation controls visible and scroll regions separate', async ({
  page,
}) => {
  const longCountry = {
    name: 'Long'.repeat(80),
    anchor: 'country-long',
    figures: [],
    missing: false,
  };
  await page.route('**/gallery-data.json', (route) =>
    route.fulfill({ json: { world, countries: [...countries, longCountry] } }),
  );
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();

  const dialog = page.getByRole('dialog', { name: 'Browse maps' });
  const results = page.getByRole('region', { name: 'Map results' });
  const list = dialog.locator('.map-links');
  const longLink = dialog.getByRole('link', { name: longCountry.name });

  for (const viewport of [
    { width: 320, height: 568 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    await expect(
      page.getByRole('heading', { name: 'One circle, half the population.' }),
    ).toBeVisible();
    const resultsScrollTop = await results.evaluate((element) => element.scrollTop);
    await page.getByRole('button', { name: 'Open maps navigation' }).click();
    await expect(dialog).toBeVisible();
    await expect
      .poll(() => dialog.evaluate((element) => getComputedStyle(element).transform))
      .toBe('matrix(1, 0, 0, 1, 0, 0)');
    const bounds = await dialog.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width);

    const closeBounds = await dialog
      .getByRole('button', { name: 'Close map navigation' })
      .boundingBox();
    expect(closeBounds.width).toBeGreaterThanOrEqual(44);
    expect(closeBounds.height).toBeGreaterThanOrEqual(44);
    await list.evaluate((element) => (element.scrollTop = element.scrollHeight));
    expect(await list.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await expect(dialog.getByRole('searchbox', { name: 'Search countries' })).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Close map navigation' })).toBeVisible();
    const longLinkBounds = await longLink.boundingBox();
    expect(longLinkBounds.x + longLinkBounds.width).toBeLessThanOrEqual(bounds.x + bounds.width);
    expect(await results.evaluate((element) => element.scrollTop)).toBe(resultsScrollTop);
    const documentMetrics = await page.evaluate(() => ({
      height: document.documentElement.scrollHeight,
      viewport: window.innerHeight,
      rootTop: document.documentElement.scrollTop,
      bodyTop: document.body.scrollTop,
    }));
    expect(documentMetrics.height).toBeLessThanOrEqual(documentMetrics.viewport);
    expect(documentMetrics.rootTop).toBe(0);
    expect(documentMetrics.bodyTop).toBe(0);

    await page.setViewportSize({ width: 1024, height: viewport.height });
    await expect.poll(() => dialog.evaluate((element) => element.matches(':modal'))).toBe(false);
    const navigationBounds = await dialog.boundingBox();
    const listPosition = await list.evaluate((element) => element.scrollTop);
    await results.evaluate((element) => (element.scrollTop = element.scrollHeight));
    await expect.poll(() => results.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    expect(await dialog.boundingBox()).toEqual(navigationBounds);
    expect(await list.evaluate((element) => element.scrollTop)).toBe(listPosition);
  }
});

test('navigation fits viewports equivalent to 200% and 400% zoom', async ({ page }) => {
  for (const zoom of [2, 4]) {
    const viewport = { width: Math.floor(1440 / zoom), height: Math.floor(900 / zoom) };
    await page.setViewportSize(viewport);
    const trigger = page.getByRole('button', { name: 'Open maps navigation' });
    await expect(trigger).toBeVisible();
    const triggerBounds = await trigger.boundingBox();
    expect(triggerBounds.x).toBeGreaterThanOrEqual(0);
    expect(triggerBounds.x + triggerBounds.width).toBeLessThanOrEqual(viewport.width);
    await trigger.click();
    const dialog = page.getByRole('dialog', { name: 'Browse maps' });
    await expect(dialog).toBeVisible();
    await expect
      .poll(() => dialog.evaluate((element) => getComputedStyle(element).transform))
      .toBe('matrix(1, 0, 0, 1, 0, 0)');
    const bounds = await dialog.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width);
    await expect(dialog.getByRole('button', { name: 'Close map navigation' })).toBeVisible();
    await expect(dialog.getByRole('searchbox', { name: 'Search countries' })).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  }
});

test('World map figures stay stacked on desktop and mobile', async ({ page }) => {
  await page.route('**/gallery-data.json', (route) =>
    route.fulfill({
      json: {
        world: [...world, { ...world[0], map: 'world-second.svg', alt: 'Second world map' }],
        countries,
      },
    }),
  );
  await page.route('**/world-second.svg', (route) =>
    route.fulfill({
      contentType: 'image/svg+xml',
      body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 6" />',
    }),
  );
  await page.goto('/');
  const maps = page.locator('#world-result figure');
  await expect(maps).toHaveCount(2);
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 844 });
    const first = await maps.nth(0).boundingBox();
    const second = await maps.nth(1).boundingBox();
    expect(second.y).toBeGreaterThan(first.y);
  }
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
  await expect(page.getByRole('searchbox', { name: 'Search countries' })).toBeVisible();
});

test('theme choice persists after reload', async ({ page }) => {
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Switch to dark theme' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  expect(await page.evaluate(() => localStorage.getItem('popcircles-theme'))).toBe('light');
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
      const globeElement = brandText.querySelector('.globe-letter');
      const globe = globeElement.getBoundingClientRect();
      const fontSize = Number.parseFloat(getComputedStyle(brandText).fontSize);
      const slotGap = Number.parseFloat(getComputedStyle(brandText).columnGap);
      return {
        slotCenter: slot.left + slot.width / 2,
        slotMiddle: slot.top + slot.height / 2,
        globeCenter: globe.left + globe.width / 2,
        globeMiddle: globe.top + globe.height / 2,
        globeScale: globe.width / slot.width,
        inlineScale: globeElement.style.scale,
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
  expect(await page.getByRole('button', { name: 'Open maps navigation' }).isVisible()).toBe(true);
  await expect(page).toHaveScreenshot('gallery-mobile-dark.png');

  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  visual = await visualProperties();
  expect(visual.background).toBe('#f5f7fb');
  expect(visual.colorScheme).toBe('light');
  await expect(page).toHaveScreenshot('gallery-mobile-light.png');
});
