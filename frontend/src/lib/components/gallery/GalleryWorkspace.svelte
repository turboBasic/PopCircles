<script lang="ts">
  import MapEntry from '#lib/components/gallery/MapEntry.svelte';
  import type { GalleryEntry } from '#lib/types/gallery';

  type Props = {
    entries: GalleryEntry[];
    selectedEntry: GalleryEntry | null;
    resultsPanel: HTMLDivElement | null;
    onScroll: () => void;
    onCopy: (event: MouseEvent, anchor: string) => Promise<void>;
  };

  let { entries, selectedEntry, resultsPanel = $bindable(), onScroll, onCopy }: Props = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (keyboard users need to focus the results scroll region) -->
<div
  id="gallery"
  bind:this={resultsPanel}
  class="results"
  role="region"
  aria-label="Map results"
  tabindex="0"
  onscroll={onScroll}
>
  <section class="map-results" aria-label="Maps">
    {#each entries as entry (entry.anchor)}
      <MapEntry
        {entry}
        isSelected={entry.anchor === (selectedEntry?.anchor ?? 'world-result')}
        {onCopy}
      />
    {/each}
  </section>
</div>

<style>
  .results {
    grid-column: 2;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: var(--header-space, 0px) 0 calc(var(--footer-space, 0px) + 2rem);
  }
  @supports (scroll-timeline: --results-scroll block) and (timeline-scope: --results-scroll) and
    (animation-range: 0px 160px) {
    .results {
      scroll-timeline: --results-scroll block;
    }
  }
  @media (max-width: 1023px) {
    .results {
      grid-column: 1;
    }
  }
</style>
