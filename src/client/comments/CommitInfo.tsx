import { Check } from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { twMerge } from 'tailwind-merge';
import { type RangeCommit } from '@/shared/protocol.js';
import { copyText } from '@/client/clipboard.js';
import { commitBody } from '@/client/model.js';

import { Button } from '@/client/ui/button.js';

const DATE = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const DATE_TIME = new Intl.DateTimeFormat(undefined, { dateStyle: 'full', timeStyle: 'long' });

/** How long a copied hash shows its check mark. */
const COPIED_MS = 1400;

/** The short hash; a click copies the full one and swaps the hash for a check mark, as `yy` does its button. */
export function CopyHash({ commit }: { commit: Pick<RangeCommit, 'sha' | 'short'> }) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const transition = '[transition:opacity_160ms_ease,transform_200ms_cubic-bezier(0.2,0.8,0.2,1)]';
  return (
    <Button
      variant="ghost"
      className="relative mt-0.5 flex-none px-1 py-px font-mono text-[0.75rem] text-muted-foreground hover:text-foreground"
      aria-label={done ? `Copied ${commit.short}` : `Copy hash ${commit.short}`}
      onClick={async () => {
        if (!(await copyText(commit.sha))) return;
        setDone(true);
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setDone(false), COPIED_MS);
      }}
      size="sm"
    >
      <span className={twMerge(transition, done && '[transform:scale(0.6)] opacity-0')}>{commit.short}</span>
      <span
        aria-hidden
        className={twMerge(
          'absolute inset-0 flex items-center justify-center text-add',
          transition,
          done ? '[transform:scale(1)_rotate(0)] opacity-100' : '[transform:scale(0.4)_rotate(-30deg)] opacity-0',
        )}
      >
        <Check size="0.875rem" strokeWidth={3} />
      </span>
    </Button>
  );
}

export function subjectOf(commit: RangeCommit): string {
  return commit.message.split('\n', 1)[0] || '(Empty commit message)';
}

/** A commit's body and author line; `full` adds the email, the time and the hash, for the hover card. */
export function CommitDetails({ commit, full = false }: { commit: RangeCommit; full?: boolean }) {
  const time = (
    <time className="flex-none" dateTime={new Date(commit.date).toISOString()} title={DATE_TIME.format(commit.date)}>
      {(full ? DATE_TIME : DATE).format(commit.date)}
    </time>
  );
  return (
    <div className="leading-[1.4]">
      {commitBody(commit.message).map((paragraph, i) => (
        <p key={i} className="m-0 mb-1.5 wrap-anywhere whitespace-pre-wrap text-muted-foreground">
          {paragraph}
        </p>
      ))}
      {full ? (
        <div className="text-[0.75rem] text-muted-foreground">
          <p className="m-0 wrap-anywhere">
            {commit.author} &lt;{commit.email}&gt;
          </p>
          <p className="m-0 flex items-center gap-1.5">
            {time}
            <span aria-hidden>·</span>
            <span className="font-mono text-foreground">{commit.short}</span>
          </p>
        </div>
      ) : (
        <p className="m-0 flex min-w-0 items-center gap-1.5 text-[0.75rem] text-muted-foreground">
          <span className="min-w-0 truncate" title={`${commit.author} <${commit.email}>`}>
            {commit.author}
          </span>
          <span aria-hidden>·</span>
          {time}
        </p>
      )}
    </div>
  );
}

/** The hovered commit in full, beside the panel and level with its row, kept inside the viewport. */
export function CommitCard({ id, commit, row }: { id: string; commit: RangeCommit; row: DOMRect }) {
  const card = useRef<HTMLDivElement>(null);
  const [top, setTop] = useState(row.top);
  useLayoutEffect(() => {
    const height = card.current?.offsetHeight ?? 0;
    setTop(Math.max(8, Math.min(row.top, window.innerHeight - height - 8)));
  }, [row]);
  return (
    <div
      ref={card}
      id={id}
      role="tooltip"
      className="pointer-events-none fixed z-40 max-h-[calc(100vh-1rem)] w-[26rem] max-w-[calc(100vw-1rem)] overflow-hidden rounded-lg border border-border bg-background px-3 py-2.5 text-[0.8125rem] shadow-[0_0.5rem_1.5rem_rgba(0,_0,_0,_0.18)]"
      style={{ top, right: window.innerWidth - row.left + 8 }}
    >
      <p className="m-0 mb-1.5 leading-[1.4] font-semibold wrap-anywhere">{subjectOf(commit)}</p>
      <CommitDetails commit={commit} full />
    </div>
  );
}
