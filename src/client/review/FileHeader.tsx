import { Button } from '@/client/ui/button.js';

import {
  ChevronDown,
  ChevronRight,
  Copy,
  Download,
  FileText,
  MessageSquare,
  MessageSquarePlus,
  RefreshCw,
} from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { twMerge } from 'tailwind-merge';

import { copyText } from '@/client/clipboard.js';
import { FilePath } from '@/client/FilePath.js';
import { CopyIcon } from '@/client/components/CopyIcon.js';

import { SearchBar } from '@/client/keyboard/SearchBar.js';

import { isCollapsed, pathFromItemId, viewedState, commentsOff } from '@/client/model.js';

import { useStore } from '@/client/store.js';

import { hostOf } from './host.js';

import { Checkbox } from '@/client/ui/checkbox.js';
import { Label } from '@/client/ui/label.js';

export function FileHeader({ id, resizeHeader }: { id: string; resizeHeader: (id: string, height: number) => void }) {
  const path = pathFromItemId(id);
  const ref = useRef<HTMLDivElement>(null);
  const localSearch = useStore(
    (s) => s.search.open && s.search.kind === 'text' && s.search.scope === 'file' && s.search.path === path,
  );
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    let previous = 0;
    const measure = () => {
      const height = element.getBoundingClientRect().height + 1; // the header's bottom border
      if (height <= 1 || height === previous) return;
      previous = height;
      resizeHeader(id, height);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [id, resizeHeader, localSearch]);
  const file = useStore((s) => s.snapshot?.changed.find((f) => f.path === path));
  return (
    // `data-path` names the file for tests and scripts; the visible title splits it into styled parts.
    <div ref={ref} data-path={path}>
      <div className="group flex h-[calc(var(--diffle-header-height)-1px)] items-center gap-2 px-2.5 py-1.5">
        <FileText size="0.875rem" className="shrink-0 text-muted-foreground" />
        {file?.oldPath && file.oldPath !== path && (
          <>
            <FilePath path={file.oldPath} nowrap tooltip={false} />
            <span className="text-muted-foreground">→</span>
          </>
        )}
        {/* The copy button beside it already hands over the full path; a hover tip would only repeat it. */}
        <FilePath path={path} nowrap tooltip={false} />
        <CopyPathButton path={path} />
        <span className="mr-auto" />
        {file && (
          <span className="flex shrink-0 gap-2 font-mono text-[0.75rem]">
            <span className="text-del">−{file.deletions}</span>
            <span className="text-add">+{file.additions}</span>
          </span>
        )}
        <FileHeaderMeta path={path} />
      </div>
      <SearchBar path={path} />
    </div>
  );
}

/** Copies the path for quoting it in a comment; shown only while the header is hovered or the button focused. */
function CopyPathButton({ path }: { path: string }) {
  const flash = useStore((s) => s.flash);
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => void (timer.current && clearTimeout(timer.current)), []);
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      className={twMerge('shrink-0 opacity-0 group-hover:opacity-100 focus-visible:opacity-100', done && 'opacity-100')}
      title="Copy path (yp)"
      aria-label="Copy path"
      onClick={async () => {
        if (!(await copyText(path))) return flash('The browser blocked clipboard access');
        setDone(true);
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => setDone(false), 1400);
      }}
    >
      <CopyIcon done={done}>
        <Copy size="0.875rem" />
      </CopyIcon>
    </Button>
  );
}

