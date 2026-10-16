// @vitest-environment jsdom
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import type { LspServerStatus, LspStatus } from '@/shared/protocol.js';

const { state } = vi.hoisted(() => ({
  state: {
    snapshot: null,
    github: { status: 'idle' },
    lsp: { enabled: true, servers: [], missing: [] } as LspStatus,
    layout: { treeVisible: true, panelVisible: true },
    theme: 'dark',
    diffStyle: 'split',
    jumps: [],
    jumpIndex: 0,
    fileView: null,
  },
}));
vi.mock('@/client/store.js', () => ({ useStore: (select: (s: typeof state) => unknown) => select(state) }));
vi.mock('@/client/header/ModePicker.js', () => ({ ModePicker: () => null }));
vi.mock('@/client/header/SettingsDialog.js', () => ({ SettingsDialog: () => null }));
vi.mock('@/client/clipboard.js', () => ({ copyText: vi.fn() }));
import { copyText } from '@/client/clipboard.js';
import { Header } from '@/client/header/Header.js';

function setServers(...statuses: Partial<LspServerStatus>[]) {
  state.lsp.servers = statuses.map((status, i) => ({
    name: `server${i}`,
    command: `server${i}`,
    languages: ['python'],
    state: 'ready',
    ...status,
  }));
}

function render(...statuses: Partial<LspServerStatus>[]) {
  setServers(...statuses);
  const container = document.createElement('div');
  container.innerHTML = renderToStaticMarkup(createElement(Header));
  return container;
}

const trigger = (container: Element) =>
  container.querySelector<HTMLButtonElement>('button[aria-label^="Language servers"]')!;
// The popover renders into a portal on the body, outside the header's container.
const popup = () => document.querySelector<HTMLElement>('[aria-label="Language server status"]');

/** Mounts the header, opens the status popover, and expands every server's details. */
async function mount(...statuses: Partial<LspServerStatus>[]) {
  setServers(...statuses);
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  await act(async () => root.render(createElement(Header)));
  return {
    container,
    open: async () => {
      await act(async () => trigger(container).click());
      return popup()!;
    },
    expand: async () => {
      for (const details of popup()!.querySelectorAll<HTMLElement>('[data-slot="collapsible-trigger"]'))
        await act(async () => details.click());
    },
    unmount: async () => {
      await act(async () => root.unmount());
      container.remove();
    },
  };
}

describe('LSP status indicator', () => {
  it('dismisses on outside click, Escape, and the close button, returning focus to the trigger', async () => {
    const view = await mount();
    try {
      await view.open();
      expect(popup()).not.toBeNull();
      await act(async () => {
        popup()!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerType: 'mouse', button: 0 }));
      });
      expect(popup()).not.toBeNull();
      // The popover starts listening for outside presses a moment after it opens.
      await act(() => new Promise((resolve) => setTimeout(resolve, 50)));
      await act(async () => {
        document.body.dispatchEvent(
          new PointerEvent('pointerdown', { bubbles: true, pointerType: 'mouse', button: 0 }),
        );
        document.body.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
      });
      expect(popup()).toBeNull();

      await view.open();
      await act(async () => {
        document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
      });
      expect(popup()).toBeNull();
      expect(document.activeElement).toBe(trigger(view.container));

      await view.open();
      await act(async () =>
        document.querySelector<HTMLButtonElement>('[aria-label="Close language server status"]')!.click(),
      );
      expect(popup()).toBeNull();
      expect(document.activeElement).toBe(trigger(view.container));
    } finally {
      await view.unmount();
    }
  });
  // Header's trigger names the most severe state across all servers.
  it.each<[Partial<LspServerStatus>[], string]>([
    [[{}], 'Language servers'],
    [[{}, { stderr: 'log' }], 'Language servers: logs'],
    [[{ stderr: 'log' }, { activity: ['Indexing'] }], 'Language servers: busy'],
    [[{ activity: ['Indexing'] }, { state: 'starting' }], 'Language servers: starting'],
    [[{ state: 'starting' }, { notice: { severity: 'warning', message: 'w' } }], 'Language servers: warning'],
    [
      [{ notice: { severity: 'warning', message: 'w' } }, { notice: { severity: 'error', message: 'e' } }],
      'Language servers: error',
    ],
    [[{ notice: { severity: 'error', message: 'e' } }, { state: 'unavailable' }], 'Language servers: unavailable'],
  ])('labels %j as %s', (servers, label) => {
    expect(trigger(render(...servers)).getAttribute('aria-label')).toBe(label);
  });

  it('shows reported work, notices, and stderr instead of claiming readiness', async () => {
    const view = await mount(
      { activity: ['Loading workspace: dependencies'] },
      { notice: { severity: 'error', message: 'Workspace loading failed' }, stderr: 'Missing build tool' },
      {},
    );
    try {
      await view.open();
      await view.expand();
      const text = popup()!.textContent;
      expect(text).toContain('Loading workspace: dependencies');
      expect(text).toContain('Working');
      expect(text).toContain('Workspace loading failed');
      expect(text).toContain('Missing build tool');
      expect(text).toContain('Connected');
    } finally {
      await view.unmount();
    }
  });

  it('names missing servers compactly with install candidates in the tooltip', async () => {
    state.lsp.missing = [
      { language: 'rust', tried: ['rust-analyzer'] },
      { language: 'python', tried: [] },
    ];
    const view = await mount();
    try {
      const open = await view.open();
      expect(open.textContent).toContain('Not on PATH');
      expect(open.textContent).toContain('Disabled');
      expect(open.querySelector('[title="rust-analyzer"]')!.textContent).toContain('rust');
    } finally {
      state.lsp.missing = [];
      await view.unmount();
    }
  });

  it.each([true, false])('reports clipboard success=%s and copies the full log', async (success) => {
    const text = 'first line\n' + 'long line '.repeat(200);
    vi.mocked(copyText).mockResolvedValue(success);
    const view = await mount({ stderr: text });
    try {
      await view.open();
      await view.expand();
      await act(async () => popup()!.querySelector<HTMLButtonElement>('[aria-label="Copy stderr"]')!.click());
      expect(copyText).toHaveBeenLastCalledWith(text);
      expect(popup()!.textContent).toContain(success ? 'Copied' : 'Copy failed. Select the log');
      expect(popup()!.querySelector('pre[aria-label="stderr log"]')!.textContent).toBe(text);
    } finally {
      await view.unmount();
    }
  });
});
