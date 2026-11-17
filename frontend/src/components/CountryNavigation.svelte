<script>
  import globeSprite from '../../assets/earth-globe.svg?url';
  import { countCountryMatches, countryUrl, filterCountries, groupCountries } from '../gallery.js';

  /** @typedef {import('../gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{worldEntry: GalleryEntry, countries: GalleryEntry[], selectedEntry: GalleryEntry | null, mobileMode: boolean, drawerOpen: boolean, desktopNavigationCollapsed: boolean, mobileDialog: HTMLDialogElement | null, browserTrigger: HTMLButtonElement | null, onSelect: (entry: GalleryEntry) => void, onCloseDrawer: () => void, onToggleDesktopNavigation: () => void, onExpandAndFocusSearch: () => void}} */
  let {
    worldEntry,
    countries,
    selectedEntry,
    mobileMode,
    drawerOpen,
    desktopNavigationCollapsed,
    mobileDialog = $bindable(),
    browserTrigger,
    onSelect,
    onCloseDrawer,
    onToggleDesktopNavigation,
    onExpandAndFocusSearch,
  } = $props();
  let query = $state('');
  let matches = $derived(filterCountries(countries, query));
  let groups = $derived(groupCountries(matches));
  let matchCount = $derived(countCountryMatches(countries, query));
  let globeFrame = $state(0);
  let globeSymbol = $derived(`frame-${String(globeFrame + 1).padStart(2, '0')}`);
  let drawerClosing = $state(false);
  /** @type {GalleryEntry | null} */
  let pendingSelection = null;
  let restoreFocus = false;

  $effect(() => {
    const anchor = selectedEntry?.anchor;
    if (anchor)
      requestAnimationFrame(() =>
        document.getElementById(`country-link-${anchor}`)?.scrollIntoView({ block: 'nearest' }),
      );
  });

  /** @param {boolean} returnFocus */
  function closeDrawer(returnFocus = true) {
    restoreFocus = mobileMode && returnFocus;
    if (mobileMode && mobileDialog?.open) {
      if (matchMedia('(prefers-reduced-motion: reduce)').matches) mobileDialog.close();
      else drawerClosing = true;
    }
    onCloseDrawer();
  }

  /** @param {AnimationEvent} event */
  function finishDrawerClose(event) {
    if (event.target !== mobileDialog || !event.animationName.endsWith('navigation-drawer-out'))
      return;
    drawerClosing = false;
    mobileDialog?.close();
  }

  /** @param {GalleryEntry} entry @param {MouseEvent} event */
  function selectEntry(entry, event) {
    if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
      return;
    event.preventDefault();
    const url = countryUrl(window.location.href, entry.anchor);
    if (url !== window.location.href) window.history.pushState(null, '', url);
    if (mobileMode) {
      query = '';
      pendingSelection = entry;
      closeDrawer(false);
      return;
    }
    onSelect(entry);
  }

  /** @param {MouseEvent} event */
  function handleDialogClick(event) {
    if (event.target === mobileDialog) closeDrawer();
  }

  /** @param {KeyboardEvent} event */
  function handleKeydown(event) {
    if (mobileMode && drawerOpen && event.key === 'Tab') {
      const focusable = [
        ...(mobileDialog?.querySelectorAll('button, input, a[href]') ?? []),
      ].filter((element) => element.getClientRects().length > 0);
      const first = focusable[0] instanceof HTMLElement ? focusable[0] : null;
      const lastCandidate = focusable.at(-1);
      const last = lastCandidate instanceof HTMLElement ? lastCandidate : null;
      if (
        event.shiftKey &&
        (document.activeElement === first || !mobileDialog?.contains(document.activeElement))
      ) {
        event.preventDefault();
        last?.focus();
      } else if (
        !event.shiftKey &&
        (document.activeElement === last || !mobileDialog?.contains(document.activeElement))
      ) {
        event.preventDefault();
        first?.focus();
      }
      return;
    }
    if (event.key !== 'Escape' || (mobileMode ? !drawerOpen : desktopNavigationCollapsed)) return;
    event.preventDefault();
    closeDrawer();
  }

  /** @param {Event} event */
  function handleCancel(event) {
    event.preventDefault();
    closeDrawer();
  }

  function handleDialogClose() {
    if (restoreFocus) requestAnimationFrame(() => browserTrigger?.focus());
    drawerClosing = false;
    restoreFocus = false;
    if (pendingSelection) onSelect(pendingSelection);
    pendingSelection = null;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<dialog
  id="country-browser-panel"
  bind:this={mobileDialog}
  class:open={!mobileMode || drawerOpen || drawerClosing}
  class:closing={drawerClosing}
  class:desktop-collapsed={desktopNavigationCollapsed && !mobileMode}
  aria-label="Country navigation"
  tabindex="-1"
  autofocus
  onclick={handleDialogClick}
  oncancel={handleCancel}
  onclose={handleDialogClose}
  onanimationend={finishDrawerClose}
>
  <div class="navigation-header">
    <svg class="drawer-globe" viewBox="0 0 512 512" aria-hidden="true">
      <use href={`${globeSprite}#${globeSymbol}`}></use>
    </svg>
    <button
      class="desktop-collapse"
      type="button"
      aria-label="Collapse maps navigation"
      aria-controls="country-browser-panel"
      aria-expanded={!desktopNavigationCollapsed}
      onclick={onToggleDesktopNavigation}>‹</button
    >
    <button
      class="close"
      id="country-browser-close"
      type="button"
      aria-controls="country-browser-panel"
      aria-expanded={drawerOpen}
      aria-label="Close map navigation"
      onclick={() => closeDrawer()}>×</button
    >
  </div>
  <div class="rail-controls">
    <button
      type="button"
      aria-label="Expand maps navigation"
      aria-controls="country-browser-panel"
      aria-expanded={!desktopNavigationCollapsed}
      onclick={onToggleDesktopNavigation}>›</button
    >
    <button type="button" aria-label="Search countries" onclick={onExpandAndFocusSearch}>⌕</button>
  </div>
  <nav aria-label="Map navigation">
    <label class="search-label" for="country-search">
      Search countries <span class="status">Matching: {matchCount}</span>
    </label>
    <div class="search-field">
      <input
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
            document.getElementById('country-search')?.focus();
          }}>×</button
        >
      {/if}
    </div>
    <p class="visually-hidden" role="status" aria-live="polite">
      {query ? `Matching: ${matches.length}` : ''}
    </p>
    <div class="map-pinned-links">
      <a
        class:active={selectedEntry?.anchor === worldEntry.anchor}
        aria-current={selectedEntry?.anchor === worldEntry.anchor ? 'page' : undefined}
        href={`#${worldEntry.anchor}`}
        onclick={(event) => selectEntry(worldEntry, event)}>{worldEntry.name}</a
      >
    </div>
    <div
      class="map-links"
      onscroll={(event) => {
        globeFrame = Math.floor((event.currentTarget.scrollTop / 240) * 72) % 72;
      }}
    >
      {#each Object.entries(groups) as [letter, entries] (letter)}
        <section class:filtered={query.trim()} aria-label={letter}>
          {#if !query.trim()}<h3>{letter}</h3>{/if}
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
  dialog:not(.open) {
    display: none;
    opacity: 0;
    transform: translateX(-100%);
  }
  dialog.open {
    display: flex;
    animation: none;
  }
  @starting-style {
    dialog.open {
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
  .navigation-header {
    display: flex;
    flex: none;
    height: 3rem;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }
  .drawer-globe {
    display: none;
    width: 3rem;
    height: 3rem;
    margin-right: auto;
  }
  .navigation-header button,
  .rail-controls button {
    width: 2.5rem;
    height: 2.5rem;
    padding: 0;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    font: inherit;
    font-size: 1.25rem;
    cursor: pointer;
  }
  .rail-controls {
    display: none;
  }
  .map-links {
    flex: 1;
    min-height: 0;
    gap: 0.35rem;
    margin-top: 1rem;
    overflow-y: auto;
    transition:
      opacity 180ms ease,
      visibility 180ms allow-discrete;
  }
  .map-pinned-links {
    flex: none;
    margin-top: 0.75rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--line);
  }
  .desktop-collapsed {
    padding-inline: 0.4rem;
  }
  .desktop-collapsed nav > label,
  .desktop-collapsed nav > .search-field,
  .desktop-collapsed nav > .visually-hidden,
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
  .map-links ul {
    display: grid;
    gap: 0.35rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .map-links h3 {
    margin: 0.75rem 0 0.25rem;
    color: var(--accent);
    font-size: 0.8rem;
  }
  .map-links section.filtered ul {
    margin-top: 0.35rem;
  }
  .map-links a,
  .map-pinned-links a {
    position: relative;
    display: block;
    padding: 0.25rem 0;
    color: var(--muted);
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  .map-links a.active,
  .map-pinned-links a.active {
    padding-left: 0.75rem;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border-radius: 0.25rem;
  }
  .map-links a.active::before,
  .map-pinned-links a.active::before {
    position: absolute;
    inset: 0.2rem auto 0.2rem 0;
    width: 3px;
    background: var(--accent);
    border-radius: 2px;
    content: '';
  }
  .map-links a:hover,
  .map-pinned-links a:hover,
  .map-links a.active,
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
  .close {
    display: none;
    flex: none;
    width: 2.5rem;
    height: 2.5rem;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    font: inherit;
    cursor: pointer;
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
    dialog.open {
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
    .navigation-header {
      justify-content: space-between;
      margin-bottom: 0.75rem;
    }
    .drawer-globe {
      display: block;
    }
    .close {
      display: block;
      width: 2.75rem;
      height: 2.75rem;
    }
    .navigation-header .close {
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
