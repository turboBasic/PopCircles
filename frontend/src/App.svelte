<script>
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import {
    entryFromHash,
    countryUrl,
    filterCountries,
    visibleEntries
  } from './gallery.js';

  let data = $state(null);
  let error = $state('');
  let query = $state('');
  let includeZeroPopulation = $state(false);
  let selectedEntry = $state(null);
  let entries = $derived(
    data
      ? [{ name: 'World', anchor: 'world-result', figures: data.world }, ...data.countries]
      : []
  );
  let matches = $derived(
    data ? [entries[0], ...filterCountries(data.countries, query, includeZeroPopulation)] : []
  );
  let visibleMapEntries = $derived(
    visibleEntries(entries, selectedEntry?.anchor ?? null, includeZeroPopulation)
  );
  let browserOpen = $state(false);
  let themeLight = $state(false);
  let copyStatus = $state('');
  let copyTimeout = null;
  let mobileNavigation = $state(null);
  let browser = $state(null);
  let browserTrigger = $state(null);
  let browserPanel = $state(null);
  let countrySearch = $state(null);
  let headerShrink = $state(0);

  function restoreEntry(availableEntries = entries) {
    selectedEntry = entryFromHash(availableEntries, window.location.hash);
  }

  function closeBrowser() {
    if (!mobileNavigation?.matches || !browserOpen) return;
    browserOpen = false;
    browserTrigger.focus();
  }

  function toggleBrowser() {
    if (browserOpen) closeBrowser();
    else browserOpen = true;
  }

  function handleBrowserKeydown(event) {
    if (!mobileNavigation?.matches || !browserOpen) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      closeBrowser();
      return;
    }
    if (event.key !== 'Tab') return;
    const focusable = [...browserPanel.querySelectorAll('input, a')].filter(
      (element) => element.getClientRects().length
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function handlePointerdown(event) {
    if (
      mobileNavigation?.matches &&
      browserOpen &&
      !browser.contains(event.target) &&
      !browserTrigger.contains(event.target)
    ) {
      closeBrowser();
    }
  }

  function selectEntry(entry) {
    const returnFocus = mobileNavigation?.matches && browserOpen;
    selectedEntry = entry;
    copyStatus = '';
    window.clearTimeout(copyTimeout);
    closeBrowser();
    if (returnFocus) requestAnimationFrame(() => browserTrigger.focus());
  }

  function showCopyStatus(message) {
    copyStatus = message;
    window.clearTimeout(copyTimeout);
    copyTimeout = window.setTimeout(() => (copyStatus = ''), 2400);
  }

  function toggleTheme() {
    themeLight = !themeLight;
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
  }

  async function copyCountryUrl(event, anchor) {
    event.preventDefault();
    const url = countryUrl(window.location.href, anchor);
    try {
      await navigator.clipboard.writeText(url);
      showCopyStatus('Country URL copied');
    } catch {
      showCopyStatus('Could not copy URL. Use the link menu to copy it.');
    }
  }

  onMount(() => {
    let active = true;
    mobileNavigation = window.matchMedia('(max-width: 800px)');
    const handleHistory = () => restoreEntry();
    const handleScroll = () => (headerShrink = Math.min(window.scrollY / 120, 1));
    const handleNavigationChange = () => {
      browserOpen = !mobileNavigation.matches;
    };
    browserOpen = !mobileNavigation.matches;
    handleScroll();
    mobileNavigation.addEventListener('change', handleNavigationChange);
    window.addEventListener('hashchange', handleHistory);
    window.addEventListener('popstate', handleHistory);
    document.addEventListener('pointerdown', handlePointerdown);
    window.addEventListener('scroll', handleScroll, { passive: true });
    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        restoreEntry([{ name: 'World', anchor: 'world-result', figures: value.world }, ...value.countries]);
      })
      .catch((cause) => {
        if (active) error = cause.message;
      });

    return () => {
      active = false;
      window.clearTimeout(copyTimeout);
      window.removeEventListener('hashchange', handleHistory);
      window.removeEventListener('popstate', handleHistory);
      window.removeEventListener('scroll', handleScroll);
      document.removeEventListener('pointerdown', handlePointerdown);
      mobileNavigation.removeEventListener('change', handleNavigationChange);
    };
  });
</script>

