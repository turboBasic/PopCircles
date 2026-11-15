<script>
  import { visibleEntries } from '../gallery.js';
  import MapFigure from './MapFigure.svelte';

  let { entries, selectedEntry, resultsPanel = $bindable(), onSelect, onScroll, onCopy } = $props();
  let visibleMapEntries = $derived(visibleEntries(entries, selectedEntry?.anchor ?? null));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex: keyboard users need to focus this scroll region. -->
<div
  bind:this={resultsPanel}
  class="results"
  role="region"
  aria-label="Map results"
  tabindex="0"
  onscroll={onScroll}
>
  <section class="map-results" aria-label="Maps">
    <div id="map-results-list" class:gallery-mode={selectedEntry === null} class:single-map={selectedEntry !== null}>
      {#each visibleMapEntries as entry (entry.anchor)}
        <section class="country" id={entry.anchor}>
          <h2><a href={`#${entry.anchor}`} onclick={() => onSelect(entry)}>{entry.name}</a></h2>
          {#if entry.missing}
            <p class="empty">No 50% result is available for this country.</p>
          {:else}
            <div class="figure-grid">
              {#each entry.figures as figure (figure.map)}
                <MapFigure
                  {figure}
                  countryAnchor={entry.anchor === 'world-result' ? null : entry.anchor}
                  loading={selectedEntry?.anchor === entry.anchor ? 'eager' : 'lazy'}
                  onCopy={onCopy}
                />
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  </section>
</div>

<style>
  .results { position: sticky; top: calc(4rem + 5rem * (1 - var(--header-shrink))); min-width: 0; height: calc(100vh - (4rem + 5rem * (1 - var(--header-shrink)))); overflow-y: auto; padding-bottom: 4rem; }
  .country { scroll-margin-top: calc(5rem + 4rem * (1 - var(--header-shrink))); margin-bottom: .8rem; padding: 1.25rem; background: var(--surface); border: 1px solid var(--line); border-radius: .8rem; }
  .empty { color: var(--muted); }
  .gallery-mode { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr)); gap: .8rem; }
  .gallery-mode #world-result { grid-column: 1 / -1; }
  .gallery-mode .country { margin: 0; }
  .gallery-mode .figure-grid { grid-template-columns: minmax(0, 1fr); }
  .country h2 a { color: inherit; text-decoration: none; }
  .country h2 a:hover { text-decoration: underline; text-underline-offset: .2em; }
  .figure-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr)); gap: 1rem; }
  @media (max-width: 800px) { .results { position: static; height: auto; overflow: visible; } }
</style>
