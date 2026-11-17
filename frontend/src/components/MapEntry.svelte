<script>
  import MapFigure from './MapFigure.svelte';

  /** @typedef {import('../gallery.js').GalleryEntry} GalleryEntry */

  /** @type {{entry: GalleryEntry, isSelected: boolean, onCopy: (event: MouseEvent, anchor: string) => Promise<void>}} */
  let { entry, isSelected, onCopy } = $props();
</script>

<section class="country" id={entry.anchor}>
  <header>
    <h2 id={`map-heading-${entry.anchor}`} tabindex="-1">
      <a href={`#${entry.anchor}`}>{entry.name}</a>
    </h2>
  </header>
  {#if entry.missing}
    <p class="empty">No 50% result is available for this country.</p>
  {:else}
    <div class="figure-grid">
      {#each entry.figures as figure (figure.map)}
        <MapFigure
          {figure}
          entryAnchor={entry.anchor}
          countryAnchor={entry.anchor === 'world-result' ? null : entry.anchor}
          loading={isSelected ? 'eager' : 'lazy'}
          {onCopy}
        />
      {/each}
    </div>
  {/if}
</section>

<style>
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
  #world-result .figure-grid {
    grid-template-columns: minmax(0, 1fr);
  }
</style>
