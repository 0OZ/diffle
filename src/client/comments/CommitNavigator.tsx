import { ChevronDown, ChevronUp, Circle, CircleDashed, GitCommitHorizontal, Layers } from 'lucide-react';
import { type ComponentProps, type ReactNode, useEffect, useId, useRef, useState } from 'react';
import { twMerge } from 'tailwind-merge';
import {
  comparisonLabel,
  rangeLabel,
  type PairStatus,
  type RangeCommit,
  type RangePair,
  type Snapshot,
} from '@/shared/protocol.js';

import { commitBody, listsWorktree, rangeStep } from '@/client/model.js';
import { useStore } from '@/client/store.js';
import { Button } from '@/client/ui/button.js';
import { CommitCard, CommitDetails, CopyHash, subjectOf } from './CommitInfo.js';

/** Hover this long before a commit's card shows, so sweeping the pointer across the list stays quiet. */
const HOVER_DELAY_MS = 250;

/**
 * The compared range's commits, newest first, under an entry for the range itself and, when the range
 * ends at the worktree, one for its uncommitted changes. Choosing a commit or the uncommitted changes
 * shows that diff alone while the list stays the range's; the shown entry is highlighted with its full
 * message, and hovering another shows that one's in a card. Within an interdiff the list is the
 * range-diff instead: the two iterations' commits paired, under an entry for the interdiff itself.
 */
