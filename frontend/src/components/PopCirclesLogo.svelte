<script>
  import { onMount } from 'svelte';

  /** @type {{scrollPosition: number}} */
  let { scrollPosition } = $props();
  const globeFrames = Object.entries(
    import.meta.glob('../../assets/earth-globe-*.svg', {
      eager: true,
      query: '?url',
      import: 'default',
    }),
  )
    .sort(([left], [right]) => left.localeCompare(right, undefined, { numeric: true }))
    .map(([, url]) => url);
  let globeFramesReady = $state(false);
  let globeFrame = $derived(
    globeFramesReady
      ? globeFrames[Math.floor((scrollPosition / 240) * globeFrames.length) % globeFrames.length]
      : globeFrames[0],
  );

  onMount(() => {
    let active = true;
    const images = globeFrames.map((src) => {
      const image = new Image();
      image.src = src;
      return image;
    });
    Promise.allSettled(images.map((image) => image.decode())).then(() => {
      if (active) globeFramesReady = true;
    });
    return () => (active = false);
  });
</script>

<a class="home-link" href="./" aria-label="PopCircles home"
  ><span>P<img class="globe-letter" src={globeFrame} alt="" />pCircles</span><img
    class="trailing-globe"
    src={globeFrame}
    alt=""
  /></a
>

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
  .home-link .globe-letter {
    display: inline;
    width: 0.8em;
    height: 0.8em;
    margin-left: -0.1em;
    margin-right: 0.1em;
    vertical-align: baseline -0.05em;
  }
  .home-link img {
    width: 24px;
    height: 24px;
  }
  .home-link .trailing-globe {
    display: none;
  }
</style>
