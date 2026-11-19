<script lang="ts">
  import MapFigure from '#lib/components/gallery/MapFigure.svelte';
  import type { GalleryEntry } from '#lib/types/gallery';

  type Props = {
    entry: GalleryEntry;
    isSelected: boolean;
    onCopy: (event: MouseEvent, anchor: string) => Promise<void>;
  };

  let { entry, isSelected, onCopy }: Props = $props();
</script>

<section class="country" class:country-result={entry.anchor !== 'world-result'} id={entry.anchor}>
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
    padding: 1.25rem var(--country-inline-padding);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 0.8rem;
  }
  .country-result {
    border: 0;
  }
  :global(.country-result + .country-result) {
    border-top: 1px dotted var(--line);
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
  @media (max-width: 1023px) {
    .country {
      padding-block: 0.5rem;
    }
    .country h2 {
      margin-block-start: 0.4em;
    }
  }
</style>
