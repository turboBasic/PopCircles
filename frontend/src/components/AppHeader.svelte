<script>
  let { globeFrame, data, themeLight, browserOpen, browserTrigger = $bindable(), onTheme, onToggleBrowser } = $props();
</script>

<header>
  {#if data?.world[0]}
    <div class="header-art" aria-hidden="true"><img src="./header-background.png" alt="" /></div>
  {/if}
  <div class="top-bar">
    <a class="home-link" href="./"><span>P<img class="globe-letter" src={globeFrame} alt="" />pCircles</span><img class="trailing-globe" src={globeFrame} alt="" /></a>
    {#if data}
      <div class="map-actions" aria-label="Gallery controls">
        <a class="icon-control" href="https://github.com/turboBasic/PopCircles" aria-label="PopCircles on GitHub">
          <svg viewBox="0 0 26 24" aria-hidden="true"><path transform="translate(1 0)" fill="currentColor" d="M12 .9a11.1 11.1 0 0 0-3.51 21.63c.55.1.76-.24.76-.54v-2.1c-3.1.67-3.76-1.32-3.76-1.32-.5-1.28-1.23-1.62-1.23-1.62-1.01-.69.08-.68.08-.68 1.12.08 1.71 1.15 1.71 1.15 1 .1.75 2.05 3.44 1.57.1-.73.39-1.23.7-1.52-2.48-.28-5.09-1.24-5.09-5.53 0-1.22.44-2.22 1.15-3-.12-.28-.5-1.42.11-2.96 0 0 .94-.3 3.05 1.15a10.6 10.6 0 0 1 5.55 0c2.12-1.44 3.05-1.15 3.05-1.15.61 1.54.23 2.68.11 2.96.72.78 1.15 1.78 1.15 3 0 4.3-2.61 5.24-5.1 5.52.4.35.75 1.03.75 2.08v3.08c0 .3.2.65.77.54A11.1 11.1 0 0 0 12 .9Z"/></svg>
        </a>
        <button class="icon-control" type="button" aria-label={themeLight ? 'Switch to dark theme' : 'Switch to light theme'} aria-pressed={themeLight} onclick={onTheme}>
          {#if themeLight}
            <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M20.2 15.4A8.6 8.6 0 0 1 8.6 3.8a8.7 8.7 0 1 0 11.6 11.6Z"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4" fill="currentColor"/><path stroke="currentColor" stroke-linecap="round" stroke-width="2" d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"/></svg>
          {/if}
        </button>
        <button bind:this={browserTrigger} class="mobile-browser-toggle" type="button" aria-controls="country-browser-panel" aria-expanded={browserOpen} aria-label={browserOpen ? 'Close map navigation' : 'Open map navigation'} onclick={onToggleBrowser}>Maps</button>
      </div>
    {/if}
  </div>
  <h1>One circle, half the population.</h1>
  <p class="intro">Explore country and world population circles.</p>
</header>

<style>
  header { position: sticky; top: 0; z-index: 4; isolation: isolate; padding: calc(4rem - 3.4rem * var(--header-shrink)) 0 calc(1rem - .4rem * var(--header-shrink)); background: var(--bg); border-bottom: 1px solid color-mix(in srgb, var(--line) calc(100% * var(--header-shrink)), transparent); }
  .header-art { position: absolute; inset: 0 auto 0 50%; width: 100vw; transform: translateX(-50%); z-index: -1; overflow: hidden; opacity: calc(.16 * (1 - var(--header-shrink))); pointer-events: none; }
  .header-art img { position: absolute; top: 50%; left: 50%; width: 104vw; height: auto; max-width: none; transform: translate(-50%, -50%); filter: grayscale(1) brightness(.65); }
  .top-bar { display: flex; align-items: center; justify-content: space-between; gap: .75rem; }
  .home-link { display: flex; align-items: center; gap: .65rem; color: var(--accent); font: 400 1rem/1 "Lilita One", sans-serif; letter-spacing: .16em; text-decoration: none; text-transform: uppercase; }
  .home-link .globe-letter { display: inline; width: .8em; height: .8em; margin-left: -.1em; margin-right: .1em; vertical-align: baseline -.05em; }
  .home-link img { width: 24px; height: 24px; }
  .home-link .trailing-globe { display: none; }
  h1 { max-width: 850px; max-height: calc(4.5rem * (1 - var(--header-shrink))); margin: calc(.7rem * (1 - var(--header-shrink))) 0 calc(1rem * (1 - var(--header-shrink))); overflow: hidden; opacity: calc(1 - var(--header-shrink)); font-size: calc(clamp(2rem, 5vw, 4rem) * (1 - var(--header-shrink))); line-height: 1.08; letter-spacing: -.045em; }
  .intro { max-height: calc(3rem * (1 - var(--header-shrink))); margin-block: calc(.5rem * (1 - var(--header-shrink))); overflow: hidden; opacity: calc(1 - var(--header-shrink)); color: var(--muted); }
  .map-actions { display: flex; align-items: center; gap: .5rem; }
  .icon-control { display: grid; width: 2.5rem; height: 2.5rem; place-items: center; padding: .45rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .5rem; text-decoration: none; }
  .icon-control svg { width: 1.25rem; height: 1.25rem; }
  .mobile-browser-toggle { display: none; }
  button { padding: .55rem .85rem; color: var(--text); background: var(--surface-2); border: 1px solid var(--line); border-radius: .5rem; font: inherit; cursor: pointer; }
  button:hover { border-color: var(--accent); }
  :global(a:focus-visible), :global(button:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 800px) { .mobile-browser-toggle { display: block; } }
</style>
