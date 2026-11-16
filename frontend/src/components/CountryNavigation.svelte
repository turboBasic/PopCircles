<script>
  import { countCountryMatches, filterCountries, groupCountries } from '../gallery.js';

  /** @typedef {import('../gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{worldEntry: GalleryEntry, countries: GalleryEntry[], selectedEntry: GalleryEntry | null, mobileMode: boolean, drawerOpen: boolean, desktopNavigationCollapsed: boolean, autoHide: boolean, mobileDialog: HTMLDialogElement | null, browserTrigger: HTMLButtonElement | null, onSelect: (entry: GalleryEntry) => void, onCloseDrawer: () => void, onToggleAutoHide: () => void, onToggleDesktopNavigation: () => void, onExpandAndFocusSearch: () => void}} */
  let {
    worldEntry,
    countries,
    selectedEntry,
    mobileMode,
    drawerOpen,
    desktopNavigationCollapsed,
    autoHide,
    mobileDialog = $bindable(),
    browserTrigger,
    onSelect,
    onCloseDrawer,
    onToggleAutoHide,
    onToggleDesktopNavigation,
    onExpandAndFocusSearch,
  } = $props();
  let query = $state('');
  let matches = $derived(filterCountries(countries, query));
  let groups = $derived(groupCountries(matches));
  let matchCount = $derived(countCountryMatches(countries, query));
  let restoreFocus = false;

  $effect(() => {
    const anchor = selectedEntry?.anchor;
    if (anchor)
      requestAnimationFrame(() =>
        document.getElementById(`country-link-${anchor}`)?.scrollIntoView({ block: 'nearest' }),
      );
  });

  function closeDrawer() {
    restoreFocus = mobileMode;
    if (restoreFocus) mobileDialog?.close();
    onCloseDrawer();
  }

  /** @param {GalleryEntry} entry */
  function selectEntry(entry) {
    if (autoHide) closeDrawer();
    onSelect(entry);
  }

  /** @param {PointerEvent} event */
  function handlePointerLeave(event) {
    if (
      event.pointerType !== 'touch' &&
      autoHide &&
      !mobileDialog?.contains(document.activeElement)
    ) {
      closeDrawer();
    }
  }

  /** @param {KeyboardEvent} event */
  function handleKeydown(event) {
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
    restoreFocus = false;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<dialog
  id="country-browser-panel"
  bind:this={mobileDialog}
  class:open={!mobileMode || drawerOpen}
  class:desktop-collapsed={desktopNavigationCollapsed && !mobileMode}
  aria-labelledby="country-browser-title"
  onpointerleave={handlePointerLeave}
  oncancel={handleCancel}
  onclose={handleDialogClose}
>
  <div class="desktop-controls">
    <button
      class="desktop-collapse"
      type="button"
      aria-label="Collapse maps navigation"
      aria-controls="country-browser-panel"
      aria-expanded={!desktopNavigationCollapsed}
      onclick={onToggleDesktopNavigation}>‹</button
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
    <button type="button" aria-label="Search maps" onclick={onExpandAndFocusSearch}>⌕</button>
  </div>
  <button
    class="close"
    type="button"
    aria-controls="country-browser-panel"
    aria-expanded={drawerOpen}
    aria-label="Close map navigation"
    onclick={closeDrawer}>☰</button
  >
  <h2 id="country-browser-title">Browse maps</h2>
  <nav aria-label="Map navigation">
    <label for="country-search">
      Search maps <span class="status"
        >{matchCount} {matchCount === 1 ? 'country' : 'countries'}</span
      >
    </label>
    <input
      id="country-search"
      type="search"
      aria-label="Search maps"
      bind:value={query}
      placeholder="Search maps"
      autocomplete="off"
    />
    <p class="visually-hidden" role="status" aria-live="polite">
      {query ? `${matches.length} matching maps` : ''}
    </p>
    <label class="auto-hide">
      <input type="checkbox" role="switch" checked={autoHide} onchange={onToggleAutoHide} />
      Auto-hide drawer
    </label>
    <div class="map-links">
      <a
        class:active={selectedEntry === null}
        aria-current={selectedEntry === null ? 'page' : undefined}
        href={`${window.location.pathname}${window.location.search}`}>All countries</a
      >
      <a
        class:active={selectedEntry?.anchor === worldEntry.anchor}
        aria-current={selectedEntry?.anchor === worldEntry.anchor ? 'page' : undefined}
        href={`#${worldEntry.anchor}`}
        onclick={() => selectEntry(worldEntry)}>{worldEntry.name}</a
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
                  onclick={() => selectEntry(entry)}>{entry.name}</a
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
    z-index: auto;
    grid-column: 1;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: calc(var(--navigation-header-space, 0px) + 1rem) 1rem
      calc(var(--footer-space, 0px) + 1rem);
    overflow: hidden;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 0.8rem;
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
    display: block;
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
  h2 {
    margin: 0;
  }
  label {
    display: block;
    margin: 0.8rem 0 0.5rem;
    font-weight: 600;
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
  nav {
    display: flex;
    height: 100%;
    min-height: 0;
    flex-direction: column;
  }
  .desktop-controls {
    position: absolute;
    top: calc(var(--navigation-header-space, 0px) + 0.65rem);
    right: 1rem;
  }
  .desktop-controls button,
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
  }
  .desktop-collapsed {
    padding-inline: 0.4rem;
  }
  .desktop-collapsed h2,
  .desktop-collapsed nav > label,
  .desktop-collapsed nav > input,
  .desktop-collapsed nav > .visually-hidden,
  .desktop-collapsed nav > .auto-hide,
  .desktop-collapsed .map-links,
  .desktop-collapsed .desktop-controls {
    display: none;
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
    color: var(--muted);
    font-size: 0.8rem;
  }
  .map-links section.filtered ul {
    margin-top: 0.35rem;
  }
  .map-links a {
    padding: 0.25rem 0;
    color: var(--muted);
    text-decoration: none;
  }
  .map-links a:hover,
  .map-links a.active {
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
  .auto-hide {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .auto-hide input {
    width: auto;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    accent-color: var(--accent);
  }
  .close {
    display: none;
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
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
  @media (max-width: 1023px) {
    dialog {
      position: fixed;
      z-index: 3;
      inset: 0;
      width: min(24rem, calc(100vw - 2rem));
      height: 100dvh;
      padding: max(1rem, env(safe-area-inset-top)) 1rem max(1rem, env(safe-area-inset-bottom));
      opacity: 0;
      transform: translateX(-100%);
      transition:
        opacity 180ms ease,
        transform 180ms ease,
        display 180ms allow-discrete,
        overlay 180ms allow-discrete;
    }
    dialog.open {
      opacity: 1;
      transform: translateX(0);
    }
    .desktop-controls,
    .rail-controls {
      display: none;
    }
    .close {
      display: block;
      top: 0.75rem;
      right: auto;
      left: 0.75rem;
    }
    dialog::backdrop {
      background: var(--chrome-bg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    dialog,
    dialog::backdrop {
      transition: none;
    }
  }
</style>
