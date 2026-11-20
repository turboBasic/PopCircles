<script lang="ts">
  import { onMount } from 'svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import { fade } from 'svelte/transition';
  import AppHeader from '#lib/components/layout/AppHeader.svelte';
  import AppFooter from '#lib/components/layout/AppFooter.svelte';
  import CountryNavigation from '#lib/components/navigation/CountryNavigation.svelte';
  import GalleryWorkspace from '#lib/components/gallery/GalleryWorkspace.svelte';
  import { entryFromHash, countryUrl } from '#lib/utils/gallery';
  import { useViewportChrome } from '#lib/components/layout/viewport-chrome.svelte.ts';
  import type { GalleryData, GalleryEntry } from '#lib/types/gallery';

  const desktopNavigationStorageKey = 'popcircles-desktop-navigation-collapsed';
  const themeStorageKey = 'popcircles-theme';

  function readStorage(key: string) {
    try {
      return localStorage.getItem(key);
    } catch {
      // Browser privacy settings can disable storage.
      return null;
    }
  }

  function writeStorage(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Browser privacy settings can disable storage.
    }
  }

  let data = $state<GalleryData | null>(null);
  let error = $state('');
  let hash = $state('');
  let mobileMode = $state(false);
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
  let entryAwaitingDrawerClose: GalleryEntry | null = null;
  let desktopNavigationCollapsed = $state(false);
  let themeLight = $state(false);
  let copyStatus = $state('');
  let copyTimeout: number | undefined;
  let resultsPanel = $state<HTMLDivElement | null>(null);
  let headerElement = $state<HTMLElement | null>(null);
  let footerElement = $state<HTMLElement | null>(null);
  const viewportChrome = useViewportChrome(
    () => resultsPanel,
    () => headerElement,
    () => footerElement,
  );

  $effect(() => {
    document.documentElement.dataset.theme = themeLight ? 'light' : 'dark';
  });

  function scrollToEntry(entry: GalleryEntry | null, behavior: 'auto' | 'smooth') {
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

  function revealEntry(entry: GalleryEntry) {
    requestAnimationFrame(() => {
      scrollToEntry(entry, 'smooth');
      document
        .getElementById(entry.anchor)
        ?.querySelector<HTMLElement>('header h2')
        ?.focus({ preventScroll: true });
    });
  }

  function selectEntry(entry: GalleryEntry) {
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

  function setDesktopNavigationCollapsed(collapsed: boolean) {
    desktopNavigationCollapsed = collapsed;
    writeStorage(desktopNavigationStorageKey, String(collapsed));
  }

  function toggleTheme() {
    themeLight = !themeLight;
    writeStorage(themeStorageKey, themeLight ? 'light' : 'dark');
  }

  function showCopyStatus(message: string) {
    copyStatus = message;
    window.clearTimeout(copyTimeout);
    copyTimeout = window.setTimeout(() => (copyStatus = ''), 2400);
  }

  async function copyCountryUrl(event: MouseEvent, anchor: string) {
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
    const viewport = window.matchMedia('(max-width: 1023px)');
    const updateMobileMode = () => (mobileMode = viewport.matches);
    viewport.addEventListener('change', updateMobileMode);
    updateMobileMode();
    hash = window.location.hash;
    const hasInitialFragment = Boolean(window.location.hash);
    desktopNavigationCollapsed = readStorage(desktopNavigationStorageKey) === 'true';
    themeLight = readStorage(themeStorageKey) === 'light';

    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        requestAnimationFrame(() => {
          scrollToEntry(selectedEntry, 'auto');
          if (!hasInitialFragment && document.activeElement === document.body) {
            resultsPanel?.focus({ preventScroll: true });
            resultsPanel?.setAttribute('data-auto-focused', '');
          }
        });
      })
      .catch((cause) => {
        if (active) error = cause instanceof Error ? cause.message : String(cause);
      });

    return () => {
      active = false;
      viewport.removeEventListener('change', updateMobileMode);
      window.clearTimeout(copyTimeout);
    };
  });
</script>

<svelte:window onpopstate={handleHistory} onhashchange={handleHistory} />

<svelte:head>
  <title>PopCircles</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Lilita+One&display=swap" rel="stylesheet" />
</svelte:head>

<div
  id="app-shell"
  style={`--header-space: ${viewportChrome.headerSpace}px; --navigation-header-space: ${viewportChrome.navigationHeaderSpace}px; --footer-space: ${viewportChrome.footerSpace}px`}
>
  <AppHeader
    scrollPosition={viewportChrome.scrollPosition}
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
      <div id="gallery-layout" class:nav-collapsed={desktopNavigationCollapsed}>
        <CountryNavigation
          worldEntry={entries[0]}
          countries={data.countries}
          {selectedEntry}
          {mobileMode}
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
  <AppFooter bind:footerElement />
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
  #app-shell {
    --country-inline-padding: 1.25rem;
    position: relative;
    display: grid;
    width: min(1600px, calc(100% - 2rem));
    height: 100dvh;
    min-height: 0;
    margin: auto;
    grid-template-rows: minmax(0, 1fr);
  }
  @supports (scroll-timeline: --results-scroll block) and (timeline-scope: --results-scroll) and
    (animation-range: 0px 160px) {
    #app-shell {
      timeline-scope: --results-scroll;
    }
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
  #gallery-layout {
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
  #gallery-layout.nav-collapsed {
    grid-template-columns: minmax(60px, 60px) minmax(0, 1fr);
  }
  @media (max-width: 1023px) {
    #app-shell {
      --country-inline-padding: 0.5rem;
      width: calc(100% - 1rem);
    }
    #gallery-layout {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr);
      gap: 0;
    }
    #gallery-layout.nav-collapsed {
      grid-template-columns: 1fr;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    #gallery-layout {
      transition: none;
    }
  }
</style>