{#snippet figureCaption(figure, countryAnchor = null)}
  {#if figure.caption}
    <p>
      {figure.caption.radius_km.toLocaleString()} km radius ·
      {(figure.caption.share * 100).toFixed(1)}% of the population ·
      {figure.caption.population.toLocaleString('en-US', { maximumFractionDigits: 0 })} people ·
      centre
      <a
        href={`https://www.openstreetmap.org/?mlat=${figure.caption.centre_lat.toFixed(4)}&mlon=${figure.caption.centre_lon.toFixed(4)}#map=12/${figure.caption.centre_lat.toFixed(4)}/${figure.caption.centre_lon.toFixed(4)}`}
        target="_blank"
        rel="noopener"
      >
        {figure.caption.centre_lat.toFixed(4)}, {figure.caption.centre_lon.toFixed(4)}
      </a>
      {#if figure.caption.settlement}
        · nearest city: {figure.caption.settlement.name},
        {figure.caption.settlement.country}
        ({Math.round(figure.caption.settlement.distance_km).toLocaleString()} km away)
      {/if}
    </p>
  {/if}
  <a href={figure.document}>View result data</a>
  {#if countryAnchor}
    · <a href={countryUrl(window.location.href, countryAnchor)} onclick={(event) => copyCountryUrl(event, countryAnchor)}>Copy country URL</a>
  {/if}
{/snippet}

<svelte:head>
  <title>PopCircles</title>
</svelte:head>

<main style:--header-shrink={headerShrink}>
  <header>
    <div class="top-bar">
      <p class="eyebrow">
        PopCircles ·
        <a class="repository-link" href="https://github.com/turboBasic/PopCircles" aria-label="turbobasic/PopCircles on GitHub">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 .9a11.1 11.1 0 0 0-3.51 21.63c.55.1.76-.24.76-.54v-2.1c-3.1.67-3.76-1.32-3.76-1.32-.5-1.28-1.23-1.62-1.23-1.62-1.01-.69.08-.68.08-.68 1.12.08 1.71 1.15 1.71 1.15 1 .1.75 2.05 3.44 1.57.1-.73.39-1.23.7-1.52-2.48-.28-5.09-1.24-5.09-5.53 0-1.22.44-2.22 1.15-3-.12-.28-.5-1.42.11-2.96 0 0 .94-.3 3.05 1.15a10.6 10.6 0 0 1 5.55 0c2.12-1.44 3.05-1.15 3.05-1.15.61 1.54.23 2.68.11 2.96.72.78 1.15 1.78 1.15 3 0 4.3-2.61 5.24-5.1 5.52.4.35.75 1.03.75 2.08v3.08c0 .3.2.65.77.54A11.1 11.1 0 0 0 12 .9Z"/></svg>
          GitHub
        </a>
      </p>
      {#if data}
        <div class="map-actions" aria-label="Gallery controls">
          <button type="button" aria-pressed={themeLight} onclick={toggleTheme}>
            Use {themeLight ? 'dark' : 'light'} theme
          </button>
          <button
            bind:this={browserTrigger}
            class="mobile-browser-toggle"
            type="button"
            aria-controls="country-browser-panel"
            aria-expanded={browserOpen}
            aria-label={browserOpen ? 'Close map navigation' : 'Open map navigation'}
            onclick={toggleBrowser}
          >
            Maps
          </button>
        </div>
      {/if}
    </div>
    <h1>One circle, half the population.</h1>
    <p class="intro">Explore country and world population circles.</p>
  </header>

  {#if error}
    <p role="alert">Could not load gallery data: {error}</p>
  {:else if !data}
    <p role="status">Loading gallery…</p>
  {:else}
    <div class="explorer">
      <aside aria-label="Map navigation">
        <details
          bind:this={browser}
          bind:open={browserOpen}
          ontoggle={() => {
            if (mobileNavigation?.matches && browser?.open) countrySearch?.focus();
          }}
        >
          <summary
            id="country-browser-trigger"
            aria-controls="country-browser-panel"
            aria-expanded={browserOpen}
          >
            Browse maps <span class="status">{matches.length} maps</span>
          </summary>
          <div
            bind:this={browserPanel}
            class="country-browser-panel"
            id="country-browser-panel"
            role={mobileNavigation?.matches && browserOpen ? 'dialog' : 'region'}
            aria-modal={mobileNavigation?.matches && browserOpen ? 'true' : undefined}
            aria-labelledby={mobileNavigation?.matches && browserOpen ? 'country-browser-title' : undefined}
            onkeydown={handleBrowserKeydown}
          >
            <h2 class="drawer-title" id="country-browser-title">Browse maps</h2>
            <label for="country-search">Search maps</label>
            <input
              bind:this={countrySearch}
              id="country-search"
              type="search"
              bind:value={query}
              placeholder="Search maps"
              autocomplete="off"
            />
            <label class="zero-population-filter">
              <input type="checkbox" bind:checked={includeZeroPopulation} />
              Include territories with zero population
            </label>
            <p class="visually-hidden" role="status" aria-live="polite">
              {query ? `${matches.length} matching maps` : ''}
            </p>
            <nav aria-label="Maps">
              {#each matches as entry (entry.anchor)}
                <a
                  class:active={selectedEntry?.anchor === entry.anchor}
                  aria-current={selectedEntry?.anchor === entry.anchor ? 'page' : undefined}
                  href={`#${entry.anchor}`}
                  onclick={() => selectEntry(entry)}
                >
                  {entry.name}
                </a>
              {:else}
                <p class="empty">No matching maps.</p>
              {/each}
            </nav>
          </div>
        </details>
        <p class="status" role="status" aria-live="polite">{matches.length} maps</p>
      </aside>

      <div class="results">
        <section class="map-results" aria-label="Maps">
          <div id="map-results-list" class:gallery-mode={selectedEntry === null} class:single-map={selectedEntry !== null}>
            {#each visibleMapEntries as entry (entry.anchor)}
              <section class="country" id={entry.anchor}>
                <h2><a href={`#${entry.anchor}`} onclick={() => selectEntry(entry)}>{entry.name}</a></h2>
                {#if entry.missing}
                  <p class="empty">No 50% result is available for this country.</p>
                {:else}
                  <div class="figure-grid">
                    {#each entry.figures as figure (figure.map)}
                      <figure>
                        <a href={figure.map}>
                          <img
                            src={figure.map}
                            alt={figure.alt}
                            loading={selectedEntry?.anchor === entry.anchor ? 'eager' : 'lazy'}
                          />
                        </a>
                        <figcaption>{@render figureCaption(figure, entry.anchor === 'world-result' ? null : entry.anchor)}</figcaption>
                      </figure>
                    {/each}
                  </div>
                {/if}
              </section>
            {/each}
          </div>
        </section>
      </div>
    </div>
  {/if}
  <footer class="site-footer">
    © Andrii Melnyk, 2026 · © EuroGeographics for the administrative boundaries
  </footer>
</main>

{#if copyStatus}
  <p class="copy-toast" role="status" aria-live="polite" aria-atomic="true" transition:fade={{ duration: 150 }}>
    {copyStatus}
  </p>
{/if}

<style>
  :global(:root) { color-scheme: dark; --bg: #0b1020; --surface: #121a2b; --surface-2: #19243a; --text: #eef3ff; --muted: #a4b0c5; --accent: #80e0c0; --line: #29364d; --glow: #203456; }
  :global(:root[data-theme="light"]) { color-scheme: light; --bg: #f5f7fb; --surface: #fff; --surface-2: #e9eef5; --text: #172033; --muted: #4b596e; --accent: #145c4e; --line: #cbd5e1; --glow: #e5edf7; }
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; color: var(--text); background: radial-gradient(ellipse at 50% -20%, var(--glow) 0, var(--bg) 55%); font: 16px/1.55 system-ui, sans-serif; }
  main { width: min(1600px, calc(100% - 2rem)); margin: auto; }
  header { position: sticky; top: 0; z-index: 4; padding: calc(4rem - 3.4rem * var(--header-shrink)) 0 calc(1rem - .4rem * var(--header-shrink)); background: var(--bg); border-bottom: 1px solid color-mix(in srgb, var(--line) calc(100% * var(--header-shrink)), transparent); }
  .top-bar { display: flex; align-items: center; justify-content: space-between; gap: .75rem; }
  .repository-link { color: var(--muted); font: inherit; letter-spacing: inherit; text-decoration: none; text-transform: inherit; }
  .repository-link svg { width: 1.33em; height: 1.33em; overflow: visible; vertical-align: -.285em; }
  .eyebrow { margin: 0; color: var(--accent); font-size: .75rem; font-weight: 700; letter-spacing: .16em; text-transform: uppercase; }
  h1 { max-width: 850px; max-height: calc(4.5rem * (1 - var(--header-shrink))); margin: calc(.7rem * (1 - var(--header-shrink))) 0 calc(1rem * (1 - var(--header-shrink))); overflow: hidden; opacity: calc(1 - var(--header-shrink)); font-size: calc(clamp(2rem, 5vw, 4rem) * (1 - var(--header-shrink))); line-height: 1.08; letter-spacing: -.045em; }
  header .intro { max-height: calc(3rem * (1 - var(--header-shrink))); margin-block: calc(.5rem * (1 - var(--header-shrink))); overflow: hidden; opacity: calc(1 - var(--header-shrink)); }
  .intro, .status, .empty, figcaption, .site-footer { color: var(--muted); }
  .explorer { display: grid; grid-template-columns: minmax(15rem, 20rem) minmax(0, 1fr); gap: 1.5rem; align-items: start; }
  aside { position: sticky; top: calc(4rem + 5rem * (1 - var(--header-shrink))); z-index: 2; display: flex; flex-direction: column; max-height: calc(100vh - (4rem + 5rem * (1 - var(--header-shrink)))); overflow: hidden; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  aside details { display: flex; flex: 1; flex-direction: column; min-height: 0; }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 1rem; cursor: pointer; font-weight: 700; }
  summary:focus-visible, input:focus-visible, button:focus-visible, a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .country-browser-panel { display: flex; flex: 1; flex-direction: column; min-height: 0; max-height: none; overflow: hidden; }
  .zero-population-filter { display: flex; align-items: center; gap: .5rem; margin: .75rem 0; color: var(--muted); }
  .zero-population-filter input { width: auto; padding: 0; }
  .drawer-title { display: none; }
  label { display: block; margin: .8rem 0 .5rem; font-weight: 600; }
  input { width: 100%; padding: .75rem 1rem; color: var(--text); background: var(--bg); border: 1px solid var(--line); border-radius: .5rem; font: inherit; }
  nav { display: grid; flex: 1; gap: .35rem; min-height: 0; margin-top: 1rem; overflow-y: auto; }
  nav a { padding: .25rem 0; color: var(--muted); text-decoration: none; }
  nav a:hover, nav a.active { color: var(--text); text-decoration: underline; text-underline-offset: .2em; }
  .status { font-size: .85rem; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  .results { min-width: 0; padding-bottom: 4rem; }
  .country { margin-bottom: .8rem; padding: 1.25rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  .map-actions { display: flex; align-items: center; gap: .5rem; }
  .mobile-browser-toggle { display: none; }
  button { padding: .55rem .85rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .5rem; font: inherit; cursor: pointer; }
  button:hover { border-color: var(--accent); }
  .copy-toast { position: fixed; right: 1.25rem; bottom: 1.25rem; z-index: 10; max-width: min(28rem, calc(100vw - 2.5rem)); margin: 0; padding: .8rem 1rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .75rem; box-shadow: 0 .5rem 2rem #0005; }
  .site-footer { margin-top: 2rem; padding: 1.25rem 0; border-top: 1px solid var(--line); font-size: .85rem; }
  .site-footer a { color: inherit; }
  .gallery-mode { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr)); gap: .8rem; }
  .gallery-mode .country { margin: 0; }
  .gallery-mode .figure-grid { grid-template-columns: minmax(0, 1fr); }
  .country h2 a { color: inherit; text-decoration: none; }
  .country h2 a:hover { text-decoration: underline; text-underline-offset: .2em; }
  .figure-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr)); gap: 1rem; }
  figure { min-width: 0; margin: 0; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); border-radius: .8rem; }
  figure img { display: block; width: 100%; height: auto; }
  figcaption { padding: .75rem 1rem; font-size: .85rem; }
  figcaption p { margin: 0 0 .5rem; }
  figcaption a { color: inherit; text-decoration-color: #65758e; text-underline-offset: 3px; }
  @media (max-width: 800px) {
    .explorer { grid-template-columns: 1fr; }
    aside { position: static; z-index: auto; display: block; max-height: none; overflow: visible; padding: 0; border: 0; background: transparent; }
    aside summary { display: none; }
    .mobile-browser-toggle { display: block; }
    .country-browser-panel { display: none; }
    details[open] .country-browser-panel { display: block; position: fixed; inset: 1rem; z-index: 5; max-height: none; overflow: auto; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; box-shadow: 0 1rem 3rem #0008; }
    .drawer-title { display: block; }
    nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
