<script>
  let query = $state('');
  let selected = $state('Germany');

  const countries = [
    { name: 'France', population: '68.4 million', radius: '1,356 km', maps: 1 },
    { name: 'Germany', population: '84.1 million', radius: '1,012 km', maps: 2 },
    { name: 'Japan', population: '123.8 million', radius: '1,428 km', maps: 1 },
    { name: 'United States', population: '334.9 million', radius: '2,109 km', maps: 1 }
  ];

  let matches = $derived(countries.filter((country) =>
    country.name.toLowerCase().includes(query.trim().toLowerCase())
  ));
  let selectedCountry = $derived(countries.find((country) => country.name === selected));

  function select(name) {
    selected = name;
    history.replaceState(null, '', `#${encodeURIComponent(name.toLowerCase().replaceAll(' ', '-'))}`);
  }
</script>

<svelte:head>
  <title>PopCircles · Svelte gallery spike</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</svelte:head>

<main>
  <header>
    <p class="eyebrow">PopCircles · Map gallery</p>
    <h1>One circle, half the population.</h1>
    <p class="intro">Explore the smallest circle holding half of each country's population.</p>
  </header>

  <div class="explorer">
    <aside aria-label="Country navigation">
      <label for="country-search">Find a country</label>
      <input id="country-search" type="search" bind:value={query} placeholder="Search countries" />
      <nav aria-label="Countries">
        {#each matches as country (country.name)}
          <button
            class:active={selected === country.name}
            aria-current={selected === country.name ? 'page' : undefined}
            onclick={() => select(country.name)}
          >
            {country.name}
          </button>
        {:else}
          <p class="empty">No matching countries.</p>
        {/each}
      </nav>
      <p class="status" role="status" aria-live="polite">{matches.length} countries</p>
    </aside>

    <section aria-label="Selected country result">
      {#if selectedCountry}
        <p class="eyebrow">50% population circle</p>
        <h2>{selectedCountry.name}</h2>
        <div class="map" role="img" aria-label={`Illustration of the population circle for ${selectedCountry.name}`}>
          <span class="circle"></span>
          <span class="map-label">{selectedCountry.name}</span>
        </div>
        <dl>
          <div><dt>Population</dt><dd>{selectedCountry.population}</dd></div>
          <div><dt>Circle radius</dt><dd>{selectedCountry.radius}</dd></div>
          <div><dt>Available maps</dt><dd>{selectedCountry.maps}</dd></div>
        </dl>
      {/if}
    </section>
  </div>
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
  .intro, dt, .status { color: #a4b0c5; }
  .explorer { display: grid; grid-template-columns: minmax(14rem, 18rem) minmax(0, 1fr); gap: 1.5rem; }
  aside, section { padding: 1.25rem; background: #121a2b; border: 1px solid #29364d; border-radius: .8rem; }
  label { display: block; margin-bottom: .5rem; font-weight: 600; }
  input, nav button { width: 100%; padding: .7rem .8rem; color: inherit; background: #0b1020; border: 1px solid #29364d; border-radius: .5rem; font: inherit; }
  input:focus-visible, button:focus-visible { outline: 2px solid #80e0c0; outline-offset: 2px; }
  nav { display: grid; gap: .4rem; margin-top: 1rem; }
  nav button { text-align: left; cursor: pointer; }
  nav button:hover, nav button.active { border-color: #80e0c0; }
  .empty { color: #a4b0c5; }
  .status { font-size: .85rem; }
  h2 { margin-top: 0; }
  .map { position: relative; display: grid; min-height: 300px; place-items: center; overflow: hidden; background: radial-gradient(ellipse, #192f3f, #101a2a 70%); border-radius: .6rem; }
  .circle { width: min(55vw, 260px); aspect-ratio: 1; border: 2px solid #80e0c0; border-radius: 50%; box-shadow: 0 0 70px #80e0c033; }
  .map-label { position: absolute; color: #a4b0c5; font-size: .85rem; }
  dl { display: flex; gap: 2rem; flex-wrap: wrap; }
  dt { font-size: .8rem; }
  dd { margin: .2rem 0 0; font-weight: 600; }
  @media (max-width: 700px) { .explorer { grid-template-columns: 1fr; } .map { min-height: 240px; } }
</style>
