<script>
  import { filterCountries } from '../gallery.js';

  let { worldEntry, countries, selectedEntry, mobileNavigation, desktopNavigationOpen, browserOpen = $bindable(), mobileDialog = $bindable(), browserTrigger, onSelect } = $props();
  let query = $state('');
  let matches = $derived(worldEntry ? [worldEntry, ...filterCountries(countries, query)] : []);

  function closeBrowser() {
    if (!mobileNavigation?.matches || !mobileDialog?.open) return;
    mobileDialog.close();
    browserOpen = false;
  }

  function selectEntry(entry) {
    closeBrowser();
    onSelect(entry);
  }

  function handleDialogClose() {
    if (!mobileNavigation?.matches) return;
    browserOpen = false;
    requestAnimationFrame(() => browserTrigger.focus());
  }
</script>

{#snippet mapLinks()}
  <nav aria-label="Maps">
    {#each matches as entry (entry.anchor)}
      <a class:active={selectedEntry?.anchor === entry.anchor} aria-current={selectedEntry?.anchor === entry.anchor ? 'page' : undefined} href={`#${entry.anchor}`} onclick={() => selectEntry(entry)}>{entry.name}</a>
    {:else}<p class="empty">No matching maps.</p>{/each}
  </nav>
{/snippet}

<aside class:collapsed={!mobileNavigation?.matches && !desktopNavigationOpen} aria-label="Map navigation">
  <label for="country-search-desktop">Search maps <span class="status">{matches.length} maps</span></label>
  <input id="country-search-desktop" type="search" bind:value={query} placeholder="Search maps" autocomplete="off" />
  <p class="visually-hidden" role="status" aria-live="polite">{query ? `${matches.length} matching maps` : ''}</p>
  <div id="country-browser-desktop">{@render mapLinks()}</div>
</aside>

<dialog id="country-browser-panel" bind:this={mobileDialog} aria-labelledby="country-browser-title" onclose={handleDialogClose}>
  <button class="close" type="button" onclick={closeBrowser}>Close maps</button>
  <h2 id="country-browser-title">Browse maps</h2>
  <label for="country-search-mobile">Search maps</label>
  <input id="country-search-mobile" type="search" bind:value={query} placeholder="Search maps" autocomplete="off" autofocus />
  <p class="visually-hidden" role="status" aria-live="polite">{query ? `${matches.length} matching maps` : ''}</p>
  {@render mapLinks()}
</dialog>

<style>
  aside { display: flex; min-height: 0; flex-direction: column; overflow-y: auto; padding: 1rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  aside.collapsed { display: none; }
  input:focus-visible, a:focus-visible, button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  label { display: block; margin: .8rem 0 .5rem; font-weight: 600; }
  input { width: 100%; padding: .75rem 1rem; color: var(--text); background: var(--bg); border: 1px solid var(--line); border-radius: .5rem; font: inherit; }
  nav { display: grid; flex: 1; gap: .35rem; margin-top: 1rem; }
  nav a { padding: .25rem 0; color: var(--muted); text-decoration: none; }
  nav a:hover, nav a.active { color: var(--text); text-decoration: underline; text-underline-offset: .2em; }
  .status { color: var(--muted); font-size: .85rem; }
  .empty { color: var(--muted); }
  dialog { width: min(34rem, calc(100vw - 2rem)); max-height: calc(100dvh - 2rem); overflow: auto; padding: 1rem; color: var(--text); background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; box-shadow: 0 1rem 3rem #0008; }
  dialog::backdrop { background: #0008; }
  dialog nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .close { float: right; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 800px) {
    aside { display: block; overflow: visible; padding: 0; border: 0; background: transparent; }
    aside { display: none; }
  }
</style>
