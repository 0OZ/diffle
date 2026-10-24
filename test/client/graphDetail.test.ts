import { describe, expect, it } from 'vitest';
import {
  announcement,
  detailOf,
  exampleOf,
  keepSelection,
  STRONGEST_SHOWN,
} from '@/client/connected/dialog/GraphDetail.js';
import type { GraphLayout, PlacedNode, RoutedEdge } from '@/client/connected/graph/layout.js';

const node = (id: string, extra: Partial<PlacedNode> = {}): PlacedNode => ({
  id,
  name: id,
  dir: '',
  status: 'modified',
  add: 0,
  del: 0,
  out: 0,
  in: 0,
  test: false,
  x: 0,
  y: 0,
  w: 148,
  h: 48,
  ...extra,
});
const edge = (from: string, to: string, weight = 1): RoutedEdge => ({
  id: `${from}\u0000${to}`,
  from,
  to,
  weight,
  uses: [],
  total: 0,
  context: false,
  points: [],
  label: { x: 0, y: 0 },
});

const layout: GraphLayout = {
  nodes: [node('a'), node('b'), node('c'), node('d')],
  edges: [edge('a', 'b', 1), edge('a', 'c', 4), edge('c', 'b', 2), edge('b', 'a', 2)],
  width: 0,
  height: 0,
};

describe('keepSelection', () => {
  it('keeps a node or arrow the layout holds and drops one it lost', () => {
    const arrow = { kind: 'edge', id: edge('a', 'b').id } as const;
    expect(keepSelection(layout, { kind: 'node', id: 'a' })).toEqual({ kind: 'node', id: 'a' });
    expect(keepSelection(layout, arrow)).toBe(arrow);
    expect(keepSelection(layout, { kind: 'node', id: 'zzz' })).toBeNull();
    expect(keepSelection(layout, { kind: 'edge', id: 'zzz' })).toBeNull();
    expect(keepSelection(layout, null)).toBeNull();
  });
});

describe('detailOf', () => {
  it('shows the strongest arrows first when nothing is selected', () => {
    const d = detailOf(layout, null);
    if (d.kind !== 'overview') throw new Error('expected the overview');
    expect(d.strongest.map((r) => `${r.from.id}>${r.to.id}`)).toEqual(['a>c', 'b>a', 'c>b', 'a>b']);
  });

  it('shows at most a handful of them', () => {
    const many: GraphLayout = {
      ...layout,
      nodes: Array.from({ length: 9 }, (_, i) => node(`n${i}`)),
      edges: Array.from({ length: 8 }, (_, i) => edge(`n${i}`, `n${i + 1}`, i)),
    };
    const d = detailOf(many, null);
    if (d.kind !== 'overview') throw new Error('expected the overview');
    expect(d.strongest).toHaveLength(STRONGEST_SHOWN);
    expect(d.strongest[0]!.edge.weight).toBe(7);
  });

  it('lists what a selected file uses and what uses it, strongest first', () => {
    const d = detailOf(layout, { kind: 'node', id: 'a' });
    if (d.kind !== 'node') throw new Error('expected a node');
    expect(d.node.id).toBe('a');
    expect(d.uses.map((r) => r.to.id)).toEqual(['c', 'b']);
    expect(d.usedBy.map((r) => r.from.id)).toEqual(['b']);
  });

  it('names both ends of a selected arrow', () => {
    const d = detailOf(layout, { kind: 'edge', id: edge('c', 'b').id });
    if (d.kind !== 'edge') throw new Error('expected an arrow');
    expect([d.link.from.id, d.link.to.id]).toEqual(['c', 'b']);
  });

  it('falls back to the overview for a selection the layout no longer has', () => {
    expect(detailOf(layout, { kind: 'node', id: 'zzz' }).kind).toBe('overview');
    expect(detailOf(layout, { kind: 'edge', id: 'zzz' }).kind).toBe('overview');
  });
});

describe('announcement', () => {
  it('says nothing without a selection', () => {
    expect(announcement(layout, null)).toBe('');
    expect(announcement(layout, { kind: 'node', id: 'gone' })).toBe('');
  });

  it('names a file with what it uses and what uses it', () => {
    expect(announcement(layout, { kind: 'node', id: 'a' })).toBe('a selected: uses 2 files, used by 1 file');
    expect(announcement(layout, { kind: 'node', id: 'd' })).toBe('d selected: uses 0 files, used by 0 files');
  });

  it('says an arrow as a sentence', () => {
    expect(announcement(layout, { kind: 'edge', id: edge('c', 'b').id })).toBe('Selected arrow: c uses b');
  });
});

describe('exampleOf', () => {
  it('reads the strongest arrow as a sentence, the worked example of the panel', () => {
    const d = detailOf(layout, null);
    if (d.kind !== 'overview') throw new Error('expected the overview');
    expect(exampleOf(d.strongest)).toBe('Example: a uses c');
  });

  it('is null when the graph has no arrow', () => {
    expect(exampleOf([])).toBeNull();
  });
});
