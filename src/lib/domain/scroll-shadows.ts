export type ScrollMetrics = {
  scrollTop: number;
  scrollLeft: number;
  scrollHeight: number;
  scrollWidth: number;
  clientHeight: number;
  clientWidth: number;
};

export type ScrollShadowState = {
  top: boolean;
  right: boolean;
  bottom: boolean;
  left: boolean;
};

const scrollTolerance = 1;

export function scrollShadowState(metrics: ScrollMetrics): ScrollShadowState {
  const maxScrollTop = Math.max(0, metrics.scrollHeight - metrics.clientHeight);
  const maxScrollLeft = Math.max(0, metrics.scrollWidth - metrics.clientWidth);

  return {
    top: maxScrollTop > scrollTolerance && metrics.scrollTop > scrollTolerance,
    right: maxScrollLeft > scrollTolerance && metrics.scrollLeft < maxScrollLeft - scrollTolerance,
    bottom: maxScrollTop > scrollTolerance && metrics.scrollTop < maxScrollTop - scrollTolerance,
    left: maxScrollLeft > scrollTolerance && metrics.scrollLeft > scrollTolerance,
  };
}
