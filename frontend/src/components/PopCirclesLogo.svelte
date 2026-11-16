<script>
  import globeSprite from '../../assets/earth-globe.svg?url';

  /** @type {{scrollPosition: number}} */
  let { scrollPosition } = $props();
  const globeFrameCount = 72;
  let globeFrame = $derived(Math.floor((scrollPosition / 240) * globeFrameCount) % globeFrameCount);
  let globeSymbol = $derived(`frame-${String(globeFrame + 1).padStart(2, '0')}`);
</script>

<a class="home-link" href="./" aria-label="PopCircles home">
  <span class="brand-text"
    >P<span class="o-slot"
      >o<svg class="globe-letter" viewBox="0 0 512 512" aria-hidden="true"
        ><use href={`${globeSprite}#${globeSymbol}`}></use></svg
      ></span
    >pCircles</span
  >
  <svg class="trailing-globe" viewBox="0 0 512 512" aria-hidden="true"
    ><use href={`${globeSprite}#${globeSymbol}`}></use></svg
  >
</a>

<style>
  .home-link {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    color: var(--accent);
    font:
      400 1rem/1 'Lilita One',
      sans-serif;
    letter-spacing: 0.16em;
    text-decoration: none;
    text-transform: uppercase;
  }
  .brand-text {
    white-space: nowrap;
  }
  .o-slot {
    position: relative;
    display: inline-block;
  }
  .globe-letter {
    display: block;
    width: 1em;
    height: 1em;
  }
  .o-slot .globe-letter {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
  }
  .trailing-globe {
    display: none;
    width: 24px;
    height: 24px;
  }
</style>
