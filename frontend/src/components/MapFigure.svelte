<script>
  import { countryUrl, territoryCaption } from '../gallery.js';

  let { figure, countryAnchor = null, loading = 'lazy', onCopy } = $props();
</script>

<figure>
  <a href={figure.map}>
    <img src={figure.map} alt={figure.alt} {loading} />
  </a>
  <figcaption>
    {#if figure.caption}
      {@const territory = territoryCaption(figure.caption.territory_share, figure.caption.territory_name)}
      <p>
        {figure.caption.radius_km.toLocaleString()} km radius ·
        {(figure.caption.share * 100).toFixed(1)}% of the population ·
        {#if territory}
          {territory} ·
        {/if}
        {figure.caption.population.toLocaleString('en-US', { maximumFractionDigits: 0 })} people ·
        centre
        <a
          href={`https://www.openstreetmap.org/?mlat=${figure.caption.centre_lat.toFixed(4)}&mlon=${figure.caption.centre_lon.toFixed(4)}#map=12/${figure.caption.centre_lat.toFixed(4)}/${figure.caption.centre_lon.toFixed(4)}`}
          target="_blank"
          rel="noopener"
        >
          {figure.caption.centre_lat.toFixed(4)}, {figure.caption.centre_lon.toFixed(4)}
        </a>
        {#if figure.caption.settlement}
          · nearest city: {figure.caption.settlement.name},
          {figure.caption.settlement.country}
          ({Math.round(figure.caption.settlement.distance_km).toLocaleString()} km away)
        {/if}
      </p>
    {/if}
    <a href={figure.document}>View result data</a>
    {#if countryAnchor}
      · <a href={countryUrl(window.location.href, countryAnchor)} onclick={(event) => onCopy(event, countryAnchor)}>Copy country URL</a>
    {/if}
  </figcaption>
</figure>

<style>
  figure { min-width: 0; margin: 0; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); border-radius: .8rem; }
  figure img { display: block; width: 100%; height: auto; }
  figcaption { padding: .75rem 1rem; color: var(--muted); font-size: .85rem; }
  figcaption p { margin: 0 0 .5rem; }
  figcaption a { color: inherit; text-decoration-color: #65758e; text-underline-offset: 3px; }
</style>
