import { describe, it, expect } from 'vitest';
import {
  getLatencyLevel,
  getLatencyColor,
  mergeNetworkQuality,
  extractGatewayLatency,
  extractExternalLatency,
} from './latency';
import type { NetworkQuality } from '@/monitor';

function makeNq(overrides: Partial<NetworkQuality> = {}): NetworkQuality {
  return {
    gatewayLatency: -1,
    externalLatency: -1,
    averageExternalLatency: -1,
    gateway: '',
    quality: 'unknown',
    timestamp: 0,
    ...overrides,
  };
}

describe('getLatencyLevel', () => {
  it('returns "excellent" for latencies <= 20', () => {
    expect(getLatencyLevel(0)).toBe('excellent');
    expect(getLatencyLevel(20)).toBe('excellent');
  });

  it('returns "great" for 21..50', () => {
    expect(getLatencyLevel(21)).toBe('great');
    expect(getLatencyLevel(50)).toBe('great');
  });

  it('returns "good" for 51..100', () => {
    expect(getLatencyLevel(51)).toBe('good');
    expect(getLatencyLevel(100)).toBe('good');
  });

  it('returns "fair" for 101..200', () => {
    expect(getLatencyLevel(101)).toBe('fair');
    expect(getLatencyLevel(200)).toBe('fair');
  });

  it('returns "poor" for 201..400', () => {
    expect(getLatencyLevel(201)).toBe('poor');
    expect(getLatencyLevel(400)).toBe('poor');
  });

  it('returns "bad" for > 400 or negative', () => {
    expect(getLatencyLevel(401)).toBe('bad');
    expect(getLatencyLevel(-1)).toBe('bad');
  });
});

describe('getLatencyColor', () => {
  it('returns rose colors for negative latency', () => {
    expect(getLatencyColor(-1)).toEqual({
      text: 'text-rose-500',
      bg: 'bg-rose-500/8',
      borderBg: 'bg-rose-500/10',
    });
  });

  it('maps excellent latency to emerald colors', () => {
    expect(getLatencyColor(10)).toEqual({
      text: 'text-emerald-500',
      bg: 'bg-emerald-500/10',
      borderBg: 'bg-emerald-500/20',
    });
  });

  it('maps good latency to blue colors', () => {
    expect(getLatencyColor(80)).toEqual({
      text: 'text-blue-500',
      bg: 'bg-blue-500/10',
      borderBg: 'bg-blue-500/20',
    });
  });
});

describe('mergeNetworkQuality', () => {
  it('returns incoming when old is null and incoming is disabled', () => {
    const incoming = makeNq({ quality: 'disabled' });
    expect(mergeNetworkQuality(null, incoming)).toBe(incoming);
  });

  it('returns old when old exists and incoming is disabled', () => {
    const old = makeNq({ quality: 'excellent', gatewayLatency: 5 });
    const incoming = makeNq({ quality: 'disabled' });
    expect(mergeNetworkQuality(old, incoming)).toBe(old);
  });

  it('returns incoming when old is null and incoming is normal', () => {
    const incoming = makeNq({ quality: 'excellent', gatewayLatency: 5 });
    expect(mergeNetworkQuality(null, incoming)).toBe(incoming);
  });

  it('returns incoming when old quality is unknown', () => {
    const old = makeNq({ quality: 'unknown' });
    const incoming = makeNq({ quality: 'good', gatewayLatency: 60 });
    expect(mergeNetworkQuality(old, incoming)).toBe(incoming);
  });

  it('merges details from old and incoming for normal qualities', () => {
    const old = makeNq({
      quality: 'excellent',
      details: { gateway: { target: 'gw', latency: 5, type: 'tcp' } },
      metrics: { totalElapsed: 100, tests: {} },
    });
    const incoming = makeNq({
      quality: 'good',
      details: { baidu: { target: 'baidu', latency: 30, type: 'tcp' } },
    });
    const result = mergeNetworkQuality(old, incoming);
    expect(result.details).toEqual({
      gateway: { target: 'gw', latency: 5, type: 'tcp' },
      baidu: { target: 'baidu', latency: 30, type: 'tcp' },
    });
    expect(result.metrics).toEqual({ totalElapsed: 100, tests: {} });
  });
});

describe('extractGatewayLatency', () => {
  it('returns -1 when nq is null', () => {
    expect(extractGatewayLatency(null)).toBe(-1);
  });

  it('returns gatewayLatency when it is >= 0', () => {
    const nq = makeNq({ gatewayLatency: 5 });
    expect(extractGatewayLatency(nq)).toBe(5);
  });

  it('falls back to details["gateway"].latency when gatewayLatency < 0', () => {
    const nq = makeNq({
      gatewayLatency: -1,
      details: { gateway: { target: 'gw', latency: 12, type: 'tcp' } },
    });
    expect(extractGatewayLatency(nq)).toBe(12);
  });

  it('returns -1 when no gateway info available', () => {
    const nq = makeNq({ gatewayLatency: -1 });
    expect(extractGatewayLatency(nq)).toBe(-1);
  });
});

describe('extractExternalLatency', () => {
  it('returns -1 when nq is null', () => {
    expect(extractExternalLatency(null)).toBe(-1);
  });

  it('returns averageExternalLatency when >= 0', () => {
    const nq = makeNq({ averageExternalLatency: 30, externalLatency: 40 });
    expect(extractExternalLatency(nq)).toBe(30);
  });

  it('falls back to externalLatency when average is < 0', () => {
    const nq = makeNq({ averageExternalLatency: -1, externalLatency: 40 });
    expect(extractExternalLatency(nq)).toBe(40);
  });

  it('computes median from external details excluding gateway', () => {
    const nq = makeNq({
      averageExternalLatency: -1,
      externalLatency: -1,
      details: {
        gateway: { target: 'gw', latency: 5, type: 'tcp' },
        a: { target: 'a', latency: 30, type: 'tcp' },
        b: { target: 'b', latency: 10, type: 'tcp' },
        c: { target: 'c', latency: 50, type: 'tcp' },
      },
    });
    // sorted external latencies: [10, 30, 50], median index = floor(3/2) = 1 -> 30
    expect(extractExternalLatency(nq)).toBe(30);
  });

  it('returns -1 when no external latency info available', () => {
    const nq = makeNq({ averageExternalLatency: -1, externalLatency: -1 });
    expect(extractExternalLatency(nq)).toBe(-1);
  });
});
