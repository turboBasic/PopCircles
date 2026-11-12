<script>
  import { onMount } from 'svelte';
  import {
    countryFromHash,
    countryUrl,
    defaultCountry,
    filterCountries,
    visibleCountries
  } from './gallery.js';

  let data = $state(null);
  let error = $state('');
  let query = $state('');
  let includeZeroPopulation = $state(false);
  let selectedCountry = $state(null);
  let allOpen = $state(false);
  let matches = $derived(
    data ? filterCountries(data.countries, query, includeZeroPopulation) : []
  );
  let selectedFigure = $derived(selectedCountry?.figures[0] ?? null);
  let visibleCountryResults = $derived(
    data ? visibleCountries(data.countries, selectedCountry?.anchor, allOpen) : []
  );
  let browserOpen = $state(false);
  let themeLight = $state(false);
  let copyStatus = $state('');
  let mobileNavigation = $state(null);
  let browser = $state(null);
  let browserTrigger = $state(null);
  let browserPanel = $state(null);
  let countrySearch = $state(null);

  function restoreCountry(countries = data?.countries) {
    if (countries) {
      selectedCountry = countryFromHash(
        countries,
        window.location.hash,
        defaultCountry(countries)
      );
    }
  }

  function closeBrowser() {
    if (!mobileNavigation?.matches || !browserOpen) return;
    browserOpen = false;
    browserTrigger.focus();
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
    if (mobileNavigation?.matches && browserOpen && !browser.contains(event.target)) {
      closeBrowser();
    }
  }

  function selectCountry(country) {
    selectedCountry = country;
    allOpen = false;
    copyStatus = '';
    closeBrowser();
  }

  function toggleTheme() {
    themeLight = !themeLight;
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
  }

  async function copyCountryUrl() {
    const url = countryUrl(window.location.href, selectedCountry?.anchor);
    try {
      await navigator.clipboard.writeText(url);
      copyStatus = 'Country URL copied.';
    } catch {
      copyStatus = `Copy failed. Copy this URL: ${url}`;
    }
  }

  onMount(() => {
    let active = true;
    mobileNavigation = window.matchMedia('(max-width: 800px)');
    const handleHistory = () => restoreCountry();
    const handleNavigationChange = () => {
      browserOpen = !mobileNavigation.matches;
    };
    browserOpen = !mobileNavigation.matches;
    mobileNavigation.addEventListener('change', handleNavigationChange);
    window.addEventListener('hashchange', handleHistory);
    window.addEventListener('popstate', handleHistory);
    document.addEventListener('pointerdown', handlePointerdown);
    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        restoreCountry(value.countries);
      })
      .catch((cause) => {
        if (active) error = cause.message;
      });

    return () => {
      active = false;
      window.removeEventListener('hashchange', handleHistory);
      window.removeEventListener('popstate', handleHistory);
      document.removeEventListener('pointerdown', handlePointerdown);
      mobileNavigation.removeEventListener('change', handleNavigationChange);
    };
  });
</script>

<svelte:head>
  <title>PopCircles · Map gallery</title>
</svelte:head>

