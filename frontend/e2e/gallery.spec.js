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
const alphabetCountries = [
  ...Array.from({ length: 40 }, (_, index) => {
    const number = String(index + 1).padStart(2, '0');
    return {
      name: `Alpine ${number}`,
      anchor: `country-alpine-${number}`,
      figures: [],
      missing: false,
    };
  }),
  { name: 'Brazil', anchor: 'country-brazil', figures: [], missing: false },
  { name: 'Canada', anchor: 'country-canada', figures: [], missing: false },
];

async function useAlphabetFixture(page) {
  await page.unroute('**/gallery-data.json');
  await page.route('**/gallery-data.json', (route) =>
    route.fulfill({ json: { world, countries: alphabetCountries } }),
  );
  await page.goto('/?fixture=alphabet#country-alpine-01');
  await expect(page.locator('#country-alpine-01')).toBeVisible();
}

function colorChannels(color) {
  if (color.startsWith('#')) {
    let hex = color.slice(1);
    if (hex.length === 3) hex = [...hex].map((channel) => channel + channel).join('');
    return [0, 2, 4].map((index) => Number.parseInt(hex.slice(index, index + 2), 16));
  }
  return color
    .match(/[\d.]+/g)
    .slice(0, 3)
    .map(Number);
}

function compositeColor(foreground, background) {
  const alphaMatch = foreground.match(/\/\s*([\d.]+)%/);
  const alpha = alphaMatch ? Number(alphaMatch[1]) / 100 : 1;
  const backgroundChannels = colorChannels(background);
  const blended = colorChannels(foreground).map((channel, index) =>
    Math.round(channel * alpha + backgroundChannels[index] * (1 - alpha)),
  );
  return `rgb(${blended.join(' ')})`;
}

