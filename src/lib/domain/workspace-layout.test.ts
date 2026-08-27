import { describe, expect, it } from 'vitest';
import {
  clampModuleRailWidth,
  inspectorPaneMinWidth,
  moduleRailMaxWidth,
  moduleRailMinWidth,
  workspaceResizeHandleWidth,
} from './workspace-layout';

describe('clampModuleRailWidth', () => {
  it('keeps the module rail within its own bounds', () => {
    expect(clampModuleRailWidth(100, 1216)).toBe(moduleRailMinWidth);
    expect(clampModuleRailWidth(900, 1216)).toBe(moduleRailMaxWidth);
  });

  it('reserves the inspector minimum width in a compact workspace', () => {
    const paneGroupWidth = 880;
    const result = clampModuleRailWidth(420, paneGroupWidth);
    const inspectorWidth = paneGroupWidth - result - workspaceResizeHandleWidth;

    expect(inspectorWidth).toBeGreaterThanOrEqual(inspectorPaneMinWidth);
  });

  it('fails open to the static rail bounds when the inspector maximum is unsatisfiable', () => {
    for (const paneGroupWidth of [2036, 2560]) {
      expect(clampModuleRailWidth(300, paneGroupWidth)).toBe(300);
      expect(clampModuleRailWidth(100, paneGroupWidth)).toBe(moduleRailMinWidth);
      expect(clampModuleRailWidth(900, paneGroupWidth)).toBe(moduleRailMaxWidth);
    }
  });
});
