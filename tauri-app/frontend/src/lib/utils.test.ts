import { describe, it, expect } from 'vitest';
import { cn, extractErrorMessage } from './utils';

describe('cn', () => {
  it('merges class names and dedupes conflicting tailwind classes', () => {
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });

  it('preserves non-conflicting classes while deduping conflicts', () => {
    expect(cn('px-2 py-1', 'px-4')).toBe('py-1 px-4');
  });

  it('filters out falsy values', () => {
    expect(cn('foo', false, null, undefined, '', 'bar')).toBe('foo bar');
  });

  it('returns empty string when given no arguments', () => {
    expect(cn()).toBe('');
  });
});

describe('extractErrorMessage', () => {
  it('returns the string as-is when given a string', () => {
    expect(extractErrorMessage('network error')).toBe('network error');
  });

  it('extracts message from an Error instance', () => {
    expect(extractErrorMessage(new Error('boom'))).toBe('boom');
  });

  it('stringifies plain objects', () => {
    expect(extractErrorMessage({ code: 500 })).toBe('[object Object]');
  });

  it('stringifies numbers', () => {
    expect(extractErrorMessage(42)).toBe('42');
  });
});
