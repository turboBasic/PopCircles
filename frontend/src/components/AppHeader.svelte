<script>
  /** @typedef {import('../gallery.js').GalleryData} GalleryData */
  import PopCirclesLogo from './PopCirclesLogo.svelte';

  /** @type {{scrollPosition: number, headerCollapseProgress: number, data: GalleryData | null, themeLight: boolean, drawerOpen: boolean, browserTrigger: HTMLButtonElement | null, onTheme: () => void, onToggleDrawer: () => void}} */
  let {
    scrollPosition,
    headerCollapseProgress,
    data,
    themeLight,
    drawerOpen,
    browserTrigger = $bindable(),
    onTheme,
    onToggleDrawer,
  } = $props();
</script>

<header
  class="app-header"
  class:collapsed={headerCollapseProgress === 1}
  style={`--header-padding: ${0.75 - 0.5 * headerCollapseProgress}rem; --art-opacity: ${0.16 * (1 - headerCollapseProgress)}; --intro-height: ${12 * (1 - headerCollapseProgress)}rem; --intro-margin: ${1.25 * (1 - headerCollapseProgress)}rem 0; --intro-opacity: ${1 - headerCollapseProgress}; --logo-size: ${1 - 0.12 * headerCollapseProgress}rem; --logo-spacing: ${0.16 - 0.03 * headerCollapseProgress}em`}
>
  {#if data?.world[0]}
    <div class="header-art" aria-hidden="true"><img src="./header-background.png" alt="" /></div>
  {/if}
  <div class="top-bar">
    <button
      bind:this={browserTrigger}
      class="drawer-toggle"
      type="button"
      aria-controls="country-browser-panel"
      aria-expanded={drawerOpen}
      aria-label={drawerOpen ? 'Close map navigation' : 'Open map navigation'}
      onclick={onToggleDrawer}>☰</button
    >
    <PopCirclesLogo {scrollPosition} />
    {#if data}
      <div class="map-actions" role="group" aria-label="Gallery controls">
        <a
          class="icon-control"
          href="https://github.com/turboBasic/PopCircles"
          aria-label="PopCircles on GitHub"
        >
          <svg viewBox="0 0 26 24" aria-hidden="true"
            ><path
              transform="translate(1 0)"
              fill="currentColor"
              d="M12 .9a11.1 11.1 0 0 0-3.51 21.63c.55.1.76-.24.76-.54v-2.1c-3.1.67-3.76-1.32-3.76-1.32-.5-1.28-1.23-1.62-1.23-1.62-1.01-.69.08-.68.08-.68 1.12.08 1.71 1.15 1.71 1.15 1 .1.75 2.05 3.44 1.57.1-.73.39-1.23.7-1.52-2.48-.28-5.09-1.24-5.09-5.53 0-1.22.44-2.22 1.15-3-.12-.28-.5-1.42.11-2.96 0 0 .94-.3 3.05 1.15a10.6 10.6 0 0 1 5.55 0c2.12-1.44 3.05-1.15 3.05-1.15.61 1.54.23 2.68.11 2.96.72.78 1.15 1.78 1.15 3 0 4.3-2.61 5.24-5.1 5.52.4.35.75 1.03.75 2.08v3.08c0 .3.2.65.77.54A11.1 11.1 0 0 0 12 .9Z"
            /></svg
          >
        </a>
        <button
          class="icon-control"
          type="button"
          aria-label={themeLight ? 'Switch to dark theme' : 'Switch to light theme'}
          aria-pressed={themeLight}
          onclick={onTheme}
        >
          {#if themeLight}
            <svg viewBox="0 0 24 24" aria-hidden="true"
              ><path
                fill="currentColor"
                d="M20.2 15.4A8.6 8.6 0 0 1 8.6 3.8a8.7 8.7 0 1 0 11.6 11.6Z"
              /></svg
            >
          {:else}
            <svg viewBox="0 0 24 24" aria-hidden="true"
              ><circle cx="12" cy="12" r="4" fill="currentColor" /><path
                stroke="currentColor"
                stroke-linecap="round"
                stroke-width="2"
                d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42"
              /></svg
            >
          {/if}
        </button>
      </div>
    {/if}
  </div>
  {#if data}
    <div class="workspace-intro" aria-hidden={headerCollapseProgress === 1}>
      <h1>One circle, half the population.</h1>
      <p>Explore country and world population circles.</p>
    </div>
  {/if}
</header>

<style>
  header {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
    z-index: 4;
    padding: var(--header-padding);
    background: transparent;
    pointer-events: none;
  }
  header::before {
    position: absolute;
    z-index: 0;
    inset: 0 0 auto;
    height: 200%;
    background: var(--chrome-bg);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    -webkit-mask-image: linear-gradient(to bottom, #000 0 50%, transparent 50% 100%);
    mask-image: linear-gradient(to bottom, #000 0 50%, transparent 50% 100%);
    pointer-events: none;
    content: '';
  }
  .header-art {
    position: absolute;
    inset: 0 auto 0 50%;
    width: 100vw;
    transform: translateX(-50%);
    z-index: 1;
    overflow: hidden;
    pointer-events: none;
    opacity: var(--art-opacity);
  }
  .header-art img {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 104vw;
    height: auto;
    max-width: none;
    transform: translate(-50%, -50%);
    filter: grayscale(1) brightness(0.65);
  }
  .top-bar {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    pointer-events: auto;
  }
  .workspace-intro {
    position: relative;
    z-index: 2;
    margin: 1.25rem 0;
    max-height: var(--intro-height);
    overflow: hidden;
    margin: var(--intro-margin);
    opacity: var(--intro-opacity);
  }
  .workspace-intro h1 {
    max-width: 850px;
    margin: 0 0 0.5rem;
    font-size: clamp(2rem, 5vw, 4rem);
    line-height: 1.08;
    letter-spacing: -0.045em;
  }
  .workspace-intro p {
    margin: 0;
    color: var(--muted);
  }
  .map-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .icon-control {
    display: grid;
    width: 2.5rem;
    height: 2.5rem;
    place-items: center;
    padding: 0.45rem;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    text-decoration: none;
  }
  .icon-control svg {
    width: 1.25rem;
    height: 1.25rem;
  }
  .drawer-toggle {
    white-space: nowrap;
  }
  @media (min-width: 1024px) {
    .drawer-toggle {
      display: none;
    }
  }
  button {
    padding: 0.55rem 0.85rem;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 0.5rem;
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    border-color: var(--accent);
  }
  :global(a:focus-visible),
  :global(button:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