function FileHeaderMeta({ path }: { path: string }) {
  const file = useStore((s) => s.snapshot?.changed.find((f) => f.path === path));
  const active = useStore((s) => s.activePath === path);
  const ref = useRef<HTMLSpanElement | null>(null);
  // The header lives in the viewer's shadow DOM; flag its host so the injected CSS can tint the active file.
  useEffect(() => {
    const host = ref.current && hostOf(ref.current);
    if (!host) return;
    host.toggleAttribute('data-active', active);
    return () => host.removeAttribute('data-active');
  }, [active]);
  const toggleCollapsed = useStore((s) => s.toggleCollapsed);
  const selectFile = useStore((s) => s.selectFile);
  const full = useStore((s) => s.fileView?.path === path);
  useEffect(() => {
    const metadata = ref.current;
    const host = metadata && hostOf(metadata);
    const header =
      host?.shadowRoot?.querySelector<HTMLElement>('[data-diffs-header]') ??
      metadata?.closest<HTMLElement>('[data-diffs-header]');
    if (!header) return;
    // Full-file view has no collapse action, so drop the pointer cue that would promise one. `auto` keeps the
    // text cursor over the filename while the surrounding band shows the plain arrow.
    if (full) {
      header.style.cursor = 'auto';
      return () => {
        header.style.cursor = '';
      };
    }
    // The header is the file's one clickable surface, so clicking it also makes the file the current one:
    // an empty file has no lines to click, and the pane must not wander off to another file (issue #151).
    const toggle = (event: MouseEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest('button, input, label, a, form, [role="button"]')) return;
      // Dragging across the path selects it for copying; only a plain click toggles.
      if (!(window.getSelection()?.isCollapsed ?? true)) return;
      selectFile(path);
      toggleCollapsed(path);
    };
    header.addEventListener('click', toggle);
    return () => header.removeEventListener('click', toggle);
  }, [path, toggleCollapsed, selectFile, full]);
  const vs = useStore((s) => (file ? viewedState(s, file) : 'unviewed'));
  const setViewed = useStore((s) => s.setViewed);
  const count = useStore((s) => s.threads.filter((t) => t.anchor.path === path && !t.resolved).length);
  const collapsedNow = useStore((s) => isCollapsed(s, path));
  const openFullFile = useStore((s) => s.openFullFile);
  const oversized = useStore((s) => s.loaded[path]?.kind === 'oversized');
  const loadPatch = useStore((s) => s.loadPatch);
  const openFileDraft = useStore((s) => s.openFileDraft);
  const external = useStore((s) => s.fileView?.external === true && s.fileView.path === path);
  const commentable = useStore((s) => s.snapshot == null || commentsOff(s.snapshot.mode) == null);
  return (
    <span ref={ref} className="inline-flex items-center gap-2.5 font-mono text-[0.75rem]">
      {count > 0 && (
        <span className="inline-flex items-center gap-0.75 rounded-[0.625rem] bg-accent px-1.75 py-0">
          <MessageSquare size="0.75rem" /> {count}
        </span>
      )}
      {file?.generated && (
        <span
          className="inline-flex items-center gap-0.75 rounded-[0.625rem] bg-accent px-1.75 py-0 font-sans text-muted-foreground"
          title="Detected as generated: starts collapsed and ranks low"
        >
          generated
        </span>
      )}
      {file?.submodule && (
        <span
          className="inline-flex items-center gap-0.75 rounded-[0.625rem] bg-accent px-1.75 py-0 font-sans text-muted-foreground"
          title="Submodule: only the recorded commit changes"
        >
          submodule
        </span>
      )}
      {oversized && (
        <Button variant="ghost" onClick={() => void loadPatch(path)} title="Large diff, not loaded yet (zo)" size="sm">
          <Download size="0.875rem" /> Load diff
        </Button>
      )}
      {file && vs === 'restale' && (
        <span
          className="inline-flex items-center gap-0.75 rounded-[0.625rem] bg-accent px-1.75 py-0 font-sans text-warn"
          title="You marked this viewed, then its contents changed"
        >
          <RefreshCw size="0.75rem" /> changed since viewed
        </span>
      )}
      {!external && commentable && (
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => openFileDraft(path)}
          title="Comment on this file as a whole (C)"
        >
          <MessageSquarePlus size="0.875rem" />
        </Button>
      )}
      {/* In the file view the bar above already carries the way back, so the header offers no second button. */}
      {!full && (!file || (!file.binary && !file.submodule && file.status !== 'D')) && (
        <Button variant="ghost" size="icon-sm" onClick={() => void openFullFile(path)} title="View full file (F)">
          <FileText size="0.875rem" />
        </Button>
      )}
      {file && !full && (
        <>
          <Label
            className="cursor-pointer gap-1 font-sans text-[length:inherit] font-normal"
            title={
              vs === 'restale'
                ? 'Mark viewed again and collapse the file (v)'
                : 'Mark as viewed and collapse the file (v)'
            }
          >
            <Checkbox checked={vs === 'viewed'} onCheckedChange={(checked) => void setViewed(path, checked)} />
            Viewed
          </Label>
        </>
      )}
      {!full && (
        <Button variant="ghost" size="icon-sm" onClick={() => toggleCollapsed(path)} title="Collapse / expand">
          {collapsedNow ? <ChevronRight size="0.875rem" /> : <ChevronDown size="0.875rem" />}
        </Button>
      )}
    </span>
  );
}
