<script>
  import { ChevronLeft, ChevronRight, Search, X } from '@lucide/svelte';
  import { tick } from 'svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import globeSprite from '#lib/assets/earth-globe.svg?url';
  import { filterCountries, groupCountries } from '#lib/utils/gallery.js';

  /** @typedef {import('#lib/types/gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{worldEntry: GalleryEntry, countries: GalleryEntry[], selectedEntry: GalleryEntry | null, mobileMode: boolean, drawerOpen: boolean, desktopNavigationCollapsed: boolean, onSelect: (entry: GalleryEntry) => void, onCloseDrawer: () => void, onDrawerClosed: () => void, onDesktopCollapsedChange: (collapsed: boolean) => void}} */
  let {
    worldEntry,
    countries,
    selectedEntry,
    mobileMode,
    drawerOpen,
    desktopNavigationCollapsed,
    onSelect,
    onCloseDrawer,
    onDrawerClosed,
    onDesktopCollapsedChange,
  } = $props();
  let query = $state('');
  let matches = $derived(filterCountries(countries, query));
  let groups = $derived(groupCountries(matches));
  const countryLetters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('');
  let globeFrame = $state(0);
  let globeSymbol = $derived(`frame-${String(globeFrame + 1).padStart(2, '0')}`);
  let drawerClosing = $state(false);
  /** @type {HTMLDialogElement | null} */
  let dialog = $state(null);
  /** @type {HTMLInputElement | null} */
  let searchInput = $state(null);
  /** @type {HTMLDivElement | null} */
  let countryGroupsScroll = $state(null);

  $effect(() => {
    const anchor = selectedEntry?.anchor;
    if (anchor)
      requestAnimationFrame(() =>
        document.getElementById(`country-link-${anchor}`)?.scrollIntoView({ block: 'nearest' }),
      );
  });

  // Wide screens show the panel in place; narrow screens present it as a modal drawer.
  $effect(() => {
    if (!dialog) return;
    if (!mobileMode) {
      if (dialog.matches(':modal')) dialog.close();
      // Setting `open` directly shows the panel without the focus move that show() performs.
      dialog.open = true;
    } else if (drawerOpen) {
      if (!dialog.open) dialog.showModal();
    } else if (dialog.open) {
      if (dialog.matches(':modal') && !prefersReducedMotion.current) drawerClosing = true;
      else dialog.close();
    }
  });

  /** @param {AnimationEvent} event */
  function finishDrawerClose(event) {
    if (event.target === dialog && event.animationName.endsWith('navigation-drawer-out'))
      dialog?.close();
  }

  function handleDialogClose() {
    drawerClosing = false;
    onDrawerClosed();
  }

  /** @param {GalleryEntry} entry @param {MouseEvent} event */
  function selectEntry(entry, event) {
    if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
      return;
    event.preventDefault();
    if (mobileMode) query = '';
    onSelect(entry);
  }

  /** @param {string} letter */
  function jumpToGroup(letter) {
    const section = document.getElementById(`country-group-${letter}`);
    if (!section || !countryGroupsScroll) return;
    const scrollBounds = countryGroupsScroll.getBoundingClientRect();
    const sectionBounds = section.getBoundingClientRect();
    countryGroupsScroll.scrollTo({
      top: countryGroupsScroll.scrollTop + sectionBounds.top - scrollBounds.top,
      behavior: prefersReducedMotion.current ? 'auto' : 'smooth',
    });
  }

  /** @param {KeyboardEvent} event */
  function handleKeydown(event) {
    // A native modal releases Tab focus to the browser chrome at its edges.
    if (event.key === 'Tab' && dialog?.matches(':modal')) {
      const focusable = [...dialog.querySelectorAll('button, input, a[href]')].filter(
        (element) => element.getClientRects().length > 0,
      );
      const edge = event.shiftKey ? focusable[0] : focusable.at(-1);
      const wrap = event.shiftKey ? focusable.at(-1) : focusable[0];
      if (
        wrap instanceof HTMLElement &&
        (document.activeElement === edge || !dialog.contains(document.activeElement))
      ) {
        event.preventDefault();
        wrap.focus();
      }
      return;
    }
    if (event.key !== 'Escape' || mobileMode || desktopNavigationCollapsed) return;
    event.preventDefault();
    onDesktopCollapsedChange(true);
  }

  /** @param {Event} event */
  function handleCancel(event) {
    event.preventDefault();
    onCloseDrawer();
  }

  async function expandAndFocusSearch() {
    onDesktopCollapsedChange(false);
    await tick();
    searchInput?.focus();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<dialog
  id="country-browser-panel"
  bind:this={dialog}
  class:closing={drawerClosing}
  class:desktop-collapsed={desktopNavigationCollapsed && !mobileMode}
  aria-labelledby="navigation-title"
  onclick={(event) => {
    if (event.target === dialog) onCloseDrawer();
  }}
  oncancel={handleCancel}
  onclose={handleDialogClose}
  onanimationend={finishDrawerClose}
>
  <header class="navigation-header">
    <svg class="drawer-globe" viewBox="0 0 512 512" aria-hidden="true">
      <use href={`${globeSprite}#${globeSymbol}`}></use>
    </svg>
    <h2 id="navigation-title">Country navigation</h2>
    <button
      class="desktop-collapse"
      type="button"
      aria-label="Collapse maps navigation"
      aria-controls="country-browser-panel"
      aria-expanded={!desktopNavigationCollapsed}
      onclick={() => onDesktopCollapsedChange(true)}
      ><ChevronLeft size={20} strokeWidth={2} aria-hidden="true" /></button
    >
    <button
      class="close"
      type="button"
      aria-controls="country-browser-panel"
      aria-expanded={drawerOpen}
      aria-label="Close map navigation"
      onclick={onCloseDrawer}><X size={20} strokeWidth={2} aria-hidden="true" /></button
    >
  </header>
  <div class="rail-controls">
    <button
      type="button"
      aria-label="Expand maps navigation"
      aria-controls="country-browser-panel"
      aria-expanded={!desktopNavigationCollapsed}
      onclick={() => onDesktopCollapsedChange(false)}
      ><ChevronRight size={20} strokeWidth={2} aria-hidden="true" /></button
    >
    <button type="button" aria-label="Search countries" onclick={expandAndFocusSearch}>
      <Search size={20} strokeWidth={2} aria-hidden="true" />
    </button>
  </div>
  <nav aria-label="Map navigation">
    <search aria-label="Filter map navigation">
      <label class="search-label" for="country-search">
        Search countries <span class="status">Matching: {matches.length}</span>
      </label>
      <div class="search-field">
        <input
          bind:this={searchInput}
          id="country-search"
          type="search"
          aria-label="Search countries"
          bind:value={query}
          placeholder="Search countries"
          autocomplete="off"
        />
        {#if query}
          <button
            class="clear-search"
            type="button"
            aria-label="Clear search"
            onclick={() => {
              query = '';
              searchInput?.focus();
            }}><X size={16} strokeWidth={2} aria-hidden="true" /></button
          >
        {/if}
      </div>
      <p class="visually-hidden" role="status" aria-live="polite">
        {query ? `Matching: ${matches.length}` : ''}
      </p>
    </search>
    <section class="map-links" aria-label="Map choices">
      <header class="map-links-header">
        <ul class="map-pinned-links">
          <li>
            <a
              class:active={selectedEntry?.anchor === worldEntry.anchor}
              aria-current={selectedEntry?.anchor === worldEntry.anchor ? 'page' : undefined}
              href={`#${worldEntry.anchor}`}
              onclick={(event) => selectEntry(worldEntry, event)}>{worldEntry.name}</a
            >
          </li>
        </ul>
        <div class="country-letter-index" role="group" aria-label="Jump to countries by letter">
          {#each countryLetters as letter (letter)}
            <button
              type="button"
              aria-label={`Jump to countries starting with ${letter}`}
              disabled={!groups[letter]}
              onclick={() => jumpToGroup(letter)}>{letter}</button
            >
          {/each}
        </div>
      </header>
      <div
        class="map-links-scroll"
        bind:this={countryGroupsScroll}
        onscroll={(event) => {
          globeFrame = Math.floor((event.currentTarget.scrollTop / 240) * 72) % 72;
        }}
      >
        {#each Object.entries(groups) as [letter, entries] (letter)}
          <section class:filtered={query.trim()} aria-labelledby={`country-group-${letter}`}>
            <h3 id={`country-group-${letter}`} class:visually-hidden={query.trim()}>{letter}</h3>
            <ul>
              {#each entries as entry (entry.anchor)}
                <li>
                  <a
                    id={`country-link-${entry.anchor}`}
                    class:active={selectedEntry?.anchor === entry.anchor}
                    aria-current={selectedEntry?.anchor === entry.anchor ? 'page' : undefined}
                    href={`#${entry.anchor}`}
                    onclick={(event) => selectEntry(entry, event)}>{entry.name}</a
                  >
                </li>
              {/each}
            </ul>
          </section>
        {:else}<p class="empty">No matching countries.</p>{/each}
      </div>
    </section>
  </nav>
</dialog>

<style>
  dialog {
    position: relative;
    display: flex;
    z-index: auto;
    grid-column: 1;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: calc(var(--navigation-header-space, 0px) + 1rem) 1rem
      calc(var(--footer-space, 0px) + 1rem);
    flex-direction: column;
    overflow: hidden;
    color: var(--text);
    background: var(--surface);
    border: 0;
    border-radius: 0;
    box-shadow: none;
    opacity: 1;
    transform: none;
  }
  dialog:not([open]) {
    display: none;
    opacity: 0;
    transform: translateX(-100%);
  }
  dialog[open] {
    display: flex;
    animation: none;
  }
  @starting-style {
    dialog[open] {
      opacity: 0;
      transform: translateX(-100%);
    }
  }
  dialog::backdrop {
    background: transparent;
    transition: background-color 180ms ease;
  }
  dialog:modal::backdrop {
    background: var(--chrome-bg);
  }
  label {
    display: block;
    margin: 0 0 0.5rem;
    font-weight: 600;
  }
  .search-label {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .status {
    margin-left: auto;
    text-align: right;
  }
  input {
    width: 100%;
    padding: 0.75rem 1rem;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    font: inherit;
  }
  .search-field {
    position: relative;
  }
  .search-field input {
    padding-right: 3rem;
  }
  .search-field input::-webkit-search-cancel-button {
    -webkit-appearance: none;
  }
  .clear-search {
    position: absolute;
    top: 50%;
    right: 0.35rem;
    display: grid;
    width: 2.25rem;
    height: 2.25rem;
    place-items: center;
    padding: 0;
    color: var(--muted);
    background: transparent;
    border: 0;
    border-radius: 50%;
    font-size: 1.25rem;
    transform: translateY(-50%);
  }
  nav {
    display: flex;
    flex: 1;
    height: auto;
    min-height: 0;
    flex-direction: column;
  }
  search {
    display: block;
    flex: none;
  }
  .navigation-header {
    display: flex;
    flex: none;
    height: 3rem;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }
  .navigation-header h2 {
    margin: 0 auto 0 0;
    font-size: 1rem;
  }
  .drawer-globe {
    display: none;
    width: 3rem;
    height: 3rem;
    margin-right: 0.5rem;
  }
  .navigation-header button,
  .rail-controls button {
    display: grid;
    width: 2.5rem;
    height: 2.5rem;
    place-items: center;
    padding: 0;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    font: inherit;
    cursor: pointer;
  }
  .navigation-header button:hover,
  .rail-controls button:hover {
    border-color: var(--accent);
  }
  .navigation-header .close {
    display: none;
    flex: none;
    width: 2.5rem;
    height: 2.5rem;
  }
  .rail-controls {
    display: none;
  }
  .map-links {
    display: flex;
    flex: 1;
    min-height: 0;
    flex-direction: column;
    margin-top: 0.75rem;
    overflow: hidden;
    transition:
      opacity 180ms ease,
      visibility 180ms allow-discrete;
  }
  .map-links-header {
    flex: none;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--line);
  }
  .map-links-scroll {
    flex: 1;
    min-height: 0;
    gap: 0.35rem;
    margin-top: 1rem;
    overflow-y: auto;
    /* Keeps the list position while the collapsing panel width reflows it. */
    overflow-anchor: none;
  }
  .map-pinned-links {
    flex: none;
    margin: 0;
    padding-left: 0;
    list-style: none;
  }
  .country-letter-index {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0;
    margin-top: 0.5rem;
  }
  .country-letter-index button {
    padding: 0;
    color: var(--accent);
    background: transparent;
    border: 0;
    font: inherit;
    font-size: 0.75rem;
    line-height: 1.75;
    cursor: pointer;
  }
  .country-letter-index button:not(:last-of-type)::after {
    margin-inline: 0.15rem;
    color: var(--muted);
    content: '・';
  }
  .country-letter-index button:disabled {
    color: var(--muted);
    opacity: 0.5;
    cursor: not-allowed;
  }
  .country-letter-index button:not(:disabled):hover {
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .desktop-collapsed {
    padding-inline: 0.4rem;
  }
  .desktop-collapsed nav > search,
  .desktop-collapsed .map-pinned-links,
  .desktop-collapsed .navigation-header {
    display: none;
  }
  .desktop-collapsed .map-links {
    opacity: 0;
    visibility: hidden;
  }
  .desktop-collapsed .rail-controls {
    display: grid;
    justify-items: center;
    gap: 0.5rem;
  }
  .map-links-scroll ul {
    display: grid;
    gap: 0.35rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .map-links-scroll h3 {
    margin: 0.75rem 0 0.25rem;
    color: var(--accent);
    font-size: 0.8rem;
  }
  .map-links-scroll section.filtered ul {
    margin-top: 0.35rem;
  }
  .map-links-scroll a,
  .map-pinned-links a {
    position: relative;
    display: block;
    padding: 0.25rem 0;
    color: var(--muted);
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  .map-links-scroll a.active,
  .map-pinned-links a.active {
    padding-left: 0.75rem;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border-radius: 0.25rem;
  }
  .map-links-scroll a.active::before,
  .map-pinned-links a.active::before {
    position: absolute;
    inset: 0.2rem auto 0.2rem 0;
    width: 3px;
    background: var(--accent);
    border-radius: 2px;
    content: '';
  }
  .map-links-scroll a:hover,
  .map-pinned-links a:hover,
  .map-links-scroll a.active,
  .map-pinned-links a.active {
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .status,
  .empty {
    color: var(--muted);
  }
  .status {
    font-size: 0.85rem;
  }
  button:focus-visible,
  input:focus-visible,
  a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  @keyframes navigation-drawer-in {
    from {
      opacity: 0.75;
      transform: translateX(-100%);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
  @keyframes navigation-drawer-out {
    from {
      opacity: 1;
      transform: translateX(0);
    }
    to {
      opacity: 0.75;
      transform: translateX(-100%);
    }
  }
  @media (max-width: 1023px) {
    dialog {
      position: fixed;
      z-index: 3;
      inset: 0;
      height: 100dvh;
      width: min(24rem, calc(100vw - 4rem));
      padding: max(1rem, env(safe-area-inset-top)) 1rem max(1rem, env(safe-area-inset-bottom));
      opacity: 0;
      transform: translateX(-100%);
      transition:
        display 400ms allow-discrete,
        overlay 400ms allow-discrete;
    }
    dialog[open] {
      display: flex;
      opacity: 1;
      transform: translateX(0);
      animation: navigation-drawer-in 400ms cubic-bezier(0.2, 0.8, 0.2, 1) both;
    }
    dialog.closing {
      opacity: 0;
      transform: translateX(-100%);
      animation: navigation-drawer-out 400ms cubic-bezier(0.2, 0.8, 0.2, 1) both;
    }
    .rail-controls {
      display: none;
    }
    .desktop-collapse {
      display: none;
    }
    .navigation-header .desktop-collapse {
      display: none;
    }
    .navigation-header {
      justify-content: space-between;
      margin-bottom: 0.75rem;
    }
    .drawer-globe {
      display: block;
    }
    .navigation-header .close {
      display: grid;
      width: 2.75rem;
      height: 2.75rem;
    }
    dialog::backdrop {
      background: var(--chrome-bg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    dialog,
    dialog::backdrop,
    .map-links {
      transition: none;
    }
    dialog {
      animation: none;
    }
  }
</style>
