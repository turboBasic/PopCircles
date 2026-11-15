<script>
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import AppHeader from './components/AppHeader.svelte';
  import CountryNavigation from './components/CountryNavigation.svelte';
  import GalleryWorkspace from './components/GalleryWorkspace.svelte';
  import {
    entryFromHash,
    countryUrl
  } from './gallery.js';

  const globeFrames = Object.entries(
    import.meta.glob('../assets/earth-globe-*.svg', { eager: true, query: '?url', import: 'default' })
  )
    .sort(([left], [right]) => left.localeCompare(right, undefined, { numeric: true }))
    .map(([, url]) => url);

  let data = $state(null);
  let error = $state('');
  let selectedEntry = $state(null);
  let entries = $derived(
    data
      ? [{ name: 'World', anchor: 'world-result', figures: data.world }, ...data.countries]
      : []
  );
  let browserOpen = $state(false);
  let themeLight = $state(false);
  let copyStatus = $state('');
  let copyTimeout = null;
  let mobileNavigation = $state(null);
  let browserTrigger = $state(null);
  let resultsPanel = $state(null);
  let headerShrink = $state(0);
  let globeFrame = $state(globeFrames[0]);
  let globeFramesReady = false;

  function resetView() {
    headerShrink = 0;
    window.scrollTo(0, 0);
    resultsPanel?.scrollTo(0, 0);
  }

  function restoreEntry(availableEntries = entries) {
    const entry = entryFromHash(availableEntries, window.location.hash);
    selectedEntry = entry;
    if (entry) requestAnimationFrame(resetView);
  }

  function toggleBrowser() {
    browserOpen = !browserOpen;
  }

  function selectEntry(entry) {
    selectedEntry = entry;
    copyStatus = '';
    window.clearTimeout(copyTimeout);
    requestAnimationFrame(() => {
      resetView();
    });
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

  function handleScroll() {
    const scrollDistance = window.scrollY + (resultsPanel?.scrollTop ?? 0);
    headerShrink = Math.min(Math.max(window.scrollY, resultsPanel?.scrollTop ?? 0) / 120, 1);
    if (globeFramesReady) {
      globeFrame = globeFrames[Math.floor(scrollDistance / 240 * globeFrames.length) % globeFrames.length];
    }
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
    const globeFrameImages = globeFrames.map((src) => {
      const image = new Image();
      image.src = src;
      return image;
    });
    Promise.allSettled(globeFrameImages.map((image) => image.decode())).then(() => {
      if (!active) return;
      globeFramesReady = true;
      handleScroll();
    });
    mobileNavigation = window.matchMedia('(max-width: 800px)');
    const handleHistory = () => restoreEntry();
    const handleNavigationChange = () => {
      browserOpen = !mobileNavigation.matches;
    };
    browserOpen = !mobileNavigation.matches;
    handleScroll();
    mobileNavigation.addEventListener('change', handleNavigationChange);
    window.addEventListener('hashchange', handleHistory);
    window.addEventListener('popstate', handleHistory);
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
      mobileNavigation.removeEventListener('change', handleNavigationChange);
    };
  });
</script>

<svelte:head>
  <title>PopCircles</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Lilita+One&display=swap" rel="stylesheet" />
</svelte:head>

<main style:--header-shrink={headerShrink}>
  <AppHeader
    {globeFrame}
    {data}
    {themeLight}
    {browserOpen}
    bind:browserTrigger
    onTheme={toggleTheme}
    onToggleBrowser={toggleBrowser}
  />

  {#if error}
    <p role="alert">Could not load gallery data: {error}</p>
  {:else if !data}
    <p role="status">Loading gallery…</p>
  {:else}
    <div class="explorer">
      <CountryNavigation
        worldEntry={entries[0]}
        countries={data.countries}
        {selectedEntry}
        {mobileNavigation}
        bind:browserOpen
        {browserTrigger}
        onSelect={selectEntry}
      />

      <GalleryWorkspace
        {entries}
        {selectedEntry}
        bind:resultsPanel
        onSelect={selectEntry}
        onScroll={handleScroll}
        onCopy={copyCountryUrl}
      />
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
  @property --header-shrink { syntax: '<number>'; inherits: true; initial-value: 0; }
  :global(:root) { color-scheme: dark; overflow-anchor: none; --bg: #0b1020; --surface: #121a2b; --surface-2: #19243a; --text: #eef3ff; --muted: #a4b0c5; --accent: #80e0c0; --line: #29364d; --glow: #203456; }
  :global(:root[data-theme="light"]) { color-scheme: light; --bg: #f5f7fb; --surface: #fff; --surface-2: #e9eef5; --text: #172033; --muted: #4b596e; --accent: #145c4e; --line: #cbd5e1; --glow: #e5edf7; }
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; color: var(--text); background: radial-gradient(ellipse at 50% -20%, var(--glow) 0, var(--bg) 55%); font: 16px/1.55 system-ui, sans-serif; }
  main { width: min(1600px, calc(100% - 2rem)); margin: auto; }
  .site-footer { color: var(--muted); }
  .explorer { display: grid; min-height: 100vh; grid-template-columns: minmax(15rem, 20rem) minmax(0, 1fr); gap: 1.5rem; align-items: start; }
  .copy-toast { position: fixed; right: 1.25rem; bottom: 1.25rem; z-index: 10; max-width: min(28rem, calc(100vw - 2.5rem)); margin: 0; padding: .8rem 1rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .75rem; box-shadow: 0 .5rem 2rem #0005; }
  .site-footer { margin-top: 2rem; padding: 1.25rem 0; border-top: 1px solid var(--line); font-size: .85rem; }
  @media (max-width: 800px) {
    .explorer { min-height: 0; grid-template-columns: 1fr; }
  }
</style>
