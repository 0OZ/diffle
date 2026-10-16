import { SegmentedControl, ToggleButton } from '../ui/ToggleButton.js';
import { Button } from '../ui/Button.js';
import type { GitStatusEntry } from '@pierre/trees';
import { FileTree, useFileTree, useFileTreeSearch } from '@pierre/trees/react';
import { ArrowLeftRight, EyeOff } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { ChangedFile, ViewedState } from '../../shared/protocol.js';
import { focusReview } from '../keyboard/useKeymap.js';
import { countViewed, filesUnder, isCollapsed, isViewed, viewedState } from '../model.js';
import { remPx } from '../scale.js';
import { useStore } from '../store.js';
import { ButtonLabel } from '../ui/ButtonLabel.js';
import { useConfirm } from '../useConfirm.js';
import type { SyncKeys } from './sync.js';
import { decorationKey, directoriesOf, expandedAfterReset, statusKey, syncStep, toGitStatus } from './sync.js';
import { TREE_CSS } from './treeCss.js';
import { toggleDirectoryViewed, TreeMenu } from './TreeMenu.js';

type Scope = 'changed' | 'all';

// The decoration lane holds one span of colored text parts: the file's +/− counts, then
// the viewed mark as a glyph (the lane takes either text or one icon, not both).
const MARK: Record<ViewedState, { glyph: string; color: string; title: string }> = {
  viewed: {
    glyph: '✓',
    color: 'var(--add)',
    title: 'Viewed · click or v to toggle',
  },
  unviewed: {
    glyph: '○',
    color: 'var(--fg-2)',
    title: 'Not viewed · click or v to toggle',
  },
  restale: {
    glyph: '◐',
    color: 'var(--warn)',
    title: 'Changed since you viewed it · click or v to mark viewed again',
  },
};

/** Whether the tree's own ellipsis is showing on the row's name. */
function ellipsized(row: HTMLElement): boolean {
  return Array.from(row.querySelectorAll('[data-truncate-marker]')).some((m) => getComputedStyle(m).opacity !== '0');
}

/** Whether a pinned lane covers the tail of the row's name, or the pane's scroll offset hides its head. */
function nameClipped(row: HTMLElement): boolean {
  const name = row.querySelector('[data-item-section="content"]');
  const pane = row.closest('[data-file-tree-virtualized-scroll]');
  if (!name || !pane) return false;
  const { left, right } = name.getBoundingClientRect();
  const view = pane.getBoundingClientRect();
  const lane = row.querySelector('[data-item-section="decoration"], [data-item-section="git"]');
  const edge = lane ? lane.getBoundingClientRect().left : view.left + pane.clientWidth;
  return right > edge + 0.5 || left < view.left - 0.5;
}

function rowDecoration(f: ChangedFile, state: ViewedState) {
  const parts: { text: string; color?: string }[] = [];
  if (f.additions > 0) parts.push({ text: `+${f.additions}`, color: 'var(--add)' });
  if (f.deletions > 0) parts.push({ text: `−${f.deletions}`, color: 'var(--del)' });
  const mark = MARK[state];
  parts.push({ text: mark.glyph, color: mark.color });
  const counts = f.binary ? 'binary' : `+${f.additions} −${f.deletions}`;
  return {
    text: parts.map((p) => p.text).join(' '),
    parts,
    title: `${counts} · ${mark.title}`,
  };
}

// A directory's mark sums its changed files: a check once all are viewed, else the progress so far.
function directoryDecoration(viewedCount: number, total: number) {
  if (viewedCount === total) {
    const title = `All ${total} changed file(s) viewed · click to mark them not viewed`;
    return {
      text: MARK.viewed.glyph,
      parts: [{ text: MARK.viewed.glyph, color: MARK.viewed.color }],
      title,
    };
  }
  const parts: { text: string; color?: string }[] = [];
  if (viewedCount > 0) parts.push({ text: `${viewedCount}/${total}`, color: 'var(--fg-2)' });
  parts.push({ text: MARK.unviewed.glyph, color: MARK.unviewed.color });
  const title = `${viewedCount} of ${total} changed file(s) viewed · click to mark all viewed`;
  return { text: parts.map((p) => p.text).join(' '), parts, title };
}

