<script>
  import { filterCountries } from '../gallery.js';

  let { worldEntry, countries, selectedEntry, mobileNavigation, desktopNavigationOpen, browserOpen = $bindable(), browserTrigger, onSelect } = $props();
  let query = $state('');
  let browser = $state(null);
  let browserPanel = $state(null);
  let countrySearch = $state(null);
  let matches = $derived(worldEntry ? [worldEntry, ...filterCountries(countries, query)] : []);

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
    if (
      mobileNavigation?.matches &&
      browserOpen &&
      !browser.contains(event.target) &&
      !browserTrigger.contains(event.target)
    ) closeBrowser();
  }

  function selectEntry(entry) {
    const returnFocus = mobileNavigation?.matches && browserOpen;
    closeBrowser();
    onSelect(entry);
    if (returnFocus) requestAnimationFrame(() => browserTrigger.focus());
  }

  $effect(() => {
    document.addEventListener('pointerdown', handlePointerdown);
    return () => document.removeEventListener('pointerdown', handlePointerdown);
  });
</script>

<aside class:collapsed={!mobileNavigation?.matches && !desktopNavigationOpen} aria-label="Map navigation">
  <details
    bind:this={browser}
    bind:open={browserOpen}
    ontoggle={() => {
      if (mobileNavigation?.matches && browser?.open) countrySearch?.focus();
    }}
  >
    <summary id="country-browser-trigger" aria-controls="country-browser-panel" aria-expanded={browserOpen}>
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
</aside>

<style>
  aside { display: flex; min-height: 0; flex-direction: column; overflow-y: auto; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  aside.collapsed { display: none; }
  aside details { display: flex; flex: 1; flex-direction: column; min-height: 0; }
  summary { display: flex; align-items: center; justify-content: space-between; gap: 1rem; cursor: pointer; font-weight: 700; }
  summary:focus-visible, input:focus-visible, a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .country-browser-panel { display: flex; flex: 1; flex-direction: column; min-height: 0; }
  .drawer-title { display: none; }
  label { display: block; margin: .8rem 0 .5rem; font-weight: 600; }
  input { width: 100%; padding: .75rem 1rem; color: var(--text); background: var(--bg); border: 1px solid var(--line); border-radius: .5rem; font: inherit; }
  nav { display: grid; flex: 1; gap: .35rem; margin-top: 1rem; }
  nav a { padding: .25rem 0; color: var(--muted); text-decoration: none; }
  nav a:hover, nav a.active { color: var(--text); text-decoration: underline; text-underline-offset: .2em; }
  .status { color: var(--muted); font-size: .85rem; }
  .empty { color: var(--muted); }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 800px) {
    aside { display: block; overflow: visible; padding: 0; border: 0; background: transparent; }
    aside summary { display: none; }
    .country-browser-panel { display: none; }
    details[open] .country-browser-panel { display: block; position: fixed; inset: 1rem; z-index: 5; max-height: none; overflow: auto; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; box-shadow: 0 1rem 3rem #0008; }
    .drawer-title { display: block; }
    nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
