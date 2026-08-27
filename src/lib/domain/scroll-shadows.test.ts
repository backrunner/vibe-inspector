import { describe, expect, it } from 'vitest';
import { scrollShadowState } from './scroll-shadows';

describe('scrollShadowState', () => {
  it('hides every shadow when content fits', () => {
    expect(
      scrollShadowState({
        scrollTop: 0,
        scrollLeft: 0,
        scrollHeight: 200,
        scrollWidth: 300,
        clientHeight: 200,
        clientWidth: 300,
      }),
    ).toEqual({ top: false, right: false, bottom: false, left: false });
  });

  it('reveals only the directions with undisclosed content', () => {
    expect(
      scrollShadowState({
        scrollTop: 40,
        scrollLeft: 0,
        scrollHeight: 500,
        scrollWidth: 700,
        clientHeight: 200,
        clientWidth: 300,
      }),
    ).toEqual({ top: true, right: true, bottom: true, left: false });
  });

  it('removes trailing shadows at the scroll limits', () => {
    expect(
      scrollShadowState({
        scrollTop: 300,
        scrollLeft: 400,
        scrollHeight: 500,
        scrollWidth: 700,
        clientHeight: 200,
        clientWidth: 300,
      }),
    ).toEqual({ top: true, right: false, bottom: false, left: true });
  });
});
