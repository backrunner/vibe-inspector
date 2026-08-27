// WheelEvent.deltaMode constants (DOM_DELTA_PIXEL/LINE/PAGE), kept as literals so
// this module stays usable outside the DOM.
const domDeltaLine = 1;
const domDeltaPage = 2;

export function normalizedWheelDelta(
  deltaY: number,
  deltaMode: number,
  pageHeight: number,
): number {
  if (deltaMode === domDeltaLine) return deltaY * 16;
  if (deltaMode === domDeltaPage) return deltaY * pageHeight;
  return deltaY;
}
