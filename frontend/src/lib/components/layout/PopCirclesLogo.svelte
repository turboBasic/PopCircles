<script lang="ts">
  import globeSprite from '#lib/assets/earth-globe.svg?url';

  let { scrollPosition, onHome }: { scrollPosition: number; onHome: () => void } = $props();
  const globeFrameCount = 72;
  let globeFrame = $derived(Math.floor((scrollPosition / 240) * globeFrameCount) % globeFrameCount);
  let globeSymbol = $derived(`frame-${String(globeFrame + 1).padStart(2, '0')}`);

  function handleHomeClick(event: MouseEvent) {
    if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
      return;
    event.preventDefault();
    onHome();
  }
</script>

<div id="logo">
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
</div>

<style>
  #logo {
    display: flex;
    align-items: center;
  }
  .home-link {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    color: var(--accent);
    font:
      400 1rem/1 'Lilita One',
      sans-serif;
    text-decoration: none;
    text-transform: uppercase;
    font-size: 1rem;
    letter-spacing: 0.16em;
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
  @supports (scroll-timeline: --results-scroll block) and (timeline-scope: --results-scroll) and
    (animation-range: 0px 160px) {
    .home-link {
      animation: collapse-logo auto linear both;
      animation-timeline: --results-scroll;
      animation-range: 0px 160px;
    }
    @keyframes collapse-logo {
      to {
        font-size: 0.88rem;
        letter-spacing: 0.13em;
      }
    }
  }
</style>