<main>
  <header>
    <p class="eyebrow">
      PopCircles · Map gallery ·
      <a class="repository-link" href="https://github.com/turboBasic/PopCircles" aria-label="turbobasic/PopCircles on GitHub">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 .9a11.1 11.1 0 0 0-3.51 21.63c.55.1.76-.24.76-.54v-2.1c-3.1.67-3.76-1.32-3.76-1.32-.5-1.28-1.23-1.62-1.23-1.62-1.01-.69.08-.68.08-.68 1.12.08 1.71 1.15 1.71 1.15 1 .1.75 2.05 3.44 1.57.1-.73.39-1.23.7-1.52-2.48-.28-5.09-1.24-5.09-5.53 0-1.22.44-2.22 1.15-3-.12-.28-.5-1.42.11-2.96 0 0 .94-.3 3.05 1.15a10.6 10.6 0 0 1 5.55 0c2.12-1.44 3.05-1.15 3.05-1.15.61 1.54.23 2.68.11 2.96.72.78 1.15 1.78 1.15 3 0 4.3-2.61 5.24-5.1 5.52.4.35.75 1.03.75 2.08v3.08c0 .3.2.65.77.54A11.1 11.1 0 0 0 12 .9Z"/></svg>
        GitHub
      </a>
    </p>
    <h1>One circle, half the population.</h1>
    <p class="intro">Explore country and world population circles.</p>
  </header>

  {#if error}
    <p role="alert">Could not load gallery data: {error}</p>
  {:else if !data}
    <p role="status">Loading gallery…</p>
  {:else}
    <div class="explorer">
      <aside aria-label="Country navigation">
        <details
          bind:this={browser}
          bind:open={browserOpen}
          ontoggle={() => {
            if (mobileNavigation?.matches && browser?.open) countrySearch?.focus();
          }}
        >
          <summary
            bind:this={browserTrigger}
            id="country-browser-trigger"
            aria-controls="country-browser-panel"
            aria-expanded={browserOpen}
          >
            Browse countries <span class="status">{data.countries.length} countries</span>
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
            <h2 class="drawer-title" id="country-browser-title">Browse countries</h2>
            <label for="country-search">Search countries</label>
            <input
              bind:this={countrySearch}
              id="country-search"
              type="search"
              bind:value={query}
              placeholder="Search countries"
              autocomplete="off"
            />
            <label class="zero-population-filter">
              <input type="checkbox" bind:checked={includeZeroPopulation} />
              Include territories with zero population
            </label>
            <p class="visually-hidden" role="status" aria-live="polite">
              {query ? `${matches.length} matching countries` : ''}
            </p>
            <nav aria-label="Countries">
              {#each matches as country (country.anchor)}
                <a
                  class:active={selectedCountry?.anchor === country.anchor}
                  aria-current={selectedCountry?.anchor === country.anchor ? 'page' : undefined}
                  href={`#${country.anchor}`}
                  onclick={() => selectCountry(country)}
                >
                  {country.name}
                </a>
              {:else}
                <p class="empty">No matching countries.</p>
              {/each}
            </nav>
          </div>
        </details>
        <p class="status" role="status" aria-live="polite">{matches.length} countries</p>
      </aside>

      <div class="results">
        {#if data.world.length}
          <details class="world">
            <summary>World</summary>
            <div class="figure-grid">
              {#each data.world as figure (figure.map)}
                <figure>
                  <a href={figure.map}><img src={figure.map} alt={figure.alt} loading="eager" /></a>
                  <figcaption><a href={figure.document}>View result data</a></figcaption>
                </figure>
              {/each}
            </div>
          </details>
        {/if}

        <section class="country-results" aria-label="Country maps">
          <div class="country-actions">
            <h2>Countries</h2>
            <div class="map-actions">
              <button type="button" aria-pressed={themeLight} onclick={toggleTheme}>
                Use {themeLight ? 'dark' : 'light'} theme
              </button>
              <button type="button" onclick={copyCountryUrl}>Copy country URL</button>
              {#if selectedFigure}
                <a href={selectedFigure.map} target="_blank" rel="noopener">Open full-resolution map</a>
              {/if}
              <button
                type="button"
                aria-controls="country-results-list"
                aria-expanded={allOpen}
                onclick={() => (allOpen = !allOpen)}
              >
                {allOpen ? 'Collapse all country maps' : 'Expand all country maps'}
              </button>
            </div>
            <p class="copy-status" role="status" aria-live="polite">{copyStatus}</p>
          </div>

          <div id="country-results-list" class:gallery-mode={allOpen} class:single-map={!allOpen}>
            {#each visibleCountryResults as country (country.anchor)}
              <section class="country" id={country.anchor}>
                <h3>{country.name}</h3>
                {#if country.missing}
                  <p class="empty">No 50% result is available for this country.</p>
                {:else}
                  <div class="figure-grid">
                    {#each country.figures as figure (figure.map)}
                      <figure>
                        <a href={figure.map}>
                          <img
                            src={figure.map}
                            alt={figure.alt}
                            loading={country.anchor === selectedCountry?.anchor ? 'eager' : 'lazy'}
                          />
                        </a>
                        <figcaption><a href={figure.document}>View result data</a></figcaption>
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
</main>

<style>
  :global(:root) { color-scheme: dark; --bg: #0b1020; --surface: #121a2b; --surface-2: #19243a; --text: #eef3ff; --muted: #a4b0c5; --accent: #80e0c0; --line: #29364d; --glow: #203456; }
  :global(:root[data-theme="light"]) { color-scheme: light; --bg: #f5f7fb; --surface: #fff; --surface-2: #e9eef5; --text: #172033; --muted: #4b596e; --accent: #145c4e; --line: #cbd5e1; --glow: #e5edf7; }
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; color: var(--text); background: radial-gradient(ellipse at 50% -20%, var(--glow) 0, var(--bg) 55%); font: 16px/1.55 system-ui, sans-serif; }
  main { width: min(1600px, calc(100% - 2rem)); margin: auto; }
  header { padding: 4rem 0 2rem; }
  .repository-link { color: var(--muted); font: inherit; letter-spacing: inherit; text-decoration: none; text-transform: inherit; }
  .repository-link svg { width: 1.33em; height: 1.33em; overflow: visible; vertical-align: -.285em; }
  .eyebrow { color: var(--accent); font-size: .75rem; font-weight: 700; letter-spacing: .16em; text-transform: uppercase; }
  h1 { max-width: 850px; margin: .7rem 0 1rem; font-size: clamp(2rem, 5vw, 4rem); line-height: 1.08; letter-spacing: -.045em; }
  .intro, .status, .empty, figcaption, .copy-status { color: var(--muted); }
  .explorer { display: grid; grid-template-columns: minmax(15rem, 20rem) minmax(0, 1fr); gap: 1.5rem; align-items: start; }
  aside { position: sticky; top: 1rem; z-index: 2; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 1rem; cursor: pointer; font-weight: 700; }
  summary:focus-visible, input:focus-visible, button:focus-visible, a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .country-browser-panel { max-height: calc(100vh - 10rem); overflow: auto; }
  .zero-population-filter { display: flex; align-items: center; gap: .5rem; margin: .75rem 0; color: var(--muted); }
  .zero-population-filter input { width: auto; padding: 0; }
  .drawer-title { display: none; }
  label { display: block; margin: .8rem 0 .5rem; font-weight: 600; }
  input { width: 100%; padding: .75rem 1rem; color: var(--text); background: var(--bg); border: 1px solid var(--line); border-radius: .5rem; font: inherit; }
  nav { display: grid; gap: .35rem; margin-top: 1rem; }
  nav a { padding: .25rem 0; color: var(--muted); text-decoration: none; }
  nav a:hover, nav a.active { color: var(--text); text-decoration: underline; text-underline-offset: .2em; }
  .status, .copy-status { font-size: .85rem; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  .results { min-width: 0; padding-bottom: 4rem; }
  .world, .country { margin-bottom: .8rem; padding: 1.25rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  .world > summary { justify-content: flex-start; font-size: 1.25rem; }
  .country-actions { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin: 2rem 0 .75rem; flex-wrap: wrap; }
  .country-actions h2 { margin: 0; }
  .map-actions { display: flex; align-items: center; flex-wrap: wrap; gap: .5rem; }
  button, .map-actions a { padding: .55rem .85rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .5rem; font: inherit; cursor: pointer; }
  .map-actions a { text-decoration: none; }
  button:hover, .map-actions a:hover { border-color: var(--accent); }
  .copy-status { width: 100%; margin: 0; overflow-wrap: anywhere; }
  .country-results { display: block; }
  .gallery-mode { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr)); gap: .8rem; }
  .gallery-mode .country { margin: 0; }
  .gallery-mode .figure-grid { grid-template-columns: minmax(0, 1fr); }
  .figure-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr)); gap: 1rem; }
  figure { min-width: 0; margin: 0; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); border-radius: .8rem; }
  figure img { display: block; width: 100%; height: auto; }
  figcaption { padding: .75rem 1rem; font-size: .85rem; }
  figcaption a { color: inherit; text-decoration-color: #65758e; text-underline-offset: 3px; }
  @media (max-width: 800px) {
    .explorer { grid-template-columns: 1fr; }
    aside { position: static; padding: 0; border: 0; background: transparent; }
    summary { padding: .8rem 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .6rem; }
    .country-browser-panel { display: none; }
    details[open] .country-browser-panel { display: block; position: fixed; inset: 1rem; z-index: 5; max-height: none; overflow: auto; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; box-shadow: 0 1rem 3rem #0008; }
    .drawer-title { display: block; }
    nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
