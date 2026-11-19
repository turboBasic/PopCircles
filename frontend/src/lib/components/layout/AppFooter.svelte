<script lang="ts">
  type Props = {
    footerElement: HTMLElement | null;
  };

  let { footerElement = $bindable() }: Props = $props();
</script>

<footer id="site-footer" bind:this={footerElement} class="site-footer">
  <a href="https://github.com/turboBasic">A project by Andrii Melnyk</a>
</footer>

<style>
  .site-footer {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 4;
    padding: 0.5rem 0;
    padding-right: calc(var(--country-inline-padding) + 1px);
    text-align: right;
    color: var(--muted);
    background: transparent;
    font-size: 0.72rem;
  }
  .site-footer::before {
    position: absolute;
    z-index: 0;
    inset: auto 0 0;
    height: 200%;
    background: var(--chrome-bg);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    -webkit-mask-image: linear-gradient(to bottom, transparent 0 50%, #000 50% 100%);
    mask-image: linear-gradient(to bottom, transparent 0 50%, #000 50% 100%);
    pointer-events: none;
    content: '';
  }
  .site-footer a {
    position: relative;
    z-index: 1;
    color: var(--text);
    text-decoration: none;
  }
  .site-footer a:hover,
  .site-footer a:focus-visible {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 0.2em;
  }
</style>
