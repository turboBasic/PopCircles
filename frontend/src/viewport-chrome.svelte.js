import { onMount } from 'svelte';

/**
 * @param {() => HTMLDivElement | null} getResultsPanel
 * @param {() => HTMLElement | null} getHeader
 * @param {() => HTMLElement | null} getFooter
 */
export function useViewportChrome(getResultsPanel, getHeader, getFooter) {
  let scrollPosition = $state(0);
  let headerCollapseProgress = $state(0);
  let headerSpace = $state(0);
  let navigationHeaderSpace = $state(0);
  let footerSpace = $state(0);

  function measureChrome() {
    const header = getHeader();
    const footer = getFooter();
    if (header) {
      navigationHeaderSpace = header.getBoundingClientRect().height;
      if (scrollPosition === 0) headerSpace = navigationHeaderSpace;
    }
    if (footer) footerSpace = footer.getBoundingClientRect().height;
  }

  function handleScroll() {
    scrollPosition = getResultsPanel()?.scrollTop ?? 0;
    headerCollapseProgress = Math.min(scrollPosition / 160, 1);
    if (scrollPosition === 0) requestAnimationFrame(measureChrome);
  }

  onMount(() => {
    const observer = new ResizeObserver(measureChrome);
    const header = getHeader();
    const footer = getFooter();
    if (header) observer.observe(header);
    if (footer) observer.observe(footer);
    handleScroll();
    measureChrome();
    return () => observer.disconnect();
  });

  return {
    get scrollPosition() {
      return scrollPosition;
    },
    get headerCollapseProgress() {
      return headerCollapseProgress;
    },
    get headerSpace() {
      return headerSpace;
    },
    get navigationHeaderSpace() {
      return navigationHeaderSpace;
    },
    get footerSpace() {
      return footerSpace;
    },
    handleScroll,
  };
}
