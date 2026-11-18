<script>
  import globeSprite from '#lib/assets/earth-globe.svg?url';

  /** @type {{scrollPosition: number, onHome: () => void}} */
  let { scrollPosition, onHome } = $props();
  const globeFrameCount = 72;
  let globeFrame = $derived(Math.floor((scrollPosition / 240) * globeFrameCount) % globeFrameCount);
  let globeSymbol = $derived(`frame-${String(globeFrame + 1).padStart(2, '0')}`);

  /** @param {MouseEvent} event */
  function handleHomeClick(event) {
    if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
      return;
    event.preventDefault();
    onHome();
  }
</script>

<a class="home-link" href="./" aria-label="PopCircles home" onclick={handleHomeClick}>
  <span class="brand-text" aria-hidden="true">
    <span class="letter-slot">P</span>
    <span class="letter-slot globe-slot"
      ><span class="globe-fallback">o</span><svg
        class="globe-letter"
        viewBox="0 0 512 512"
        style:scale="1.07"
        aria-hidden="true"><use href={`${globeSprite}#${globeSymbol}`}></use></svg
      ></span
    >
    <span class="letter-slot">p</span>
    <span class="letter-slot">C</span>
    <span class="letter-slot">i</span>
    <span class="letter-slot">r</span>
    <span class="letter-slot">c</span>
    <span class="letter-slot">l</span>
    <span class="letter-slot">e</span>
    <span class="letter-slot">s</span>
  </span>
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
    font-size: var(--logo-size);
    letter-spacing: var(--logo-spacing);
  }
  .brand-text {
    display: flex;
    gap: 0.12em;
    white-space: nowrap;
  }
  .letter-slot {
    flex: 0 0 0.72em;
    width: 0.72em;
    text-align: center;
  }
  .globe-slot {
    position: relative;
    margin-left: -0.08em;
    margin-right: 0.1em;
  }
  .globe-fallback {
    visibility: hidden;
  }
  .globe-letter {
    display: block;
    width: 100%;
    height: 0.72em;
  }
  .globe-slot .globe-letter {
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
