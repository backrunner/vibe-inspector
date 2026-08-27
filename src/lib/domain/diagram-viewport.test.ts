import { describe, expect, it } from 'vitest';
import { normalizedWheelDelta } from './diagram-viewport';

describe('normalizedWheelDelta', () => {
  it('passes pixel deltas through unchanged', () => {
    expect(normalizedWheelDelta(120, 0, 800)).toBe(120);
  });

  it('converts line deltas to pixels', () => {
    expect(normalizedWheelDelta(3, 1, 800)).toBe(48);
  });

  it('converts page deltas using the viewport height', () => {
    expect(normalizedWheelDelta(2, 2, 800)).toBe(1600);
  });
});
