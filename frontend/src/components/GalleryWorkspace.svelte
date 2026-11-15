<script>
  import { visibleEntries } from '../gallery.js';
  import MapFigure from './MapFigure.svelte';

  /** @typedef {import('../gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{entries: GalleryEntry[], selectedEntry: GalleryEntry | null, resultsPanel: HTMLDivElement | null, onSelect: (entry: GalleryEntry) => void, onScroll: () => void, onCopy: (event: MouseEvent, anchor: string) => Promise<void>}} */
  let { entries, selectedEntry, resultsPanel = $bindable(), onSelect, onScroll, onCopy } = $props();
  let visibleMapEntries = $derived(visibleEntries(entries, selectedEntry?.anchor ?? null));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (keyboard users need to focus the results scroll region) -->
<div
  bind:this={resultsPanel}
  class="results"
  role="region"
  aria-label="Map results"
  tabindex="0"
  onscroll={onScroll}
>
  <section class="map-results" aria-label="Maps">
    <div id="map-results-list">
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
                  loading={entry.anchor === (selectedEntry?.anchor ?? 'world-result')
                    ? 'eager'
                    : 'lazy'}
                  {onCopy}
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
  .results {
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 2rem;
  }
  .country {
    scroll-margin-top: 1rem;
    margin-bottom: 0.8rem;
    padding: 1.25rem;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 0.8rem;
  }
  .empty {
    color: var(--muted);
  }
  .country h2 a {
    color: inherit;
    text-decoration: none;
  }
  .country h2 a:hover {
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
  .figure-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr));
    gap: 1rem;
  }
</style>
