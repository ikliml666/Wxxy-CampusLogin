import { describe, it, expect } from 'vitest';
import { hexToHsl } from './color';

describe('hexToHsl', () => {
  it('converts pure red (#ff0000) to h=0 s=100 l=50', () => {
    expect(hexToHsl('#ff0000')).toEqual({ h: 0, s: 100, l: 50 });
  });

  it('converts pure green (#00ff00) to h=120 s=100 l=50', () => {
    expect(hexToHsl('#00ff00')).toEqual({ h: 120, s: 100, l: 50 });
  });

  it('converts pure blue (#0000ff) to h=240 s=100 l=50', () => {
    expect(hexToHsl('#0000ff')).toEqual({ h: 240, s: 100, l: 50 });
  });

  it('converts white (#ffffff) to h=0 s=0 l=100', () => {
    expect(hexToHsl('#ffffff')).toEqual({ h: 0, s: 0, l: 100 });
  });

  it('converts black (#000000) to h=0 s=0 l=0', () => {
    expect(hexToHsl('#000000')).toEqual({ h: 0, s: 0, l: 0 });
  });

  it('returns default fallback for invalid hex string', () => {
    expect(hexToHsl('not-a-color')).toEqual({ h: 230, s: 70, l: 55 });
  });

  it('returns default fallback for empty string', () => {
    expect(hexToHsl('')).toEqual({ h: 230, s: 70, l: 55 });
  });

  it('returns default fallback for short hex (#fff)', () => {
    expect(hexToHsl('#fff')).toEqual({ h: 230, s: 70, l: 55 });
  });

  it('accepts hex without leading #', () => {
    expect(hexToHsl('ff0000')).toEqual({ h: 0, s: 100, l: 50 });
  });
});
