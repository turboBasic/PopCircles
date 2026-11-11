<script>
  import { onMount } from 'svelte';
  import { countryFromHash, defaultCountry, filterCountries } from './gallery.js';

  let data = $state(null);
  let error = $state('');
  let query = $state('');
  let selectedCountry = $state(null);
  let matches = $derived(data ? filterCountries(data.countries, query) : []);

  onMount(() => {
    let active = true;
    const restoreCountry = () => {
      if (data) {
        selectedCountry = countryFromHash(
          data.countries,
          window.location.hash,
          defaultCountry(data.countries)
        );
      }
    };

    window.addEventListener('hashchange', restoreCountry);
    window.addEventListener('popstate', restoreCountry);
    fetch('./gallery-data.json')
      .then((response) => {
        if (!response.ok) throw new Error(`Gallery data request failed (${response.status})`);
        return response.json();
      })
      .then((value) => {
        if (!active) return;
        data = value;
        restoreCountry();
      })
      .catch((cause) => {
        if (active) error = cause.message;
      });

    return () => {
      active = false;
      window.removeEventListener('hashchange', restoreCountry);
      window.removeEventListener('popstate', restoreCountry);
    };
  });
</script>

<svelte:head>
  <title>PopCircles · Map gallery</title>
</svelte:head>

<main>
  <header>
    <p class="eyebrow">PopCircles · Map gallery</p>
    <h1>One circle, half the population.</h1>
    <p class="intro">Explore country and world population circles.</p>
  </header>

  {#if error}
    <p role="alert">Could not load gallery data: {error}</p>
  {:else if !data}
    <p role="status">Loading gallery…</p>
  {:else}
    <div class="explorer">
      <aside aria-label="Country navigation">
        <label for="country-search">Find a country</label>
        <input id="country-search" type="search" bind:value={query} placeholder="Search countries" />
        <nav aria-label="Countries">
          {#each matches as country (country.anchor)}
            <a
              class:active={selectedCountry?.anchor === country.anchor}
              aria-current={selectedCountry?.anchor === country.anchor ? 'page' : undefined}
              href={`#${country.anchor}`}
            >
              {country.name}
            </a>
          {:else}
            <p class="empty">No matching countries.</p>
          {/each}
        </nav>
        <p class="status" role="status" aria-live="polite">{matches.length} countries</p>
      </aside>

      <div class="results">
        {#if data.world.length}
          <section class="world" aria-label="World maps">
            <h2>World</h2>
            <div class="figure-grid">
              {#each data.world as figure (figure.map)}
                <figure>
                  <a href={figure.map}><img src={figure.map} alt={figure.alt} loading="lazy" /></a>
                  <figcaption><a href={figure.document}>View result data</a></figcaption>
                </figure>
              {/each}
            </div>
          </section>
        {/if}

        {#if selectedCountry}
          <section class="country" id={selectedCountry.anchor} aria-label="Selected country result">
            <p class="eyebrow">50% population circle</p>
            <h2>{selectedCountry.name}</h2>
            {#if selectedCountry.missing}
              <p class="empty">No 50% result is available for this country.</p>
            {:else}
              <div class="figure-grid">
                {#each selectedCountry.figures as figure (figure.map)}
                  <figure>
                    <a href={figure.map}><img src={figure.map} alt={figure.alt} loading="lazy" /></a>
                    <figcaption><a href={figure.document}>View result data</a></figcaption>
                  </figure>
                {/each}
              </div>
            {/if}
          </section>
        {/if}
      </div>
    </div>
  {/if}
</main>

<style>
  :global(*) { box-sizing: border-box; }
  :global(body) {
    margin: 0;
    color: #eef3ff;
    background: radial-gradient(ellipse at 50% -20%, #203456 0, #0b1020 55%);
    font: 16px/1.55 system-ui, sans-serif;
  }
  main { width: min(1120px, calc(100% - 2rem)); margin: auto; }
  header { padding: 4rem 0 2rem; }
  .eyebrow { color: #80e0c0; font-size: .75rem; font-weight: 700; letter-spacing: .16em; text-transform: uppercase; }
  h1 { max-width: 760px; margin: .7rem 0 1rem; font-size: clamp(2rem, 5vw, 4rem); line-height: 1.08; letter-spacing: -.045em; }
  .intro, .status { color: #a4b0c5; }
  .explorer { display: grid; grid-template-columns: minmax(14rem, 18rem) minmax(0, 1fr); gap: 1.5rem; }
  aside, .country, .world { padding: 1.25rem; background: #121a2b; border: 1px solid #29364d; border-radius: .8rem; }
  label { display: block; margin-bottom: .5rem; font-weight: 600; }
  input { width: 100%; padding: .7rem .8rem; color: inherit; background: #0b1020; border: 1px solid #29364d; border-radius: .5rem; font: inherit; }
  input:focus-visible, a:focus-visible { outline: 2px solid #80e0c0; outline-offset: 2px; }
  nav { display: grid; gap: .4rem; margin-top: 1rem; }
  nav a { padding: .7rem .8rem; color: inherit; text-decoration: none; border: 1px solid #29364d; border-radius: .5rem; }
  nav a:hover, nav a.active { border-color: #80e0c0; }
  .empty { color: #a4b0c5; }
  .status { font-size: .85rem; }
  .results { display: grid; align-content: start; gap: 1.5rem; min-width: 0; }
  h2 { margin-top: 0; }
  .figure-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr)); gap: 1rem; }
  figure { min-width: 0; margin: 0; overflow: hidden; background: #19243a; border: 1px solid #29364d; border-radius: .6rem; }
  figure img { display: block; width: 100%; height: auto; }
  figcaption { padding: .75rem 1rem; color: #a4b0c5; font-size: .85rem; }
  figcaption a { color: inherit; }
  @media (max-width: 700px) { .explorer { grid-template-columns: 1fr; } }
</style>
