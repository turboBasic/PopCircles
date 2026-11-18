<script>
  import { Copy, ExternalLink, FileText } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { countryUrl, MAP_ASPECT_RATIO, territorySharePercent } from '../gallery.js';

  /** @typedef {import('../gallery.js').GalleryFigure} GalleryFigure */
  /** @typedef {import('../gallery.js').FigureCaption} FigureCaption */

  /** @type {{figure: GalleryFigure, entryAnchor: string, countryAnchor?: string | null, loading?: 'lazy' | 'eager', onCopy: (event: MouseEvent, anchor: string) => Promise<void>}} */
  let { figure, entryAnchor, countryAnchor = null, loading = 'lazy', onCopy } = $props();
  /** @type {HTMLImageElement | null} */
  let image = $state(null);
  let loaded = $state(false);
  let failed = $state(false);

  /** @param {FigureCaption} caption */
  function formatFigureCaption(caption) {
    const centreLat = caption.centre_lat.toFixed(4);
    const centreLon = caption.centre_lon.toFixed(4);
    return {
      radius: `${caption.radius_km.toLocaleString()} km`,
      population: caption.population.toLocaleString('en-US', { maximumFractionDigits: 0 }),
      share: `${(caption.share * 100).toFixed(1)}%`,
      territoryShare: territorySharePercent(caption.territory_share),
      territoryName: caption.territory_name,
      centre: `${centreLat}, ${centreLon}`,
      centreUrl: `https://www.openstreetmap.org/?mlat=${centreLat}&mlon=${centreLon}#map=12/${centreLat}/${centreLon}`,
      settlement: caption.settlement
        ? {
            name: caption.settlement.name,
            country: caption.settlement.country,
            distance: Math.round(caption.settlement.distance_km).toLocaleString(),
          }
        : null,
    };
  }
  let captionView = $derived(figure.caption ? formatFigureCaption(figure.caption) : null);

  onMount(() => {
    if (image?.complete) {
      loaded = image.naturalWidth > 0;
      failed = image.naturalWidth === 0;
    }
  });
</script>

<figure>
  <a class="map-link" href={`#${entryAnchor}`}>
    <div class="map-frame" style:aspect-ratio={MAP_ASPECT_RATIO}>
      {#if !loaded && !failed}<span class="image-status">Loading map…</span>{/if}
      {#if failed}<span class="image-status" role="alert">Map image could not be loaded.</span>{/if}
      <img
        bind:this={image}
        class:loaded
        src={figure.map}
        alt={figure.alt}
        {loading}
        onload={() => (loaded = true)}
        onerror={() => (failed = true)}
      />
    </div>
  </a>
  <figcaption>
    {#if captionView}
      <div class="figure-caption">
        <p>
          radius: <strong>{captionView.radius}</strong> · population:
          <strong>{captionView.population}</strong>
          / <strong>{captionView.share}</strong> ·
          {#if captionView.territoryShare !== null}
            Circle covers <strong>{captionView.territoryShare}%</strong> of {captionView.territoryName}
            ·
          {/if}
          centre:
          <a class="caption-link" href={captionView.centreUrl} target="_blank" rel="noopener">
            {captionView.centre}
            <ExternalLink size={16} strokeWidth={2} aria-hidden="true" />
          </a>
          {#if captionView.settlement}
            · nearest city: {captionView.settlement.name}, {captionView.settlement.country}
            ({captionView.settlement.distance} km away)
          {/if}
        </p>
      </div>
    {/if}
    <footer class="figure-footer">
      <span class="figure-actions">
        <a class="caption-link" href={figure.document}>
          <FileText size={16} strokeWidth={2} aria-hidden="true" />
          View data
        </a>
        {#if countryAnchor}
          · <a
            class="caption-link"
            href={countryUrl(window.location.href, countryAnchor)}
            onclick={(event) => onCopy(event, countryAnchor)}
            ><Copy size={16} strokeWidth={2} aria-hidden="true" /> Copy URL</a
          >
        {/if}
      </span>
      <span class="figure-attribution">© EuroGeographics for the administrative boundaries</span>
    </footer>
  </figcaption>
</figure>

<style>
  figure {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.8rem;
  }
  .map-frame {
    position: relative;
    width: 100%;
    overflow: hidden;
    background: var(--surface-2);
  }
  .map-link {
    display: block;
  }
  figure img {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    opacity: 0;
  }
  figure img.loaded {
    opacity: 1;
  }
  .image-status {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 1rem;
    color: var(--muted);
    text-align: center;
  }
  figcaption {
    padding: 0;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .figure-caption {
    padding: 0.75rem 1rem;
  }
  figcaption p {
    margin: 0 0 0.5rem;
  }
  .figure-footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.45rem 1rem;
    color: var(--muted);
    background: var(--surface-quiet);
    font-size: 0.75rem;
  }
  .figure-attribution {
    text-align: right;
  }
  figcaption a,
  .figure-footer a {
    color: inherit;
    text-decoration-color: var(--line);
    text-underline-offset: 3px;
  }
  .caption-link {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }
  figcaption a:hover {
    color: var(--accent);
  }
</style>
