<script>
  import { onMount } from 'svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import { MediaQuery } from 'svelte/reactivity';
  import { fade } from 'svelte/transition';
  import AppHeader from './components/AppHeader.svelte';
  import CountryNavigation from './components/CountryNavigation.svelte';
  import GalleryWorkspace from './components/GalleryWorkspace.svelte';
  import { entryFromHash, countryUrl } from './gallery.js';
  import { useViewportChrome } from './viewport-chrome.svelte.js';

  /** @typedef {import('./gallery.js').GalleryData} GalleryData */
  /** @typedef {import('./gallery.js').GalleryEntry} GalleryEntry */

  const desktopNavigationStorageKey = 'popcircles-desktop-navigation-collapsed';
  const themeStorageKey = 'popcircles-theme';

  /** @param {string} key */
  function readStorage(key) {
    try {
      return localStorage.getItem(key);
    } catch {
      // Browser privacy settings can disable storage.
      return null;
    }
  }

  /** @param {string} key @param {string} value */
  function writeStorage(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Browser privacy settings can disable storage.
    }
  }

  const narrowViewport = new MediaQuery('(max-width: 1023px)');
  let data = $state(/** @type {GalleryData | null} */ (null));
  let error = $state('');
  let hash = $state(window.location.hash);
  let entries = $derived(
    data
      ? [
          { name: 'World', anchor: 'world-result', figures: data.world, missing: false },
          ...data.countries,
        ]
      : [],
  );
  let selectedEntry = $derived(entryFromHash(entries, hash));
  let drawerOpen = $state(false);
  /** @type {GalleryEntry | null} */
  let entryAwaitingDrawerClose = null;
  let desktopNavigationCollapsed = $state(readStorage(desktopNavigationStorageKey) === 'true');
  let themeLight = $state(readStorage(themeStorageKey) === 'light');
  let copyStatus = $state('');
  /** @type {number | undefined} */
  let copyTimeout;
  /** @type {HTMLDivElement | null} */
  let resultsPanel = $state(null);
  /** @type {HTMLElement | null} */
  let headerElement = $state(null);
  /** @type {HTMLElement | null} */
  let footerElement = $state(null);
  const viewportChrome = useViewportChrome(
    () => resultsPanel,
    () => headerElement,
    () => footerElement,
  );

  $effect(() => {
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
  });

  /** @param {GalleryEntry | null} entry @param {'auto' | 'smooth'} behavior */
  function scrollToEntry(entry, behavior) {
    const result = entry && document.getElementById(entry.anchor);
    if (!result) {
      resultsPanel?.scrollTo({
        top: 0,
        behavior: prefersReducedMotion.current ? 'auto' : behavior,
      });
      return;
    }
    result.scrollIntoView({
      block: 'start',
      behavior: prefersReducedMotion.current ? 'auto' : behavior,
    });
  }

  /** @param {GalleryEntry} entry */
  function revealEntry(entry) {
    requestAnimationFrame(() => {
      scrollToEntry(entry, 'smooth');
      document.getElementById(`map-heading-${entry.anchor}`)?.focus({ preventScroll: true });
    });
  }

  /** @param {GalleryEntry} entry */
  function selectEntry(entry) {
    const url = countryUrl(window.location.href, entry.anchor);
    if (url !== window.location.href) window.history.pushState(null, '', url);
    hash = window.location.hash;
    copyStatus = '';
    window.clearTimeout(copyTimeout);
    if (drawerOpen) {
      entryAwaitingDrawerClose = entry;
      drawerOpen = false;
    } else revealEntry(entry);
  }

  function navigateHome() {
    const url = countryUrl(window.location.href, null);
    if (url !== window.location.href) window.history.pushState(null, '', url);
    hash = window.location.hash;
    copyStatus = '';
    window.clearTimeout(copyTimeout);
    requestAnimationFrame(() => scrollToEntry(null, 'smooth'));
  }

  function handleDrawerClosed() {
    drawerOpen = false;
    if (entryAwaitingDrawerClose) revealEntry(entryAwaitingDrawerClose);
    entryAwaitingDrawerClose = null;
  }

  function handleHistory() {
    hash = window.location.hash;
    requestAnimationFrame(() => scrollToEntry(selectedEntry, 'smooth'));
  }

  /** @param {boolean} collapsed */
  function setDesktopNavigationCollapsed(collapsed) {
    desktopNavigationCollapsed = collapsed;
    writeStorage(desktopNavigationStorageKey, String(collapsed));
  }

  function toggleTheme() {
    themeLight = !themeLight;
    writeStorage(themeStorageKey, themeLight ? 'light' : 'dark');
  }

  /** @param {string} message */
  function showCopyStatus(message) {
    copyStatus = message;
    window.clearTimeout(copyTimeout);
    copyTimeout = window.setTimeout(() => (copyStatus = ''), 2400);
  }

  /** @param {MouseEvent} event @param {string} anchor */
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
    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        requestAnimationFrame(() => scrollToEntry(selectedEntry, 'auto'));
      })
      .catch((cause) => {
        if (active) error = cause.message;
      });

    return () => {
      active = false;
      window.clearTimeout(copyTimeout);
    };
  });
