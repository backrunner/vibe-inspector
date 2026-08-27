export const moduleRailMinWidth = 220;
export const moduleRailMaxWidth = 420;
export const inspectorPaneMinWidth = 560;
export const inspectorPaneMaxWidth = 1600;
export const workspaceResizeHandleWidth = 8;

export function clampModuleRailWidth(requestedWidth: number, paneGroupWidth: number): number {
  const sharedWidth = Math.max(0, paneGroupWidth - workspaceResizeHandleWidth);
  const dynamicMinimum = Math.max(moduleRailMinWidth, sharedWidth - inspectorPaneMaxWidth);
  const dynamicMaximum = Math.min(moduleRailMaxWidth, sharedWidth - inspectorPaneMinWidth);
  const maximum = Math.max(moduleRailMinWidth, dynamicMaximum);
  // When the inspector maximum makes the constraints unsatisfiable, fail open to
  // the static rail minimum instead of collapsing the range to a single width.
  const minimum =
    dynamicMinimum > moduleRailMaxWidth
      ? moduleRailMinWidth
      : Math.min(dynamicMinimum, maximum);

  return Math.round(Math.min(maximum, Math.max(minimum, requestedWidth)));
}
