<script>
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import AppHeader from './components/AppHeader.svelte';
  import CountryNavigation from './components/CountryNavigation.svelte';
  import GalleryWorkspace from './components/GalleryWorkspace.svelte';
  import { entryFromHash, countryUrl } from './gallery.js';
  import { useViewportChrome } from './viewport-chrome.svelte.js';

  /** @typedef {import('./gallery.js').GalleryData} GalleryData */
  /** @typedef {import('./gallery.js').GalleryEntry} GalleryEntry */

  let data = $state(/** @type {GalleryData | null} */ (null));
  let error = $state('');
  /** @type {GalleryEntry | null} */
  let selectedEntry = $state(null);
  let entries = $derived(
    data
      ? [
          { name: 'World', anchor: 'world-result', figures: data.world, missing: false },
          ...data.countries,
        ]
      : [],
  );
  let mobileDrawerOpen = $state(false);
  let desktopNavigationCollapsed = $state(false);
  let desktopNavigationScrollTop = 0;
  /** @type {number | undefined} */
  let desktopNavigationScrollTimeout;
  /** @type {HTMLDialogElement | null} */
  let mobileDialog = $state(null);
  let themeLight = $state(false);
  let copyStatus = $state('');
  /** @type {number | undefined} */
  let copyTimeout;
  /** @type {MediaQueryList | null} */
  let mobileNavigation = $state(null);
  let mobileMode = $state(false);
  let initialNavigationSync = true;
  /** @type {HTMLButtonElement | null} */
  let browserTrigger = $state(null);
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

  function resetView() {
    resultsPanel?.scrollTo(0, 0);
  }

  /** @param {GalleryEntry[]} availableEntries */
  function restoreEntry(availableEntries = entries) {
    const entry = entryFromHash(availableEntries, window.location.hash);
    selectedEntry = entry;
    requestAnimationFrame(resetView);
  }

  function syncNavigation() {
    if (!mobileDialog || !mobileNavigation) return;
    const initialSync = initialNavigationSync;
    initialNavigationSync = false;
    if (mobileMode) {
      if (mobileDialog.open && !mobileDialog.matches(':modal')) mobileDialog.close();
      if (mobileDrawerOpen && !mobileDialog.open) mobileDialog.showModal();
    } else if (!mobileDialog.open || mobileDialog.matches(':modal')) {
      if (mobileDialog.open) mobileDialog.close();
      mobileDialog.show();
      const focusedElement = document.activeElement;
      if (
        initialSync &&
        focusedElement instanceof HTMLElement &&
        mobileDialog.contains(focusedElement)
      )
        focusedElement.blur();
    }
  }

  function toggleMobileNavigation() {
    mobileDrawerOpen = !mobileDrawerOpen;
    syncNavigation();
    if (mobileDrawerOpen)
      requestAnimationFrame(() => document.getElementById('country-browser-close')?.focus());
  }

  function toggleDesktopNavigation() {
    window.clearTimeout(desktopNavigationScrollTimeout);
    if (!desktopNavigationCollapsed) {
      desktopNavigationScrollTop = document.querySelector('.map-links')?.scrollTop ?? 0;
    }
    desktopNavigationCollapsed = !desktopNavigationCollapsed;
    if (!desktopNavigationCollapsed) {
      desktopNavigationScrollTimeout = window.setTimeout(
        () => document.querySelector('.map-links')?.scrollTo(0, desktopNavigationScrollTop),
        200,
      );
    }
  }

  function expandDesktopNavigationAndFocusSearch() {
    desktopNavigationCollapsed = false;
    requestAnimationFrame(() => document.getElementById('country-search')?.focus());
  }

  /** @param {GalleryEntry} entry */
  function selectEntry(entry) {
    selectedEntry = entry;
    copyStatus = '';
    window.clearTimeout(copyTimeout);
    requestAnimationFrame(() => {
      resetView();
      document.getElementById(`map-heading-${entry.anchor}`)?.focus();
    });
  }

  /** @param {string} message */
  function showCopyStatus(message) {
    copyStatus = message;
    window.clearTimeout(copyTimeout);
    copyTimeout = window.setTimeout(() => (copyStatus = ''), 2400);
  }

  function toggleTheme() {
    themeLight = !themeLight;
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
    try {
      localStorage.setItem('popcircles-theme', themeLight ? 'light' : 'dark');
    } catch {
      // Browser privacy settings can disable storage.
    }
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
    try {
      themeLight = localStorage.getItem('popcircles-theme') === 'light';
    } catch {
      themeLight = false;
    }
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
    const navigationQuery = window.matchMedia('(max-width: 1023px)');
    mobileNavigation = navigationQuery;
    mobileMode = navigationQuery.matches;
    const handleHistory = () => restoreEntry();
    mobileDrawerOpen = false;
    const handleNavigationChange = () => {
      mobileDrawerOpen = false;
      mobileMode = navigationQuery.matches;
      syncNavigation();
    };
    navigationQuery.addEventListener('change', handleNavigationChange);
    requestAnimationFrame(syncNavigation);
    window.addEventListener('hashchange', handleHistory);
    window.addEventListener('popstate', handleHistory);
    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        requestAnimationFrame(syncNavigation);
        restoreEntry([
          { name: 'World', anchor: 'world-result', figures: value.world, missing: false },
          ...value.countries,
        ]);
      })
      .catch((cause) => {
        if (active) error = cause.message;
      });

    return () => {
      active = false;
      window.clearTimeout(copyTimeout);
      window.clearTimeout(desktopNavigationScrollTimeout);
      window.removeEventListener('hashchange', handleHistory);
      window.removeEventListener('popstate', handleHistory);
      navigationQuery.removeEventListener('change', handleNavigationChange);
    };
  });
</script>

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
    drawerOpen={mobileDrawerOpen}
    bind:headerElement
    bind:browserTrigger
    onTheme={toggleTheme}
    onToggleDrawer={toggleMobileNavigation}
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
          {mobileMode}
          drawerOpen={mobileDrawerOpen}
          {desktopNavigationCollapsed}
          bind:mobileDialog
          {browserTrigger}
          onSelect={selectEntry}
          onCloseDrawer={() => {
            if (mobileMode) mobileDrawerOpen = false;
            else desktopNavigationCollapsed = true;
          }}
          onToggleDesktopNavigation={toggleDesktopNavigation}
          onExpandAndFocusSearch={expandDesktopNavigationAndFocusSearch}
        />

        <GalleryWorkspace
          {entries}
          {selectedEntry}
          bind:resultsPanel
          onSelect={selectEntry}
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
    box-shadow: 0 0.5rem 2rem #0005;
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