function contrastRatio(first, second) {
  const luminance = (color) => {
    const channels = colorChannels(color).map((channel) => channel / 255);
    const [red, green, blue] = channels.map((channel) =>
      channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
    );
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  };
  const [lighter, darker] = [luminance(first), luminance(second)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}

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

// --- Navigation and history ---

test('[keyboard-and-accessibility] major page components have unique IDs', async ({ page }) => {
  for (const id of [
    'site-header',
    'site-header__intro',
    'site-header__intro-content',
    'site-actions',
    'header-art',
    'top-bar',
    'app-shell',
    'logo',
    'country-navigation-panel',
    'country-navigation',
    'country-links',
    'gallery-layout',
    'gallery',
    'country-cards',
    'site-footer',
  ])
    await expect(page.locator(`#${id}`)).toHaveCount(1);
  await expect(
    page.locator('#logo').getByRole('link', { name: 'PopCircles home' }),
  ).toHaveAttribute('href', './');
});

test('[navigation-and-history] searches country navigation and selects a country', async ({
  page,
}) => {
  const navigation = page.getByRole('navigation', { name: 'Country navigation' });
  await expect(page.getByRole('region', { name: 'Country links' })).toBeVisible();
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
  await expect(navigation.getByRole('link', { name: 'World' })).toHaveAttribute(
    'href',
    '#world-result',
  );
  await expect(page.getByRole('dialog', { name: 'Country navigation' })).toContainText(
    'Country navigation',
  );
  await expect(navigation.getByRole('search', { name: 'Filter map navigation' })).toBeVisible();
  await expect(navigation.getByText('Matching: 80')).toBeVisible();
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
  await expect(navigation.getByRole('link')).toHaveCount(2);
  await expect(navigation.locator('.search-label .status')).toHaveText('Matching: 1');
  const countryGroup = navigation.getByRole('heading', { name: 'C', exact: true });
  await expect(countryGroup).toHaveClass(/visually-hidden/);
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('no result');
  await expect(navigation.getByText('No matching countries.')).toBeVisible();
  await navigation.getByRole('button', { name: 'Clear search' }).click();
  await expect(navigation.getByRole('searchbox', { name: 'Search countries' })).toHaveValue('');
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
  await page
    .getByRole('navigation', { name: 'Country navigation' })
    .getByRole('link', { name: 'Country 42' })
    .click();

  await expect(page).toHaveURL(/#country-42$/);
  await expect(page.locator('#country-42')).toBeVisible();
  await expect(page.locator('#country-41')).toBeVisible();
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
  const mapLink = page.getByRole('link', { name: 'Country 42 map' });
  await expect(mapLink).toHaveAttribute('href', '#country-42');
  await mapLink.click();
  await expect(page).toHaveURL(/#country-42$/);
  await navigation.getByRole('link', { name: 'World' }).click();
  await expect(page).toHaveURL(/#world-result$/);
  await expect(page.locator('#world-result')).toBeInViewport();
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
});

test('[keyboard-and-accessibility] Gallery receives focus and arrow keys on initial load', async ({
  page,
}) => {
  const gallery = page.locator('#gallery');
  await expect(gallery).toBeFocused();
  await expect(gallery).toHaveCSS('outline-style', 'none');
  await expect(gallery).toHaveAttribute('data-auto-focused', '');

  // Headless Playwright delivers these keys but does not apply native scrolling; headed Chrome is checked manually.
  await page.evaluate(() => {
    window.__galleryKeyEvents = [];
    window.addEventListener('keydown', (event) => {
      window.__galleryKeyEvents.push({
        key: event.key,
        targetId: event.target instanceof Element ? event.target.id : '',
        defaultPrevented: event.defaultPrevented,
      });
    });
  });
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowUp');
  expect(await page.evaluate(() => window.__galleryKeyEvents)).toEqual([
    { key: 'ArrowDown', targetId: 'gallery', defaultPrevented: false },
    { key: 'ArrowUp', targetId: 'gallery', defaultPrevented: false },
  ]);
  const bounds = await gallery.boundingBox();
  await page.mouse.click(bounds.x + 8, bounds.y + 8);
  await expect(gallery).toHaveAttribute('data-auto-focused', '');
  await expect(gallery).toHaveCSS('outline-style', 'none');
  await page.getByRole('button', { name: /Switch to (light|dark) theme/ }).focus();
  await expect(gallery).not.toHaveAttribute('data-auto-focused');
});

test('[keyboard-and-accessibility] direct result fragments do not move focus to Gallery', async ({
  page,
}) => {
  await page.goto('/#country-42');
  await expect(page.locator('#country-42')).toBeInViewport();
  await expect(page.locator('#gallery')).not.toBeFocused();
});

test('[navigation-and-history] logo returns to the start and clears the country selection without reloading', async ({
  page,
}) => {
  let galleryDataRequests = 0;
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.endsWith('/gallery-data.json')) galleryDataRequests += 1;
  });

  await page
    .getByRole('navigation', { name: 'Country navigation' })
    .getByRole('link', { name: 'Country 42' })
    .click();
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  await expect(page).toHaveURL(/#country-42$/);
  await expect.poll(() => results.evaluate((region) => region.scrollTop)).toBeGreaterThan(0);

  await page.getByRole('link', { name: 'PopCircles home' }).click();

  await expect(page).toHaveURL('/');
  await expect.poll(() => results.evaluate((region) => region.scrollTop)).toBe(0);
  await expect(
    page.getByRole('navigation', { name: 'Country navigation' }).locator('[aria-current="page"]'),
  ).toHaveCount(0);
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
  expect(galleryDataRequests).toBe(0);

  await page.goBack();
  await expect(page).toHaveURL(/#country-42$/);
  await expect(
    page.getByRole('navigation', { name: 'Country navigation' }).getByRole('link', {
      name: 'Country 42',
    }),
  ).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('#country-42')).toBeInViewport();
  await page.goForward();
  await expect(page).toHaveURL('/');
  await expect.poll(() => results.evaluate((region) => region.scrollTop)).toBe(0);
  await expect(
    page.getByRole('navigation', { name: 'Country navigation' }).locator('[aria-current="page"]'),
  ).toHaveCount(0);
  expect(galleryDataRequests).toBe(0);
});

test('scrolls the selected country into view in the navigation list', async ({ page }) => {
  await page.goto('/#country-80');
  const navigation = page.getByRole('navigation', { name: 'Country navigation' });
  const selectedLink = navigation.getByRole('link', { name: 'Country 80' });
  await expect(selectedLink).toHaveAttribute('aria-current', 'page');
  await expect(selectedLink).toBeInViewport();
  await expect
    .poll(() => selectedLink.evaluate((link) => getComputedStyle(link, '::before').width))
    .toBe('3px');
  await expect
    .poll(() => navigation.locator('.map-links-scroll').evaluate((list) => list.scrollTop))
    .toBeGreaterThan(0);
});

test('[navigation-and-history] [keyboard-and-accessibility] A-Z jumps scroll only the drawer and preserve selection and URL', async ({
  page,
}) => {
  await useAlphabetFixture(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.emulateMedia({ reducedMotion: 'reduce' });

  const dialog = page.getByRole('dialog', { name: 'Country navigation' });
  const navigation = dialog.getByRole('navigation', { name: 'Country navigation' });
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  const countryList = navigation.locator('.map-links-scroll');
  const letterIndex = navigation.getByRole('group', { name: 'Jump to countries by letter' });
  const startUrl = page.url();
  const resultsScrollTop = await results.evaluate((region) => region.scrollTop);
  const selectedCountry = navigation.getByRole('link', { name: 'Alpine 01', exact: true });

  await expect(selectedCountry).toHaveAttribute('aria-current', 'page');
  await expect(
    letterIndex.getByRole('button', { name: 'Jump to countries starting with B' }),
  ).toBeEnabled();
  const brazilButton = letterIndex.getByRole('button', {
    name: 'Jump to countries starting with B',
  });
  await brazilButton.focus();
  await expect(brazilButton).toHaveCSS('outline-style', 'solid');
  await page.keyboard.press('Enter');

  await expect(navigation.locator('#country-group-B')).toBeInViewport();
  await expect.poll(() => countryList.evaluate((list) => list.scrollTop)).toBeGreaterThan(0);
  expect(page.url()).toBe(startUrl);
  expect(await results.evaluate((region) => region.scrollTop)).toBe(resultsScrollTop);
  await expect(selectedCountry).toHaveAttribute('aria-current', 'page');

  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Open maps navigation' }).click();
  await expect(dialog).toBeVisible();
  await expect.poll(() => dialog.evaluate((element) => element.matches(':modal'))).toBe(true);
  const mobileResultsScrollTop = await results.evaluate((region) => region.scrollTop);
  await countryList.evaluate((list) => (list.scrollTop = 0));
  await brazilButton.click();

  await expect(dialog).toBeVisible();
  await expect(navigation.locator('#country-group-B')).toBeInViewport();
  expect(page.url()).toBe(startUrl);
  expect(await results.evaluate((region) => region.scrollTop)).toBe(mobileResultsScrollTop);
  await expect(selectedCountry).toHaveAttribute('aria-current', 'page');
});

test('[navigation-and-history] A-Z availability follows the displayed country groups and search filter', async ({
  page,
}) => {
  await useAlphabetFixture(page);
  const navigation = page.getByRole('navigation', { name: 'Country navigation' });
  const letterIndex = navigation.getByRole('group', { name: 'Jump to countries by letter' });
  const letter = (value) =>
    letterIndex.getByRole('button', {
      name: `Jump to countries starting with ${value}`,
    });

  await expect(letter('A')).toBeEnabled();
  await expect(letter('B')).toBeEnabled();
  await expect(letter('C')).toBeEnabled();
  await expect(letter('D')).toBeDisabled();
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('Brazil');
  await expect(letter('A')).toBeDisabled();
  await expect(letter('B')).toBeEnabled();
  await expect(letter('C')).toBeDisabled();
  await navigation.getByRole('searchbox', { name: 'Search countries' }).fill('no match');
  await expect(letterIndex.locator('button:enabled')).toHaveCount(0);
});

test('[navigation-and-history] direct links, Back and Forward scroll to the selected result and keep every card rendered', async ({
  page,
}) => {
  await page.goto('/#country-42');
  const drawer = page.getByRole('dialog', { name: 'Country navigation' });
  const navigation = page.getByRole('navigation', { name: 'Country navigation' });
  await expect(navigation.getByRole('link', { name: 'Country 42' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  await expect(page.locator('#country-cards > .country-card')).toHaveCount(81);
  await expect(page.locator('#country-42')).toBeInViewport();
  await navigation.getByRole('link', { name: 'Country 01' }).click();
  await expect(page).toHaveURL(/#country-1$/);
  await expect(page.locator('#country-1')).toBeInViewport();
  await expect.poll(() => results.evaluate((region) => region.scrollTop)).toBeGreaterThan(0);
  await expect(navigation.getByRole('link', { name: 'Country 01' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.goBack();
  await expect(page).toHaveURL(/#country-42$/);
  await expect(page.locator('#country-42')).toBeInViewport();
  await expect(navigation.getByRole('link', { name: 'Country 42' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.goForward();
  await expect(page).toHaveURL(/#country-1$/);
  await expect(page.locator('#country-1')).toBeInViewport();
  await expect(navigation.getByRole('link', { name: 'Country 01' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expect(drawer).toBeVisible();
});

test('[navigation-and-history] result fragment links update selection and browser history', async ({
  page,
}) => {
  await page.goto('/#country-42');
  const navigation = page.getByRole('navigation', { name: 'Country navigation' });
  const selectCountry = async (name) =>
    expect(navigation.getByRole('link', { name })).toHaveAttribute('aria-current', 'page');

  await selectCountry('Country 42');
  await page.locator('#country-1 h2 a').click();
  await expect(page).toHaveURL(/#country-1$/);
  await expect(page.locator('#country-1')).toBeInViewport();
  await selectCountry('Country 01');

  await page.goBack();
  await expect(page).toHaveURL(/#country-42$/);
  await selectCountry('Country 42');
  await page.goForward();
  await expect(page).toHaveURL(/#country-1$/);
  await selectCountry('Country 01');
});

test('drawer selection scrolls the results smoothly instead of jumping', async ({ page }) => {
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  await results.evaluate((region) => {
    window.galleryScrollSamples = [];
    let frames = 0;
    function sample() {
      window.galleryScrollSamples.push(region.scrollTop);
      if (frames++ < 120) requestAnimationFrame(sample);
    }
    requestAnimationFrame(sample);
  });

  await page
    .getByRole('navigation', { name: 'Country navigation' })
    .getByRole('link', { name: 'Country 80', exact: true })
    .click();
  await expect(page).toHaveURL(/#country-80$/);
  await expect
    .poll(() => page.evaluate(() => new Set(window.galleryScrollSamples.map(Math.round)).size))
    .toBeGreaterThan(2);
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

// --- Responsive navigation ---

test('[responsive-navigation] desktop navigation collapses to a rail without overlapping results', async ({
  page,
}) => {
  for (const width of [1024, 1440]) {
    await page.setViewportSize({ width, height: 768 });
    const drawer = page.getByRole('dialog', { name: 'Country navigation' });
    await expect(drawer).toBeVisible();
    await expect(page.getByRole('navigation', { name: 'Country navigation' })).toHaveCount(1);
    const workspaceBefore = await page
      .getByRole('region', { name: 'Country and World workspace' })
      .boundingBox();
    const drawerBounds = await drawer.boundingBox();
    expect(drawerBounds.x).toBeGreaterThanOrEqual(0);
    expect(drawerBounds.width).toBeGreaterThanOrEqual(300);
    expect(workspaceBefore.x).toBeGreaterThan(drawerBounds.x + drawerBounds.width);
    await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
    await expect
      .poll(() => drawer.evaluate((element) => element.getBoundingClientRect().width))
      .toBeGreaterThanOrEqual(56);
    await expect
      .poll(() => drawer.evaluate((element) => element.getBoundingClientRect().width))
      .toBeLessThanOrEqual(64);
    const rail = await drawer.boundingBox();
    const workspaceExpanded = await page
      .getByRole('region', { name: 'Country and World workspace' })
      .boundingBox();
    expect(rail.width).toBeGreaterThanOrEqual(56);
    expect(rail.width).toBeLessThanOrEqual(64);
    await expect(drawer.getByRole('link', { name: 'World' })).toBeHidden();
    await expect(drawer.getByRole('searchbox', { name: 'Search countries' })).toBeHidden();
    expect(workspaceExpanded.width).toBeGreaterThan(workspaceBefore.width);
    expect(workspaceExpanded.x).toBeGreaterThan(rail.x + rail.width);
    await page.getByRole('button', { name: 'Search countries' }).click();
    await expect(drawer.getByRole('searchbox', { name: 'Search countries' })).toBeFocused();
    await expect
      .poll(() =>
        page
          .getByRole('region', { name: 'Country and World workspace' })
          .evaluate((element) => element.getBoundingClientRect().width),
      )
      .toBeLessThan(workspaceExpanded.width);
    const workspaceAfterExpand = await page
      .getByRole('region', { name: 'Country and World workspace' })
      .boundingBox();
    expect(workspaceAfterExpand.width).toBeLessThan(workspaceExpanded.width);
    const list = drawer.locator('.map-links-scroll');
    await list.evaluate((element) => (element.scrollTop = 500));
    const listScrollTop = await list.evaluate((element) => element.scrollTop);
    await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
    await page.getByRole('button', { name: 'Expand maps navigation' }).click();
    await expect
      .poll(() => drawer.evaluate((element) => element.getBoundingClientRect().width))
      .toBeGreaterThanOrEqual(300);
    await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBe(listScrollTop);
    await page.getByRole('searchbox', { name: 'Search countries' }).fill('country 42');
    await drawer
      .getByRole('navigation', { name: 'Country navigation' })
      .getByRole('link', { name: 'Country 42', exact: true })
      .click();
    await expect(page).toHaveURL(/#country-42$/);
    await expect(
      drawer
        .getByRole('navigation', { name: 'Country navigation' })
        .getByRole('link', { name: 'Country 42', exact: true }),
    ).toHaveAttribute('aria-current', 'page');
    await expect(page.getByRole('searchbox', { name: 'Search countries' })).toHaveValue(
      'country 42',
    );
    if (width === 1024) await page.goto('/');
  }
});

test('[responsive-navigation] desktop navigation defaults expanded and remembers its state', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(page.getByRole('button', { name: 'Collapse maps navigation' })).toBeVisible();

  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await expect(page.getByRole('button', { name: 'Expand maps navigation' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Expand maps navigation' })).toBeVisible();

  await page.getByRole('button', { name: 'Expand maps navigation' }).click();
  await expect(page.getByRole('button', { name: 'Collapse maps navigation' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Collapse maps navigation' })).toBeVisible();
});

test('[responsive-navigation] mobile navigation remains modal and closes on resize', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();
  const drawer = page.getByRole('dialog', { name: 'Country navigation' });
  await expect(drawer).toBeHidden();
  await page.getByRole('button', { name: 'Open maps navigation' }).click();
  await expect(drawer).toBeVisible();
  await expect.poll(() => drawer.evaluate((element) => element.matches(':modal'))).toBe(true);
  expect(
    await page.getByRole('region', { name: 'Country and World workspace' }).boundingBox(),
  ).not.toBeNull();
  const mobileWorkspace = await page
    .getByRole('region', { name: 'Country and World workspace' })
    .boundingBox();
  await drawer.getByRole('button', { name: 'Close map navigation' }).click();
  await expect(drawer).toBeHidden();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  expect(
    await page.getByRole('region', { name: 'Country and World workspace' }).boundingBox(),
  ).toEqual(mobileWorkspace);
  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(drawer).toBeVisible();
  await expect.poll(() => drawer.evaluate((element) => element.matches(':modal'))).toBe(false);
  await page.getByRole('button', { name: 'Collapse maps navigation' }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  const mobileResults = await page
    .getByRole('region', { name: 'Country and World workspace' })
    .boundingBox();
  expect(mobileResults.x).toBeLessThan(20);
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

test('[responsive-navigation] [keyboard-and-accessibility] country navigation is keyboard operable and returns focus after Escape', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(page.getByRole('button', { name: 'Open maps navigation' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  const trigger = page.getByRole('button', { name: 'Open maps navigation' });
  await expect(trigger).toHaveAttribute('aria-controls', 'country-navigation-panel');
  const triggerBounds = await trigger.boundingBox();
  expect(triggerBounds.width).toBeGreaterThanOrEqual(44);
  expect(triggerBounds.height).toBeGreaterThanOrEqual(44);
  await trigger.focus();
  await page.keyboard.press('Enter');

  const dialog = page.getByRole('dialog', { name: 'Country navigation' });
  await expect(dialog).toBeVisible();
  await expect(trigger).toBeHidden();
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
  await expect(dialog.locator(':focus')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Expand maps navigation' })).toBeVisible();
});

test('[responsive-navigation] mobile selection and viewport changes preserve query and independent state', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'One circle, half the population.' }),
  ).toBeVisible();
  const trigger = page.getByRole('button', { name: 'Open maps navigation' });
  const dialog = page.getByRole('dialog', { name: 'Country navigation' });
  const search = dialog.getByRole('searchbox', { name: 'Search countries' });

  await trigger.click();
  expect(await dialog.evaluate((element) => getComputedStyle(element).animationName)).toMatch(
    /navigation-drawer-in$/,
  );
  expect(await dialog.evaluate((element) => getComputedStyle(element).animationDuration)).toBe(
    '0.4s',
  );
  await expect(dialog.getByRole('button', { name: 'Close map navigation' })).toBeFocused();
  await search.fill('country 42');
  const startUrl = page.url();
  await page.mouse.click(380, 500);
  await expect
    .poll(() => dialog.evaluate((element) => getComputedStyle(element).animationName))
    .toMatch(/navigation-drawer-out$/);
  await expect(dialog).toBeVisible();
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
  await expect(page.locator('#country-42 header h2')).toBeFocused();
  await trigger.click();
  await expect(search).toHaveValue('');
  await dialog.getByRole('button', { name: 'Close map navigation' }).click();
  await expect(trigger).toBeFocused();
});

test('[responsive-navigation] [themes] [keyboard-and-accessibility] drawer presentation fits each viewport, theme, and motion preference', async ({
  page,
}) => {
  const viewports = [
    { width: 1440, height: 900 },
    { width: 1024, height: 768 },
    { width: 390, height: 844 },
    { width: 320, height: 844 },
  ];
  const drawer = page.getByRole('dialog', { name: 'Country navigation' });

  for (const viewport of viewports) {
    await page.setViewportSize(viewport);
    await page.reload();
    await expect(
      page.getByRole('heading', { name: 'One circle, half the population.' }),
    ).toBeVisible();
    const workspace = page.getByRole('region', { name: 'Country and World workspace' });
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
    if (viewport.width <= 1023)
      expect(viewport.width - (bounds.x + bounds.width)).toBeGreaterThanOrEqual(48);
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

    const scroll = await drawer.locator('.map-links-scroll').evaluate((links) => {
      links.scrollTop = links.scrollHeight;
      return links.scrollTop;
    });
    expect(scroll).toBeGreaterThan(0);
    await drawer.locator('.map-links-scroll').evaluate((links) => (links.scrollTop = 0));

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

// --- Viewport and scrolling ---

test('[viewport-and-scrolling] results scroll inside the viewport without scrolling the document', async ({
  page,
}) => {
  const metrics = await page
    .getByRole('region', { name: 'Country and World workspace' })
    .evaluate((results) => {
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

test('[viewport-and-scrolling] header follows the Gallery scroll timeline', async ({ page }) => {
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  const header = page.locator('#site-header');
  const logo = page.getByRole('link', { name: 'PopCircles home' });
  const intro = page.locator('#site-header__intro');
  const headerArt = page.locator('#header-art');

  async function scrollResultsTo(top) {
    await results.evaluate((region, position) => {
      region.scrollTop = position;
    }, top);
  }

  async function styleNumber(element, property) {
    return element.evaluate(
      (node, name) => Number.parseFloat(getComputedStyle(node).getPropertyValue(name)),
      property,
    );
  }

  async function introHeight() {
    return intro.evaluate((element) => element.getBoundingClientRect().height);
  }

  async function naturalIntroHeight() {
    return intro
      .locator('#site-header__intro-content')
      .evaluate((element) => element.getBoundingClientRect().height);
  }

  async function buttonPositions() {
    return page.evaluate(() =>
      Object.fromEntries(
        [
          ['desktop-navigation', '#country-navigation-panel .desktop-collapse'],
          ['drawer-navigation', '#site-header .drawer-toggle'],
          ['github', '#site-actions a'],
          ['theme', '#site-actions button'],
        ].map(([name, selector]) => [
          name,
          document.querySelector(selector)?.getBoundingClientRect().x ?? null,
        ]),
      ),
    );
  }

  async function expectChromeAlignedToCards(headerCollapsed = false, mobile = false) {
    const alignment = await page.evaluate(() => {
      const bounds = (selector) => document.querySelector(selector)?.getBoundingClientRect();
      const country = bounds('#world-result');
      const figure = bounds('#world-result figure');
      const theme = bounds('#site-actions button');
      const drawer = bounds('#site-header .drawer-toggle');
      const footer = bounds('#site-footer a');
      return {
        countryLeft: country?.left,
        countryRight: country?.right,
        figureRight: figure?.right,
        themeRight: theme?.right,
        drawerLeft: drawer?.left,
        footerRight: footer?.right,
      };
    });

    expect(alignment.footerRight).toBeCloseTo(alignment.figureRight, 1);
    if (headerCollapsed) {
      expect(alignment.themeRight).toBeCloseTo(alignment.countryRight, 1);
      if (mobile) expect(alignment.drawerLeft).toBeCloseTo(alignment.countryLeft, 1);
    }
  }

  async function timelineProgress() {
    return intro.evaluate(
      (element) => element.getAnimations()[0]?.effect?.getComputedTiming().progress ?? null,
    );
  }

  const supportsScrollTimeline = await page.evaluate(
    () =>
      CSS.supports('scroll-timeline: --results-scroll block') &&
      CSS.supports('timeline-scope: --results-scroll') &&
      CSS.supports('animation-range: 0px 160px'),
  );

  for (const viewport of [
    { width: 1280, height: 800 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    await scrollResultsTo(0);
    const expandedHeight = await introHeight();
    expect(expandedHeight).toBeGreaterThan(0);
    expect(expandedHeight).toBeCloseTo(await naturalIntroHeight(), 1);
    expect(await styleNumber(header, 'padding-top')).toBeCloseTo(12, 1);
    expect(await styleNumber(intro, 'margin-top')).toBeCloseTo(20, 1);
    expect(await styleNumber(intro, 'opacity')).toBeCloseTo(1, 2);
    const expandedButtonPositions = await buttonPositions();
    await expectChromeAlignedToCards();

    if (!supportsScrollTimeline) {
      await scrollResultsTo(160);
      await expect.poll(introHeight).toBeCloseTo(expandedHeight, 1);
      expect(await styleNumber(intro, 'opacity')).toBeCloseTo(1, 2);
      await expectChromeAlignedToCards();
      continue;
    }

    await expect.poll(() => styleNumber(headerArt, 'opacity')).toBeCloseTo(0.16, 2);
    expect(await styleNumber(logo, 'font-size')).toBeCloseTo(16, 1);
    expect(await styleNumber(logo, 'letter-spacing')).toBeCloseTo(2.56, 1);

    await scrollResultsTo(80);
    await expect.poll(introHeight).toBeCloseTo(expandedHeight / 2, 1);
    await expect.poll(() => styleNumber(header, 'padding-top')).toBeCloseTo(8, 1);
    expect(await styleNumber(intro, 'margin-top')).toBeCloseTo(10, 1);
    expect(await styleNumber(intro, 'opacity')).toBeCloseTo(0.5, 2);
    expect(await styleNumber(headerArt, 'opacity')).toBeCloseTo(0.08, 2);
    expect(await styleNumber(logo, 'font-size')).toBeCloseTo(15.04, 1);
    expect(await styleNumber(logo, 'letter-spacing')).toBeCloseTo(2.26, 1);
    const halfCollapsedButtonPositions = await buttonPositions();
    for (const control of ['github', 'theme']) {
      expect(halfCollapsedButtonPositions[control]).toBeCloseTo(
        expandedButtonPositions[control] + 6,
        1,
      );
    }
    if (viewport.width > 1023) {
      expect(halfCollapsedButtonPositions['desktop-navigation']).toBeCloseTo(
        expandedButtonPositions['desktop-navigation'] - 4,
        1,
      );
    } else {
      expect(halfCollapsedButtonPositions['drawer-navigation']).toBeCloseTo(
        expandedButtonPositions['drawer-navigation'] - 6,
        1,
      );
    }

    await scrollResultsTo(160);
    await expect.poll(introHeight).toBe(0);
    await expect.poll(() => styleNumber(header, 'padding-top')).toBeCloseTo(4, 1);
    expect(await styleNumber(intro, 'margin-top')).toBeCloseTo(0, 1);
    expect(await styleNumber(intro, 'opacity')).toBeCloseTo(0, 2);
    expect(await styleNumber(headerArt, 'opacity')).toBeCloseTo(0, 2);
    await expectChromeAlignedToCards(true, viewport.width <= 1023);
    expect(await styleNumber(logo, 'font-size')).toBeCloseTo(14.08, 1);
    expect(await styleNumber(logo, 'letter-spacing')).toBeCloseTo(1.8304, 1);
    const collapsedButtonPositions = await buttonPositions();
    for (const control of ['github', 'theme']) {
      expect(collapsedButtonPositions[control]).toBeCloseTo(
        expandedButtonPositions[control] + 12,
        1,
      );
    }
    if (viewport.width > 1023) {
      expect(collapsedButtonPositions['desktop-navigation']).toBeCloseTo(
        expandedButtonPositions['desktop-navigation'] - 8,
        1,
      );
    } else {
      expect(collapsedButtonPositions['drawer-navigation']).toBeCloseTo(
        expandedButtonPositions['drawer-navigation'] - 12,
        1,
      );
    }
    await expect(logo).toBeVisible();
    await expect(page.getByRole('link', { name: 'PopCircles on GitHub' })).toBeVisible();
    await expect(page.getByRole('button', { name: /Switch to .* theme/ })).toBeVisible();
    await expect(
      page.getByRole('button', {
        name: viewport.width <= 1023 ? 'Open maps navigation' : 'Collapse maps navigation',
      }),
    ).toBeVisible();
    await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await expect(page.getByRole('button', { name: 'Switch to dark theme' })).toBeVisible();
    await page.getByRole('button', { name: 'Switch to dark theme' }).click();

    await scrollResultsTo(80);
    await expect.poll(introHeight).toBeCloseTo(expandedHeight / 2, 1);
    await page.setViewportSize({
      width: viewport.width <= 1023 ? 320 : 1024,
      height: viewport.height,
    });
    await scrollResultsTo(0);
    await expect.poll(timelineProgress).toBeCloseTo(0, 2);
    const resizedExpandedHeight = await naturalIntroHeight();
    expect(resizedExpandedHeight).not.toBeCloseTo(expandedHeight, 1);
    await expect.poll(introHeight).toBeCloseTo(resizedExpandedHeight, 1);
    await scrollResultsTo(80);
    await expect.poll(introHeight).toBeCloseTo(resizedExpandedHeight / 2, 1);

    await scrollResultsTo(160);
    await expect.poll(introHeight).toBe(0);
    await expectChromeAlignedToCards(true, viewport.width <= 1023);

    await scrollResultsTo(0);
    await expect.poll(introHeight).toBeCloseTo(resizedExpandedHeight, 1);
    await expect.poll(() => styleNumber(intro, 'opacity')).toBeCloseTo(1, 2);
    const dimensions = await page.evaluate(() => ({
      documentHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
    }));
    expect(dimensions.documentHeight).toBeLessThanOrEqual(dimensions.viewportHeight);
  }
});

test('header collapse respects reduced motion', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  const intro = page.locator('#site-header__intro');
  await results.evaluate((region) => {
    region.scrollTop = 160;
  });
  await expect
    .poll(() => intro.evaluate((element) => element.getBoundingClientRect().height))
    .toBe(0);
  await expect(page.locator('#site-header')).toHaveCSS('transition-duration', '0s');
  await expect(page.getByRole('link', { name: 'PopCircles home' })).toHaveCSS(
    'transition-duration',
    '0s',
  );
});

test('[viewport-and-scrolling] short narrow viewports keep navigation controls visible and scroll regions separate', async ({
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

  const dialog = page.getByRole('dialog', { name: 'Country navigation' });
  const results = page.getByRole('region', { name: 'Country and World workspace' });
  const list = dialog.locator('.map-links-scroll');
  const longLink = dialog.getByRole('link', { name: longCountry.name });

  for (const viewport of [
    { width: 320, height: 568 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
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
    await expect(dialog.getByRole('button', { name: 'Collapse maps navigation' })).toBeHidden();
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
    await expect
      .poll(() =>
        dialog.evaluate((element) => ({
          open: element.open,
          modal: element.matches(':modal'),
        })),
      )
      .toEqual({ open: true, modal: false });
    await expect(dialog.getByRole('button', { name: 'Collapse maps navigation' })).toBeVisible();
    await expect
      .poll(() =>
        dialog.evaluate((element) => ({
          position: getComputedStyle(element).position,
          width: element.getBoundingClientRect().width,
        })),
      )
      .toEqual({ position: 'relative', width: 320 });
    const navigationBounds = await dialog.boundingBox();
    await list.evaluate((element) => (element.scrollTop = element.scrollHeight));
    await results.evaluate((element) => (element.scrollTop = element.scrollHeight));
    await expect.poll(() => results.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    expect(await dialog.boundingBox()).toEqual(navigationBounds);
    const listMetrics = await list.evaluate((element) => ({
      scrollTop: element.scrollTop,
      maxScrollTop: element.scrollHeight - element.clientHeight,
    }));
    expect(listMetrics.maxScrollTop - listMetrics.scrollTop).toBeLessThanOrEqual(1);
  }
});

test('[viewport-and-scrolling] navigation fits viewports equivalent to 200% and 400% zoom', async ({
  page,
}) => {
  for (const zoom of [2, 4]) {
    const viewport = { width: Math.floor(1440 / zoom), height: Math.floor(900 / zoom) };
    await page.setViewportSize(viewport);
    const trigger = page.getByRole('button', { name: 'Open maps navigation' });
    await expect(trigger).toBeVisible();
    const triggerBounds = await trigger.boundingBox();
    expect(triggerBounds.x).toBeGreaterThanOrEqual(0);
    expect(triggerBounds.x + triggerBounds.width).toBeLessThanOrEqual(viewport.width);
    await trigger.click();
    const dialog = page.getByRole('dialog', { name: 'Country navigation' });
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

// --- Map presentation ---

test('[map-presentation] emphasizes the map caption values and labels their units', async ({
  page,
}) => {
  await page.unroute('**/gallery-data.json');
  await page.route('**/gallery-data.json', (route) =>
    route.fulfill({
      json: {
        world: [
          {
            ...world[0],
            caption: {
              radius_km: 212,
              share: 0.501,
              population: 18154224,
              centre_lat: 35.0083,
              centre_lon: 69.1583,
              settlement: { name: 'Charikar', country: 'Afghanistan', distance_km: 1 },
              territory_share: 0.21,
              territory_name: 'Afghanistan',
            },
          },
        ],
        countries,
      },
    }),
  );
  await page.reload();

  const caption = page.locator('.map-caption p');
  await expect(caption).toHaveText(
    'radius: 212 km · population: 18,154,224 / 50.1% · Circle covers 21% of Afghanistan · centre: 35.0083, 69.1583 · nearest city: Charikar, Afghanistan (1 km away)',
  );
  await expect(caption.locator('strong')).toHaveText(['212 km', '18,154,224', '50.1%', '21%']);
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

test('[map-presentation] World and Country results share the same card presentation', async ({
  page,
}) => {
  const cards = page.locator('#country-cards > .country-card');
  const styles = await cards.evaluateAll((elements) =>
    elements.slice(0, 3).map((element) => {
      const style = getComputedStyle(element);
      return {
        background: style.backgroundColor,
        borderColor: style.borderColor,
        borderRadius: style.borderRadius,
        borderStyle: style.borderStyle,
        borderWidth: style.borderWidth,
      };
    }),
  );

  expect(styles[0].borderStyle).toBe('solid');
  expect(styles[1]).toEqual(styles[0]);
  expect(styles[2]).toEqual(styles[0]);
});

// --- Keyboard and accessibility ---

test('[keyboard-and-accessibility] interactive controls have accessible names', async ({
  page,
}) => {
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

test('[keyboard-and-accessibility] decorative control icons preserve sizes and accessible names', async ({
  page,
}) => {
  const iconSize = (control) =>
    control.locator('svg').evaluate((icon) => icon.getBoundingClientRect().width);

  const theme = page.getByRole('button', { name: 'Switch to light theme' });
  await expect(theme.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(theme)).toBe(20);

  const collapse = page.getByRole('button', { name: 'Collapse maps navigation' });
  await expect(collapse.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(collapse)).toBe(20);

  const viewData = page.getByRole('link', { name: 'View data' }).first();
  await expect(viewData.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(viewData)).toBe(16);

  const copyUrl = page.getByRole('link', { name: 'Copy URL' });
  await expect(copyUrl.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(copyUrl)).toBe(16);

  await collapse.click();
  const search = page.getByRole('button', { name: 'Search countries' });
  await expect(search.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(search)).toBe(20);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  const open = page.getByRole('button', { name: 'Open maps navigation' });
  await expect(open.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(open)).toBe(20);
  await open.click();

  const close = page.getByRole('button', { name: 'Close map navigation' });
  await expect(close.locator('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(await iconSize(close)).toBe(20);
});

// --- Themes ---

test('theme choice persists after reload', async ({ page }) => {
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Switch to dark theme' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  expect(await page.evaluate(() => localStorage.getItem('popcircles-theme'))).toBe('light');
});

test('[themes] GitHub link shares the theme control hover treatment', async ({ page }) => {
  const githubLink = page.getByRole('link', { name: 'PopCircles on GitHub' });
  const themeButton = page.getByRole('button', { name: 'Switch to light theme' });
  const icon = githubLink.locator('.github-icon');
  const borderColor = (control) =>
    control.evaluate((element) => getComputedStyle(element).borderTopColor);
  const initialBorderColor = await borderColor(githubLink);

  await expect(icon).toHaveAttribute('aria-hidden', 'true');
  await expect(icon).toHaveCSS('width', '20px');
  const maskImage = await icon.evaluate((element) => getComputedStyle(element).maskImage);
  const assetUrl = maskImage.match(/^url\(["']?(.+?)["']?\)$/)?.[1];
  expect(assetUrl).toBeTruthy();
  const response = await page.request.get(new URL(assetUrl, page.url()).toString());
  expect(response.ok()).toBe(true);
  expect(response.headers()['content-type']).toContain('image/svg+xml');

  await githubLink.hover();
  const githubHoverBorderColor = await borderColor(githubLink);
  await themeButton.hover();
  const themeHoverBorderColor = await borderColor(themeButton);

  expect(githubHoverBorderColor).not.toBe(initialBorderColor);
  expect(githubHoverBorderColor).toBe(themeHoverBorderColor);
});

// --- Branding ---

test('favicon loads from a frontend asset', async ({ page }) => {
  const favicon = page.locator('head link[rel="icon"]');
  await expect(favicon).toHaveCount(1);
  await expect(favicon).toHaveAttribute('type', 'image/svg+xml');
  const href = await favicon.getAttribute('href');
  const response = await page.request.get(new URL(href, page.url()).toString());
  expect(response.ok()).toBe(true);
  expect(response.headers()['content-type']).toContain('image/svg+xml');
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

  const results = page.getByRole('region', { name: 'Country and World workspace' });
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

// --- Responsive layout and themes ---

test('[themes] palette text and control colors meet contrast thresholds', async ({ page }) => {
  for (const theme of ['dark', 'light']) {
    const palette = await page.evaluate(() => {
      const style = getComputedStyle(document.documentElement);
      return Object.fromEntries(
        [
          '--bg',
          '--surface',
          '--surface-2',
          '--surface-quiet',
          '--glow',
          '--text',
          '--muted',
          '--accent',
          '--line',
          '--chrome-bg',
        ].map((token) => [token, style.getPropertyValue(token).trim()]),
      );
    });

    for (const foreground of ['--text', '--muted', '--accent']) {
      for (const background of ['--bg', '--surface', '--surface-2', '--surface-quiet', '--glow']) {
        expect(
          contrastRatio(palette[foreground], palette[background]),
          `${theme}: ${foreground} on ${background}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }

    for (const background of ['--bg', '--surface', '--surface-2', '--surface-quiet']) {
      expect(
        contrastRatio(palette['--line'], palette[background]),
        `${theme}: --line against ${background}`,
      ).toBeGreaterThanOrEqual(3);
    }

    const worstCaseBackdrop = theme === 'dark' ? '#fff' : '#000';
    const compositedChrome = compositeColor(palette['--chrome-bg'], worstCaseBackdrop);
    for (const foreground of ['--text', '--muted', '--accent']) {
      expect(
        contrastRatio(palette[foreground], compositedChrome),
        `${theme}: ${foreground} over composited chrome`,
      ).toBeGreaterThanOrEqual(4.5);
    }

    if (theme === 'dark') await page.getByRole('button', { name: 'Switch to light theme' }).click();
  }
});

test('[map-presentation] result links and boundary attribution use a quiet, smaller panel', async ({
  page,
}) => {
  const footer = page.locator('.map-footer').first();

  for (const theme of ['dark', 'light']) {
    const styles = await footer.evaluate((element) => {
      const style = getComputedStyle(element);
      const caption = getComputedStyle(element.closest('figure').querySelector('figcaption'));
      const palette = getComputedStyle(document.documentElement);
      const figure = element.closest('figure');
      const figureStyle = getComputedStyle(figure);
      const panelBounds = element.getBoundingClientRect();
      const figureBounds = figure.getBoundingClientRect();
      return {
        background: style.backgroundColor,
        paletteBackground: palette.getPropertyValue('--surface-quiet').trim(),
        color: style.color,
        muted: palette.getPropertyValue('--muted').trim(),
        fontSize: style.fontSize,
        captionFontSize: caption.fontSize,
        fontWeight: style.fontWeight,
        parentTag: element.tagName,
        edgeInsets: {
          left:
            panelBounds.left - figureBounds.left - Number.parseFloat(figureStyle.borderLeftWidth),
          right:
            figureBounds.right -
            panelBounds.right -
            Number.parseFloat(figureStyle.borderRightWidth),
          bottom:
            figureBounds.bottom -
            panelBounds.bottom -
            Number.parseFloat(figureStyle.borderBottomWidth),
        },
      };
    });

    expect(styles.background).toBe(`rgb(${colorChannels(styles.paletteBackground).join(', ')})`);
    expect(styles.color).toBe(`rgb(${colorChannels(styles.muted).join(', ')})`);
    expect(Number.parseFloat(styles.fontSize)).toBeLessThan(
      Number.parseFloat(styles.captionFontSize),
    );
    expect(styles.fontSize).toBe('12px');
    expect(styles.fontWeight).toBe('400');
    expect(styles.parentTag).toBe('FOOTER');
    expect(styles.edgeInsets.left).toBeCloseTo(0, 1);
    expect(styles.edgeInsets.right).toBeCloseTo(0, 1);
    expect(styles.edgeInsets.bottom).toBeCloseTo(0, 1);
    await expect(footer.getByRole('link', { name: 'View data' })).toBeVisible();
    await expect(
      footer.getByText('© EuroGeographics for the administrative boundaries'),
    ).toBeVisible();

    if (theme === 'dark') await page.getByRole('button', { name: 'Switch to light theme' }).click();
  }
});

test('[viewport-and-scrolling] [map-presentation] [themes] responsive layout, map ratio, and theme styles keep their expected values', async ({
  page,
}) => {
  const visualProperties = () =>
    page.evaluate(() => ({
      background: getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
      colorScheme: getComputedStyle(document.documentElement).colorScheme,
      documentHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
      columns: getComputedStyle(document.querySelector('#gallery-layout')).gridTemplateColumns,
      mapAspectRatio: Number.parseFloat(
        getComputedStyle(document.querySelector('.map-frame')).aspectRatio,
      ),
    }));

  let visual = await visualProperties();
  expect(visual.background).toBe('#0b1020');
  expect(visual.colorScheme).toBe('dark');
  expect(visual.documentHeight).toBeLessThanOrEqual(visual.viewportHeight);
  expect(await page.getByRole('region', { name: 'Country and World workspace' }).isVisible()).toBe(
    true,
  );
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
  await expect(page.getByRole('navigation', { name: 'Country navigation' })).toBeHidden();
  expect(await page.getByRole('button', { name: 'Open maps navigation' }).isVisible()).toBe(true);
  await expect(page).toHaveScreenshot('gallery-mobile-dark.png');

  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  visual = await visualProperties();
  expect(visual.background).toBe('#f5f7fb');
  expect(visual.colorScheme).toBe('light');
  await expect(page).toHaveScreenshot('gallery-mobile-light.png');
});