export function CommitNavigator({ snapshot }: { snapshot: Snapshot }) {
  const { mode, commits } = snapshot;
  const focusCommit = useStore((s) => s.focusCommit);
  const showPair = useStore((s) => s.showPair);
  const stepCommit = useStore((s) => s.stepCommit);
  const interdiff = mode.interdiff ?? null;
  const [open, setOpen] = useState(true);
  const [hover, setHover] = useState<{ commit: RangeCommit; row: DOMRect } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const list = useRef<HTMLDivElement>(null);
  const cardId = useId();
  // A single commit alone has nothing to step through; it is shown as the one active entry.
  const range = mode.within ?? (mode.base === 'parent' ? null : mode);
  const active = mode.pair ? mode.pair.new : mode.base === 'parent' || (mode.within && !interdiff) ? mode.new : null;
  // An interdiff shows neither the range nor one of its commits; "All changes" leads back to the range.
  const whole = active === null && !interdiff;
  const uncommitted = listsWorktree(snapshot);
  // Without a commit, the uncommitted changes are the whole range: one entry, already shown.
  const onlyUncommitted = range?.new === 'worktree' && commits.total === 0;
  const older = commits.total - commits.list.length;

  useEffect(() => {
    const current = list.current?.querySelector<HTMLElement>('[aria-current="true"]');
    current?.scrollIntoView({ block: 'nearest' });
    // Focus left on a row stepped away from would mark it as well as the shown one.
    if (list.current?.contains(document.activeElement) && document.activeElement !== current)
      (current?.matches('button') ? current : current?.querySelector<HTMLElement>('button'))?.focus({
        preventScroll: true,
      });
  }, [active, open]);
  useEffect(() => () => clearTimeout(timer.current), []);

  const hoverStart = (commit: RangeCommit, row: Element) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setHover({ commit, row: row.getBoundingClientRect() }), HOVER_DELAY_MS);
  };
  const hoverEnd = () => {
    clearTimeout(timer.current);
    setHover(null);
  };

  const entry = (current: boolean, onClick: () => void, title: string, label: string) => (
    <Button
      variant="ghost"
      className={twMerge(
        'h-auto w-full justify-start gap-2 rounded-none border-0 py-1 pr-2.5 pl-2 text-left leading-[1.125rem] outline-none focus-visible:bg-accent sm:h-auto',
        current && 'bg-primary/12 shadow-[inset_2px_0_0_var(--accent)] hover:bg-primary/12',
      )}
      aria-current={current ? 'true' : undefined}
      onClick={onClick}
      size="sm"
    >
      <Layers size="0.8125rem" className={current ? 'text-primary' : 'text-muted-foreground'} />
      <span className={twMerge('flex-none', current && 'font-semibold')}>{title}</span>
      <span className="min-w-0 truncate font-mono text-[0.75rem] text-muted-foreground">{label}</span>
    </Button>
  );

  return (
    <section
      className="flex max-h-[40%] shrink-0 flex-col border-b border-b-border text-[0.8125rem]"
      aria-label="Commits"
    >
      <div className="flex min-h-10 flex-none items-center gap-0.5 pr-1.5">
        <Button
          variant="ghost"
          className="h-auto min-w-0 flex-1 justify-start gap-1.5 self-stretch rounded-none border-0 px-2.5 py-1.5 text-left font-semibold outline-none focus-visible:bg-accent sm:h-auto"
          aria-expanded={open}
          title={open ? 'Collapse the commits' : 'Expand the commits'}
          onClick={() => setOpen(!open)}
          size="sm"
        >
          <GitCommitHorizontal size="0.9375rem" /> Commits
          <span className="rounded-[0.625rem] bg-accent px-1.75 py-0 font-medium text-muted-foreground">
            {interdiff ? interdiff.pairs.length : commits.total}
          </span>
        </Button>
        {range && (
          <>
            <Button
              variant="ghost"
              size="icon-sm"
              className="outline-none focus-visible:bg-accent"
              disabled={rangeStep(snapshot, -1) === undefined}
              onClick={() => stepCommit(-1)}
              title="Older commit (<)"
              aria-label="Older commit"
            >
              <ChevronDown size="0.875rem" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              className="outline-none focus-visible:bg-accent"
              disabled={rangeStep(snapshot, 1) === undefined}
              onClick={() => stepCommit(1)}
              title="Newer commit (>)"
              aria-label="Newer commit"
            >
              <ChevronUp size="0.875rem" />
            </Button>
          </>
        )}
      </div>
      {open && (
        <div ref={list} className="min-h-0 overflow-auto pb-1.5" onScroll={hoverEnd}>
          {range && !onlyUncommitted && entry(whole, () => focusCommit(null), 'All changes', comparisonLabel(range))}
          {interdiff &&
            entry(
              active === null,
              () => showPair(null),
              interdiff.from.iteration != null
                ? `Since #${interdiff.from.iteration}`
                : `Since ${rangeLabel(interdiff.from)}`,
              `${interdiff.pairs.filter((p) => p.status !== 'identical').length} of ${interdiff.pairs.length} commits differ`,
            )}
          <ol className="m-0 list-none p-0">
            {(uncommitted || onlyUncommitted) && (
              <BeadRow
                label="Uncommitted changes"
                dashed
                rail={uncommitted}
                active={onlyUncommitted || active === 'worktree'}
                onPick={uncommitted ? () => focusCommit('worktree') : undefined}
              />
            )}
            {interdiff
              ? interdiff.pairs.toReversed().map((pair) => {
                  const commit = pair.new ?? pair.old!;
                  return (
                    <PairRow
                      key={`${pair.old?.sha ?? '-'}:${pair.new?.sha ?? '-'}`}
                      pair={pair}
                      active={pair.new != null && pair.new.sha === active}
                      described={hover?.commit.sha === commit.sha ? cardId : undefined}
                      onPick={pair.new && pair.status !== 'identical' ? () => showPair(pair.new!.sha) : undefined}
                      onHover={(row) => hoverStart(commit, row)}
                      onLeave={hoverEnd}
                    />
                  );
                })
              : commits.list
                  .toReversed()
                  .map((commit) => (
                    <CommitRow
                      key={commit.sha}
                      commit={commit}
                      rail={commits.list.length + (uncommitted ? 1 : 0) > 1}
                      active={commit.sha === active}
                      described={hover?.commit.sha === commit.sha ? cardId : undefined}
                      onPick={range ? () => focusCommit(commit.sha) : undefined}
                      onHover={(row) => hoverStart(commit, row)}
                      onLeave={hoverEnd}
                    />
                  ))}
          </ol>
          {range && !interdiff && commits.total === 0 && !onlyUncommitted && (
            <p className="m-0 px-2.5 py-1 text-[0.75rem] text-muted-foreground">No commits</p>
          )}
          {!interdiff && older > 0 && (
            <p className="m-0 px-2.5 py-1 text-[0.75rem] text-muted-foreground">
              {older} older {older === 1 ? 'commit' : 'commits'} not shown
            </p>
          )}
        </div>
      )}
      {/* The shown commit already lays its message out in the list. */}
      {hover && hover.commit.sha !== active && <CommitCard id={cardId} commit={hover.commit} row={hover.row} />}
    </section>
  );
}

