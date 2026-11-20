<script lang="ts">
  import { Copy, ExternalLink, FileText } from '@lucide/svelte';
  import { territorySharePercent } from '#lib/utils/gallery';
  import type { FigureCaption } from '#lib/types/gallery';

  type Props = {
    caption: FigureCaption | undefined;
    document: string;
    countryAnchor: string | null;
    onCopy: (event: MouseEvent, anchor: string) => Promise<void>;
  };

  let { caption, document: documentUrl, countryAnchor, onCopy }: Props = $props();

  function formatFigureCaption(value: FigureCaption) {
    const centreLat = value.centre_lat.toFixed(4);
    const centreLon = value.centre_lon.toFixed(4);
    return {
      radius: `${value.radius_km.toLocaleString()} km`,
      population: value.population.toLocaleString('en-US', { maximumFractionDigits: 0 }),
      share: `${(value.share * 100).toFixed(1)}%`,
      territoryShare: territorySharePercent(value.territory_share),
      territoryName: value.territory_name,
      centre: `${centreLat}, ${centreLon}`,
      centreUrl: `https://www.openstreetmap.org/?mlat=${centreLat}&mlon=${centreLon}#map=12/${centreLat}/${centreLon}`,
      settlement: value.settlement
        ? {
            name: value.settlement.name,
            country: value.settlement.country,
            distance: Math.round(value.settlement.distance_km).toLocaleString(),
          }
        : null,
    };
  }

  let captionView = $derived(caption ? formatFigureCaption(caption) : null);
</script>

{#if captionView}
  <div class="map-caption">
    <p>
      radius: <strong>{captionView.radius}</strong> · population:
      <strong>{captionView.population}</strong>
      / <strong>{captionView.share}</strong> ·
      {#if captionView.territoryShare !== null}
        Circle covers <strong>{captionView.territoryShare}%</strong> of {captionView.territoryName}
        ·
      {/if}
      centre:
      <a class="map-caption-link" href={captionView.centreUrl} target="_blank" rel="noopener">
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
<footer class="map-footer">
  <span>
    <a class="map-caption-link" href={documentUrl}>
      <FileText size={16} strokeWidth={2} aria-hidden="true" />
      View data
    </a>
    {#if countryAnchor}
      · <a
        class="map-caption-link"
        href={`#${countryAnchor}`}
        onclick={(event) => onCopy(event, countryAnchor)}
        ><Copy size={16} strokeWidth={2} aria-hidden="true" /> Copy URL</a
      >
    {/if}
  </span>
  <span class="map-attribution">© EuroGeographics for the administrative boundaries</span>
</footer>

<style>
  .map-caption {
    padding: 0.75rem 1rem;
  }
  p {
    margin: 0 0 0.5rem;
  }
  .map-footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.45rem 1rem;
    color: var(--muted);
    background: var(--surface-quiet);
    font-size: 0.75rem;
  }
  .map-attribution {
    text-align: right;
  }
  .map-caption-link {
    color: inherit;
    text-decoration-color: var(--line);
    text-underline-offset: 3px;
    align-items: center;
    display: inline-flex;
    gap: 0.25rem;
  }
  .map-caption-link:hover {
    color: var(--accent);
  }
</style>
