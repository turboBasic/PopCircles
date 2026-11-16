<script>
  import { filterCountries } from '../gallery.js';

  /** @typedef {import('../gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{worldEntry: GalleryEntry, countries: GalleryEntry[], selectedEntry: GalleryEntry | null, mobileNavigation: MediaQueryList | null, drawerOpen: boolean, autoHide: boolean, mobileDialog: HTMLDialogElement | null, browserTrigger: HTMLButtonElement | null, onSelect: (entry: GalleryEntry) => void, onCloseDrawer: () => void, onToggleAutoHide: () => void}} */
  let {
    worldEntry,
    countries,
    selectedEntry,
    mobileNavigation,
    drawerOpen,
    autoHide,
    mobileDialog = $bindable(),
    browserTrigger,
    onSelect,
    onCloseDrawer,
    onToggleAutoHide,
  } = $props();
  let query = $state('');
  let matches = $derived(worldEntry ? [worldEntry, ...filterCountries(countries, query)] : []);
  let restoreFocus = false;

  function closeDrawer() {
    restoreFocus = mobileNavigation?.matches ?? false;
    mobileDialog?.close();
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
    if (event.key !== 'Escape' || !drawerOpen) return;
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
  class:open={drawerOpen}
  aria-labelledby="country-browser-title"
  onpointerleave={handlePointerLeave}
  oncancel={handleCancel}
  onclose={handleDialogClose}
>
  <button class="close" type="button" aria-label="Close map navigation" onclick={closeDrawer}
    >☰</button
  >
  <h2 id="country-browser-title">Browse maps</h2>
  <nav aria-label="Map navigation">
    <label for="country-search">Search maps <span class="status">{matches.length} maps</span></label
    >
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
      {#each matches as entry (entry.anchor)}
        <a
          class:active={selectedEntry?.anchor === entry.anchor}
          aria-current={selectedEntry?.anchor === entry.anchor ? 'page' : undefined}
          href={`#${entry.anchor}`}
          onclick={() => selectEntry(entry)}>{entry.name}</a
        >
      {:else}<p class="empty">No matching maps.</p>{/each}
    </div>
  </nav>
</dialog>

<style>
  dialog {
    position: fixed;
    z-index: 3;
    inset: var(--navigation-header-space, 0px) auto var(--footer-space, 0px)
      max(1rem, calc((100vw - 1600px) / 2));
    width: min(20rem, calc(100vw - 2rem));
    max-width: none;
    height: auto;
    max-height: none;
    margin: 0;
    padding: 1rem;
    overflow: hidden;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 0;
    box-shadow: 0 1rem 3rem #0005;
  }
  dialog:not(.open) {
    display: none;
  }
  dialog::backdrop {
    background: transparent;
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
  .map-links {
    display: grid;
    flex: 1;
    min-height: 0;
    gap: 0.35rem;
    margin-top: 1rem;
    overflow-y: auto;
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
  @media (max-width: 800px) {
    dialog {
      inset: 0;
      width: min(24rem, calc(100vw - 3rem));
      height: 100dvh;
      padding-top: calc(var(--navigation-header-space, 0px) + 1rem);
    }
    .close {
      display: block;
      top: 0.75rem;
      right: auto;
      left: 0.75rem;
    }
    dialog::backdrop {
      background: #0008;
    }
  }
</style>