/** How each pair state reads and looks in the list. */
const PAIR_STATUS: Record<PairStatus, { glyph: string; label: string; className: string }> = {
  identical: { glyph: '=', label: 'identical', className: 'text-muted-foreground' },
  changed: { glyph: '!', label: 'amended', className: 'text-primary' },
  message: { glyph: '!', label: 'reworded', className: 'text-primary' },
  added: { glyph: '+', label: 'added', className: 'text-add' },
  dropped: { glyph: '−', label: 'dropped', className: 'text-del' },
};

/** A pair's glyph, subject and state, laid out alike whether the pair opens or not. */
const pairHead =
  'h-auto min-w-0 flex-1 border-0 sm:h-auto items-start gap-2 py-1 pr-1 pl-2.5 justify-start text-left leading-[1.125rem]';

/**
 * One range-diff row: the newer commit's subject (the older one's for a dropped commit), both
 * hashes, and the pair's state. Identical and dropped pairs are shown for orientation but have
 * nothing to open; the shown pair lays out its new message and, when reworded, the old one.
 */
function PairRow({
  pair,
  active,
  described,
  onPick,
  onHover,
  onLeave,
}: {
  pair: RangePair;
  active: boolean;
  described: string | undefined;
  onPick: (() => void) | undefined;
  onHover(row: Element): void;
  onLeave(): void;
}) {
  const status = PAIR_STATUS[pair.status];
  const commit = pair.new ?? pair.old!;
  const reworded = pair.old && pair.new && pair.old.message !== pair.new.message;
  const head = (
    <>
      <span aria-hidden className={twMerge('w-2 flex-none text-center font-mono font-bold', status.className)}>
        {status.glyph}
      </span>
      <span
        className={twMerge('min-w-0 flex-1', active ? 'font-semibold wrap-anywhere whitespace-normal' : 'truncate')}
      >
        {subjectOf(commit)}
      </span>
      <span className={twMerge('flex-none text-[0.75rem]', status.className)}>{status.label}</span>
    </>
  );
  return (
    <li
      className={twMerge(
        'group relative',
        active ? 'bg-primary/12 shadow-[inset_2px_0_0_var(--accent)]' : onPick && 'hover:bg-accent',
        !onPick && 'opacity-70',
      )}
      aria-current={active ? 'true' : undefined}
      onPointerEnter={(e) => onHover(e.currentTarget)}
      onPointerLeave={onLeave}
    >
      <div className="flex items-start pr-1.5">
        {onPick ? (
          <Button
            variant="ghost"
            className={twMerge(
              pairHead,
              'rounded-none border-0 outline-none hover:bg-transparent',
              !active && 'focus-visible:bg-accent',
            )}
            aria-describedby={described}
            onClick={onPick}
            size="sm"
          >
            {head}
          </Button>
        ) : (
          <div className={twMerge('flex', pairHead)}>{head}</div>
        )}
        <span className="flex flex-none items-center font-mono text-[0.75rem] text-muted-foreground">
          {pair.old ? <CopyHash commit={pair.old} /> : <span className="px-1">———————</span>}
          <span aria-hidden>→</span>
          {pair.new ? <CopyHash commit={pair.new} /> : <span className="px-1">———————</span>}
        </span>
      </div>
      {active && (
        <div className="pr-2.5 pb-1.5 pl-6.5">
          <CommitDetails commit={commit} />
          {reworded && pair.old && (
            <div className="mt-1 border-l-2 border-border pl-2 text-muted-foreground">
              <div className="text-[0.75rem] font-semibold">Previous message</div>
              <div className="wrap-anywhere">{subjectOf(pair.old)}</div>
              {commitBody(pair.old.message).map((paragraph, i) => (
                <p key={i} className="m-0 mt-1 wrap-anywhere whitespace-pre-wrap">
                  {paragraph}
                </p>
              ))}
            </div>
          )}
        </div>
      )}
    </li>
  );
}

