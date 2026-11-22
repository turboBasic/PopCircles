<script lang="ts">
  import { onMount } from 'svelte';

  type Props = {
    src: string;
    alt: string;
    aspectRatio?: number;
    entryAnchor: string;
    loading: 'lazy' | 'eager';
  };

  let { src, alt, aspectRatio = 11 / 6, entryAnchor, loading }: Props = $props();
  let image = $state<HTMLImageElement | null>(null);
  let loaded = $state(false);
  let failed = $state(false);

  onMount(() => {
    if (image?.complete) {
      loaded = image.naturalWidth > 0;
      failed = image.naturalWidth === 0;
    }
  });
</script>

<a class="map-link" href={`#${entryAnchor}`}>
  <div class="map-frame" style:aspect-ratio={aspectRatio}>
    {#if !loaded && !failed}<span class="map-image-status">Loading map…</span>{/if}
    {#if failed}<span class="map-image-status" role="alert">Map image could not be loaded.</span
      >{/if}
    <img
      bind:this={image}
      class:loaded
      {src}
      {alt}
      {loading}
      onload={() => (loaded = true)}
      onerror={() => (failed = true)}
    />
  </div>
</a>

<style>
  .map-frame {
    position: relative;
    width: 100%;
    margin-inline: auto;
    overflow: hidden;
    background: var(--surface-2);
  }
  .map-link {
    display: block;
  }
  img {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0;
  }
  img.loaded {
    opacity: 1;
  }
  .map-image-status {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 1rem;
    color: var(--muted-foreground);
    text-align: center;
  }
</style>
