<script lang="ts">
  import { Menu, Moon, Sun } from '@lucide/svelte';
  import type { GalleryData } from '#lib/types/gallery';
  import githubIcon from '#lib/assets/github-icon.svg?no-inline';
  import PopCirclesLogo from '#lib/components/layout/PopCirclesLogo.svelte';

  type Props = {
    scrollPosition: number;
    data: GalleryData | null;
    themeLight: boolean;
    drawerOpen: boolean;
    headerElement: HTMLElement | null;
    onHome: () => void;
    onTheme: () => void;
    onOpenDrawer: () => void;
  };

  let {
    scrollPosition,
    data,
    themeLight,
    drawerOpen,
    headerElement = $bindable(),
    onHome,
    onTheme,
    onOpenDrawer,
  }: Props = $props();
</script>

<header id="site-header" bind:this={headerElement}>
  {#if data?.world[0]}
    <div id="header-art" aria-hidden="true"><img src="./header-background.png" alt="" /></div>
  {/if}
  <div id="top-bar">
    <button
      class="drawer-toggle"
      class:hidden={drawerOpen}
      type="button"
      aria-controls="country-navigation-panel"
      aria-expanded={drawerOpen}
      aria-label="Open maps navigation"
      onclick={onOpenDrawer}><Menu size={20} strokeWidth={2} aria-hidden="true" /></button
    >
    <PopCirclesLogo {scrollPosition} {onHome} />
    {#if data}
      <div id="site-actions" role="group" aria-label="Gallery controls">
        <a
          class="icon-control"
          href="https://github.com/turboBasic/PopCircles"
          aria-label="PopCircles on GitHub"
        >
          <span class="github-icon" style={`--github-icon: url("${githubIcon}")`} aria-hidden="true"
          ></span>
        </a>
        <button
          class="icon-control"
          type="button"
          aria-label={themeLight ? 'Switch to dark theme' : 'Switch to light theme'}
          aria-pressed={themeLight}
          onclick={onTheme}
        >
          {#if themeLight}
            <Moon size={20} strokeWidth={2} aria-hidden="true" />
          {:else}
            <Sun size={20} strokeWidth={2} aria-hidden="true" />
          {/if}
        </button>
      </div>
    {/if}
  </div>
  {#if data}
    <div id="site-header__intro" class="workspace-intro">
      <div id="site-header__intro-content" class="workspace-intro-content">
        <h1>One circle, half the population.</h1>
        <p>Explore country and world population circles.</p>
      </div>
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
    padding: 0.75rem;
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
  #header-art {
    position: absolute;
    inset: 0 auto 0 50%;
    width: 100vw;
    transform: translateX(-50%);
    z-index: 1;
    overflow: hidden;
    pointer-events: none;
    opacity: 0.16;
  }
  #header-art img {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 104vw;
    height: auto;
    max-width: none;
    transform: translate(-50%, -50%);
    filter: grayscale(1) brightness(0.65);
  }
  #top-bar {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    pointer-events: auto;
  }
  #site-header__intro {
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    position: relative;
    z-index: 2;
    margin: 1.25rem 0;
    overflow: hidden;
    opacity: 1;
  }
  #site-header__intro-content {
    min-block-size: 0;
    overflow: hidden;
  }
  @supports (scroll-timeline: --results-scroll block) and (timeline-scope: --results-scroll) and
    (animation-range: 0px 160px) {
    header {
      animation: collapse-header auto linear both;
      animation-timeline: --results-scroll;
      animation-range: 0px 160px;
    }
    #header-art {
      animation: fade-header-art auto linear both;
      animation-timeline: --results-scroll;
      animation-range: 0px 160px;
    }
    #site-header__intro {
      animation: collapse-intro auto linear both;
      animation-timeline: --results-scroll;
      animation-range: 0px 160px;
    }
    @keyframes collapse-header {
      to {
        padding: 0.25rem 0;
      }
    }
    @keyframes fade-header-art {
      to {
        opacity: 0;
      }
    }
    @keyframes collapse-intro {
      to {
        grid-template-rows: minmax(0, 0fr);
        margin: 0;
        opacity: 0;
      }
    }
  }
  #site-header__intro h1 {
    max-width: 850px;
    margin: 0 0 0.5rem;
    font-size: clamp(2rem, 5vw, 4rem);
    line-height: 1.08;
    letter-spacing: -0.045em;
  }
  #site-header__intro p {
    margin: 0;
    color: var(--muted);
  }
  #site-actions {
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
  .github-icon {
    display: block;
    width: 1.25rem;
    height: 1.25rem;
  }
  .github-icon {
    background-color: currentColor;
    mask: var(--github-icon) center / contain no-repeat;
  }
  .drawer-toggle {
    display: grid;
    min-width: 2.75rem;
    min-height: 2.75rem;
    place-items: center;
    padding: 0;
    white-space: nowrap;
  }
  .drawer-toggle.hidden {
    visibility: hidden;
    pointer-events: none;
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
  button:hover,
  .icon-control:hover {
    border-color: var(--accent);
  }
  :global(a:focus-visible),
  :global(button:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