/**
 * One bead of the commit necklace: a dot on the rail and a label. A dashed dot marks what is not a
 * commit yet. The shown bead is highlighted; `trailing` and `details` follow its label and, shown, sit below it.
 */
function BeadRow({
  label,
  dashed = false,
  rail,
  active,
  described,
  onPick,
  onHover,
  onLeave,
  trailing,
  details,
}: {
  label: string;
  dashed?: boolean;
  rail: boolean;
  active: boolean;
  described?: string;
  /** Absent when the bead cannot be focused: it is the comparison itself. */
  onPick: (() => void) | undefined;
  onHover?(row: Element): void;
  onLeave?(): void;
  trailing?: ReactNode;
  details?: ReactNode;
}) {
  const Dot = dashed ? CircleDashed : Circle;
  const head = (
    <span className={twMerge('min-w-0 flex-1', active ? 'font-semibold wrap-anywhere whitespace-normal' : 'truncate')}>
      {label}
    </span>
  );
  // The dot and both rail segments share one anchor and transform, so no browser rounds them apart.
  const anchor = 'pointer-events-none absolute left-3.5 -translate-x-1/2';
  const segment = `${anchor} w-px bg-border`;
  return (
    <li
      className={twMerge(
        'group relative',
        active ? 'bg-primary/12 shadow-[inset_2px_0_0_var(--accent)]' : 'hover:bg-accent',
      )}
      data-dashed={dashed || undefined}
      aria-current={active ? 'true' : undefined}
      onPointerEnter={onHover && ((e) => onHover(e.currentTarget))}
      onPointerLeave={onLeave}
    >
      {rail && (
        <>
          {/* Down to this dot's top edge, from the one above; a dashed dot's segment reaches it instead. */}
          <span
            aria-hidden
            className={twMerge(segment, 'top-0 h-[0.5625rem] group-first:hidden [[data-dashed]+li>&]:hidden')}
          />
          {/* From this dot's bottom edge down to the next one; dashed, it runs on to the next dot over that row. */}
          <span
            aria-hidden
            className={twMerge(
              segment,
              'top-[1.0625rem] bottom-0 group-last:hidden',
              dashed &&
                '-bottom-[0.5625rem] z-1 bg-transparent bg-[linear-gradient(var(--color-border)_50%,transparent_50%)] bg-size-[1px_4px]',
            )}
          />
        </>
      )}
      <Dot
        aria-hidden
        size="0.5rem"
        // About 1.5px in the 24-unit box; overflow keeps its outer edge, which passes the box, round.
        strokeWidth={5}
        // Round caps would close the dashes' gaps at this size.
        strokeLinecap={dashed ? 'butt' : undefined}
        className={twMerge(
          // The first line's middle.
          anchor,
          'top-[0.5625rem] overflow-visible',
          active
            ? dashed
              ? 'fill-muted text-primary'
              : 'fill-primary text-primary'
            : 'fill-muted text-muted-foreground group-hover:fill-accent',
        )}
      />
      <div className="flex items-start pr-1.5">
        {onPick ? (
          <Button
            variant="ghost"
            className={twMerge(
              // The list marks the shown bead itself; a ring around the clicked row would only repeat it.
              'h-auto min-w-0 flex-1 items-start justify-start rounded-none border-0 py-1 pr-1 pl-6.5 text-left leading-[1.125rem] outline-none hover:bg-transparent sm:h-auto',
              !active && 'focus-visible:bg-accent',
            )}
            aria-describedby={described}
            onClick={onPick}
            size="sm"
          >
            {head}
          </Button>
        ) : (
          <div className="flex min-w-0 flex-1 items-start py-1 pr-1 pl-6.5 leading-[1.125rem]">{head}</div>
        )}
        {trailing}
      </div>
      {active && details && <div className="pr-2.5 pb-1.5 pl-6.5">{details}</div>}
    </li>
  );
}

function CommitRow({
  commit,
  ...row
}: { commit: RangeCommit } & Pick<
  ComponentProps<typeof BeadRow>,
  'rail' | 'active' | 'described' | 'onPick' | 'onHover' | 'onLeave'
>) {
  return (
    <BeadRow
      {...row}
      label={subjectOf(commit)}
      trailing={<CopyHash commit={commit} />}
      details={<CommitDetails commit={commit} />}
    />
  );
}