</script>

<svelte:window onpopstate={handleHistory} />

<svelte:head>
  <title>PopCircles</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Lilita+One&display=swap" rel="stylesheet" />
</svelte:head>

<div
  class="app-shell"
  style={`--header-space: ${viewportChrome.headerSpace}px; --navigation-header-space: ${viewportChrome.navigationHeaderSpace}px; --footer-space: ${viewportChrome.footerSpace}px`}
>
  <AppHeader
    scrollPosition={viewportChrome.scrollPosition}
    headerCollapseProgress={viewportChrome.headerCollapseProgress}
    {data}
    {themeLight}
    {drawerOpen}
    bind:headerElement
    onHome={navigateHome}
    onTheme={toggleTheme}
    onOpenDrawer={() => (drawerOpen = true)}
  />

  <main>
    {#if error}
      <div class="workspace-message">
        <p role="alert">Could not load gallery data: {error}</p>
      </div>
    {:else if !data}
      <div class="workspace-message">
        <p role="status">Loading gallery…</p>
      </div>
    {:else}
      <div class="explorer" class:nav-collapsed={desktopNavigationCollapsed}>
        <CountryNavigation
          worldEntry={entries[0]}
          countries={data.countries}
          {selectedEntry}
          mobileMode={narrowViewport.current}
          {drawerOpen}
          {desktopNavigationCollapsed}
          onSelect={selectEntry}
          onCloseDrawer={() => (drawerOpen = false)}
          onDrawerClosed={handleDrawerClosed}
          onDesktopCollapsedChange={setDesktopNavigationCollapsed}
        />

        <GalleryWorkspace
          {entries}
          {selectedEntry}
          bind:resultsPanel
          onScroll={viewportChrome.handleScroll}
          onCopy={copyCountryUrl}
        />
      </div>
    {/if}
  </main>
  <footer bind:this={footerElement} class="site-footer">
    <a href="https://github.com/turboBasic">A project by Andrii Melnyk</a>
  </footer>
</div>

{#if copyStatus}
  <p
    class="copy-toast"
    role="status"
    aria-live="polite"
    aria-atomic="true"
    transition:fade={{ duration: 150 }}
  >
    {copyStatus}
  </p>
{/if}

<style>
  .app-shell {
    position: relative;
    display: grid;
    width: min(1600px, calc(100% - 2rem));
    height: 100dvh;
    min-height: 0;
    margin: auto;
    grid-template-rows: minmax(0, 1fr);
  }
  main {
    display: grid;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
  }
  .workspace-message {
    display: flex;
    min-height: 0;
    flex-direction: column;
    justify-content: space-between;
  }
  .workspace-message p {
    margin-block: 1rem;
  }
  .site-footer {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 4;
    padding: 0.5rem 0;
    color: var(--muted);
    background: transparent;
    font-size: 0.72rem;
  }
  .site-footer::before {
    position: absolute;
    z-index: 0;
    inset: auto 0 0;
    height: 200%;
    background: var(--chrome-bg);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    -webkit-mask-image: linear-gradient(to bottom, transparent 0 50%, #000 50% 100%);
    mask-image: linear-gradient(to bottom, transparent 0 50%, #000 50% 100%);
    pointer-events: none;
    content: '';
  }
  .site-footer a {
    position: relative;
    z-index: 1;
    color: var(--text);
    text-decoration: none;
  }
  .site-footer a:hover,
  .site-footer a:focus-visible {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .explorer {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: minmax(60px, 20rem) minmax(0, 1fr);
    gap: 1.5rem;
    align-items: stretch;
    transition: grid-template-columns 180ms ease;
  }
  .copy-toast {
    position: fixed;
    right: 1.25rem;
    bottom: 1.25rem;
    z-index: 10;
    max-width: min(28rem, calc(100vw - 2.5rem));
    margin: 0;
    padding: 0.8rem 1rem;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.75rem;
    box-shadow: 0 0.5rem 2rem var(--shadow);
  }
  .explorer.nav-collapsed {
    grid-template-columns: minmax(60px, 60px) minmax(0, 1fr);
  }
  @media (max-width: 1023px) {
    .app-shell {
      width: calc(100% - 2rem);
    }
    .explorer {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr);
      gap: 0;
    }
    .explorer.nav-collapsed {
      grid-template-columns: 1fr;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .explorer {
      transition: none;
    }
  }
</style>