export function FileTreePane() {
  const snapshot = useStore((s) => s.snapshot);
  const openFile = useStore((s) => s.openFile);
  const [scope, setScope] = useState<Scope>('changed');
  const setViewedMany = useStore((s) => s.setViewedMany);
  const viewed = useStore((s) => s.viewed);
  const config = useStore((s) => s.config);
  // Memoised: a raw selector would rescan every changed file on each store update, including cursor moves.
  const viewedCount = useMemo(
    () => (snapshot ? countViewed({ viewed, config }, snapshot.changed) : 0),
    [snapshot, viewed, config],
  );
  const unview = useConfirm(() => void setViewedMany(snapshot?.changed.map((f) => f.path) ?? [], false));

  const paths = useMemo(() => {
    if (!snapshot) return [] as string[];
    return scope === 'changed' ? snapshot.changed.map((f) => f.path) : snapshot.tree;
  }, [snapshot, scope]);

  const gitStatus = useMemo<GitStatusEntry[]>(() => snapshot?.changed.map(toGitStatus) ?? [], [snapshot]);

  const openRef = useRef(openFile);
  openRef.current = openFile;

  const { model } = useFileTree({
    paths,
    gitStatus,
    // Explicit expansion, not 'open': the library ignores `initialExpandedPaths` under 'open',
    // and the sync effect below needs it to carry collapsed folders across `resetPaths`.
    initialExpansion: 'closed',
    initialExpandedPaths: directoriesOf(paths),
    search: true,
    // The tree sizes rows and paddings in px; derive both from the root font size so it follows the app's scale.
    itemHeight: Math.round(1.875 * remPx()),
    density: remPx() / 16,
    icons: { set: 'standard' },
    unsafeCSS: TREE_CSS,
    onSelectionChange: (selected) => {
      const path = selected[0];
      // The active file is mirrored into the selection below; only a change of file is a request to open one.
      if (path && !path.endsWith('/') && path !== useStore.getState().activePath) void openRef.current(path);
    },
    // Counts and viewed mark in the decoration lane; reads live state at render time.
    // Clicks are handled below, since decorations are plain text.
    renderRowDecoration: ({ item }) => {
      const s = useStore.getState();
      const changed = s.snapshot?.changed ?? [];
      if (item.kind === 'directory') {
        const files = filesUnder(changed, item.path);
        return files.length ? directoryDecoration(countViewed(s, files), files.length) : null;
      }
      const f = changed.find((x) => x.path === item.path);
      return f ? rowDecoration(f, viewedState(s, f)) : null;
    },
  });

  const search = useFileTreeSearch(model);
  const searchEmpty = search.value.trim().length > 0 && search.matchingPaths.length === 0;

  // A click on the decoration toggles viewed without selecting (or, on a directory, folding) the row. The tree renders in
  // a shadow root, so the hit test walks the composed path from the pointer target. Only the
  // decoration's own <span>s count: a click on the lane's padding must select the row, not toggle viewed.
  const bodyRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bodyRef.current?.firstElementChild?.toggleAttribute('data-search-empty', searchEmpty);
  }, [searchEmpty]);
  const scrolls = useStore((s) => s.layout.treeScroll);
  const setLayout = useStore((s) => s.setLayout);
  useEffect(() => {
    bodyRef.current?.firstElementChild?.toggleAttribute('data-hscroll', scrolls);
  }, [scrolls]);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    // The library clears search on blur, even with its 'retain' option after typing.
    // Keep the filter while reading the diff; Escape still explicitly closes it.
    const retainSearch = (event: Event) => {
      const target = event.composedPath()[0];
      if (target instanceof Element && target.matches('[data-file-tree-search-input]')) event.stopPropagation();
    };
    const onSearchKeyDown = (event: KeyboardEvent) => {
      if (
        model.getSearchValue().trim() &&
        model.getSearchMatchingPaths().length === 0 &&
        ['Enter', 'ArrowUp', 'ArrowDown'].includes(event.key)
      ) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (event.key !== 'Enter' || event.isComposing || !model.isSearchOpen()) return;
      const path = model.getFocusedPath();
      if (!path || path.endsWith('/')) return;
      event.preventDefault();
      event.stopPropagation();
      const s = useStore.getState();
      if (isCollapsed(s, path)) s.toggleCollapsed(path);
      void openRef.current(path);
      // The tree re-focuses its input when it renders; hand focus over after that update.
      requestAnimationFrame(focusReview);
    };
    body.addEventListener('blur', retainSearch, true);
    body.addEventListener('keydown', onSearchKeyDown, true);
    return () => {
      body.removeEventListener('blur', retainSearch, true);
      body.removeEventListener('keydown', onSearchKeyDown, true);
    };
  }, [model]);

  useEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const decorationClick = (e: Event): { path: string; directory: boolean } | null => {
      const path = e.composedPath() as HTMLElement[];
      const lane = path.findIndex((n) => n instanceof HTMLElement && n.dataset.itemSection === 'decoration');
      if (lane <= 0) return null;
      const row = path.find((n) => n instanceof HTMLElement && n.dataset.itemPath != null);
      const p = row?.dataset.itemPath;
      return p ? { path: p, directory: row.dataset.itemType === 'folder' } : null;
    };
    const onPointerDown = (e: PointerEvent) => {
      if (e.button !== 0 || !decorationClick(e)) return;
      e.preventDefault();
      e.stopPropagation();
    };
    const onClick = (e: MouseEvent) => {
      const hit = decorationClick(e);
      if (!hit) {
        const row = (e.composedPath() as HTMLElement[]).find(
          (n) => n instanceof HTMLElement && n.dataset.itemType === 'file',
        );
        const path = row?.dataset.itemPath;
        if (!path) return;
        // Choosing a file is the start of reading it: expand it and hand the keys to the review pane.
        // The tree focuses its row on click, so hand focus over a frame later.
        requestAnimationFrame(focusReview);
        const s = useStore.getState();
        if (isCollapsed(s, path)) s.toggleCollapsed(path);
        // Re-clicking the active file changes no selection, so `onSelectionChange` stays silent.
        if (path === s.activePath) void openRef.current(path);
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      if (hit.directory) return toggleDirectoryViewed(hit.path);
      const s = useStore.getState();
      const f = s.snapshot?.changed.find((x) => x.path === hit.path);
      if (f) void s.setViewed(hit.path, !isViewed(s, f));
    };
    el.addEventListener('pointerdown', onPointerDown, true);
    el.addEventListener('click', onClick, true);
    return () => {
      el.removeEventListener('pointerdown', onPointerDown, true);
      el.removeEventListener('click', onClick, true);
    };
  }, []);

  // A long name is cut by the tree's ellipsis, or, scrolling sideways, by the pinned lanes over its
  // tail and the scroll offset over its head. On entry into a cut row, its full path becomes the
  // row's `title`, and `TooltipHost` reads it right after: this capture listener on the shadow root
  // runs before the host's. Rows are recycled across paths, so nothing is cached.
  useEffect(() => {
    const shadow = bodyRef.current?.firstElementChild?.shadowRoot;
    if (!shadow) return;
    const searchInput = shadow.querySelector<HTMLInputElement>('[data-file-tree-search-input]');
    if (searchInput) searchInput.placeholder = 'Search files…';
    const onEnter = (e: Event) => {
      const row = (e.composedPath() as HTMLElement[]).find(
        (n) => n instanceof HTMLElement && n.dataset.itemPath != null,
      );
      const from = (e as PointerEvent | FocusEvent).relatedTarget;
      // Crossing between a row's own parts changes nothing.
      if (!row || (from instanceof Node && row.contains(from))) return;
      const cut = shadow.host.hasAttribute('data-hscroll') ? nameClipped(row) : ellipsized(row);
      if (cut) row.title = (row.dataset.itemPath ?? '').replace(/\/$/, '');
      else row.removeAttribute('title');
    };
    shadow.addEventListener('pointerover', onEnter, true);
    shadow.addEventListener('focusin', onEnter, true);
    return () => {
      shadow.removeEventListener('pointerover', onEnter, true);
      shadow.removeEventListener('focusin', onEnter, true);
    };
  }, []);

  const setTreeModel = useStore((s) => s.setTreeModel);
  useEffect(() => {
    setTreeModel(model);
    return () => setTreeModel(null);
  }, [model, setTreeModel]);

  // Keep the model in sync when the snapshot, scope, viewed marks or auto-viewed patterns change.
  // `syncStep` picks the update by content, so a save that leaves the rows alone redraws nothing.
  const keys = useMemo<SyncKeys>(
    () => ({
      paths: paths.join('\n'),
      status: statusKey(snapshot?.changed ?? []),
      decoration: snapshot ? decorationKey({ viewed, config }, snapshot.changed) : '',
    }),
    [paths, snapshot, viewed, config],
  );
  const synced = useRef<SyncKeys | null>(null);
  const syncedScope = useRef(scope);
  useEffect(() => {
    const step = syncStep(synced.current, keys);
    synced.current = keys;
    // A scope switch starts the folders afresh: the whole repository opens folded down to the active file.
    if (syncedScope.current !== scope) {
      syncedScope.current = scope;
      const active = useStore.getState().activePath;
      model.resetPaths(paths, {
        initialExpandedPaths: directoriesOf(scope === 'all' ? (active ? [active] : []) : paths),
      });
      model.setGitStatus(gitStatus);
      return;
    }
    switch (step) {
      case 'reset':
        model.resetPaths(paths, {
          initialExpandedPaths: expandedAfterReset(model, paths),
        });
        model.setGitStatus(gitStatus); // a reset draws every row afresh
        break;
      case 'status':
        model.setGitStatus(gitStatus); // redraws the rows, decorations included
        break;
      case 'decoration':
        // `setGitStatus` no-ops while its signature is unchanged; re-applying the composition redraws the rows.
        model.setComposition(model.getComposition());
        break;
      case 'none':
        break;
    }
  }, [model, keys, paths, gitStatus, scope]);

  // The selected row follows the file under the cursor, so the tree always shows where the reader is.
  const activePath = useStore((s) => s.activePath);
  useEffect(() => {
    const item = model.getItem(activePath ?? '');
    if (!activePath || !item) return;
    for (const p of model.getSelectedPaths()) if (p !== activePath) model.getItem(p)?.deselect();
    item.select();
    model.scrollToPath(activePath);
  }, [model, activePath, paths]);

  return (
    <aside className="tree-theme flex min-h-0 flex-col bg-surface">
      <div className="@container flex items-center gap-2 border-b border-b-border px-2.5 py-1.5 text-muted [&_button]:flex-none [&_button]:whitespace-nowrap">
        <SegmentedControl>
          <ToggleButton selected={scope === 'changed'} onClick={() => setScope('changed')}>
            Changed
          </ToggleButton>
          <ToggleButton selected={scope === 'all'} onClick={() => setScope('all')}>
            All files
          </ToggleButton>
        </SegmentedControl>
        {/* The count goes first when the bar is too narrow for every control: the buttons stay reachable. */}
        <div className="ml-auto flex items-center gap-2">
          <span className="@max-[15rem]:hidden">{paths.length}</span>
          <Button
            variant="ghost"
            icon
            className={scrolls ? '' : 'opacity-45'}
            aria-label="Scroll long names"
            aria-pressed={scrolls}
            title={
              scrolls
                ? 'Long names scroll sideways: click to shorten them instead'
                : 'Long names are shortened: click to scroll them sideways'
            }
            onClick={() => setLayout({ treeScroll: !scrolls })}
          >
            <ArrowLeftRight size="0.875rem" />
          </Button>
          <Button
            variant="ghost"
            danger={unview.armed}
            icon={!unview.armed}
            feedback={unview.armed ? 'confirm' : undefined}
            disabled={viewedCount === 0}
            title={
              unview.armed
                ? 'Click again to mark all files not viewed'
                : `Mark all ${viewedCount} viewed file(s) not viewed`
            }
            onClick={unview.fire}
          >
            <EyeOff size="0.875rem" />
            <ButtonLabel text={unview.armed && 'Un-view all?'} />
          </Button>
        </div>
      </div>
      <div className="relative min-h-0 flex-1" ref={bodyRef}>
        <FileTree
          model={model}
          className="h-full"
          renderContextMenu={(item, ctx) => <TreeMenu path={item.path} kind={item.kind} close={ctx.close} />}
        />
        {searchEmpty && (
          <div role="status" className="absolute inset-x-0 top-14 pr-2.5 pl-[calc(1.5rem+1px)] text-muted">
            No matching files
          </div>
        )}
      </div>
    </aside>
  );
}
