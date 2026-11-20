<script lang="ts">
  import CountryCard from '#lib/components/gallery/CountryCard.svelte';
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
  role="region"
  aria-label="Country and World workspace"
  tabindex="0"
  onscroll={onScroll}
  onblur={(event) => event.currentTarget.removeAttribute('data-auto-focused')}
>
  <section id="country-cards" aria-label="Country and World results">
    {#each entries as entry (entry.anchor)}
      <CountryCard
        {entry}
        isSelected={entry.anchor === (selectedEntry?.anchor ?? 'world-result')}
        {onCopy}
      />
    {/each}
  </section>
</div>

<style>
  #gallery {
    grid-column: 2;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: var(--header-space, 0px) 0 calc(var(--footer-space, 0px) + 2rem);
  }
  :global(#gallery[data-auto-focused]:focus) {
    outline: none;
  }
  @supports (scroll-timeline: --results-scroll block) and (timeline-scope: --results-scroll) and
    (animation-range: 0px 160px) {
    #gallery {
      scroll-timeline: --results-scroll block;
    }
  }
  @media (max-width: 1023px) {
    #gallery {
      grid-column: 1;
    }
  }
</style>
