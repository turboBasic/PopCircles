<script lang="ts">
  import MapCaption from './MapCaption.svelte';
  import MapImage from './MapImage.svelte';
  import type { GalleryFigure } from '#lib/types/gallery';

  type Props = {
    figure: GalleryFigure;
    entryAnchor: string;
    countryAnchor?: string | null;
    loading?: 'lazy' | 'eager';
    onCopy: (event: MouseEvent, anchor: string) => Promise<void>;
  };

  let { figure, entryAnchor, countryAnchor = null, loading = 'lazy', onCopy }: Props = $props();
</script>

<figure class="map-card">
  <MapImage src={figure.map} alt={figure.alt} {entryAnchor} {loading} />
  <figcaption>
    <MapCaption caption={figure.caption} document={figure.document} {countryAnchor} {onCopy} />
  </figcaption>
</figure>

<style>
  .map-card {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.8rem;
  }
  figcaption {
    padding: 0;
    color: var(--muted-foreground);
    font-size: 0.85rem;
  }
</style>
