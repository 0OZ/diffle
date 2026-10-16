import { twMerge } from 'tailwind-merge';
import { Button } from '@/client/ui/button.js';
import { Input } from '@/client/ui/input.js';
import { Label } from '@/client/ui/label.js';
import {
  ChevronDown,
  ChevronRight,
  GitCommitHorizontal,
  GitCommitVertical,
  GitCompareArrows,
  GitPullRequest,
  History,
  PencilRuler,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { comparisonLabel, type ModeRequest, type RefsResponse } from '@/shared/protocol.js';
import { api } from '@/client/api.js';
import { focusReview } from '@/client/keyboard/useKeymap.js';
import { lastCommitsRequest } from '@/client/model.js';
import { useStore } from '@/client/store.js';
import { RefInput } from './RefInput.js';
import { RefsPane } from './RefsPane.js';
import { CommitOffsetInput, parseOffset } from './CommitOffsetInput.js';
import { CommitPreview } from './CommitPreview.js';
import { Kbd } from '@/client/ui/kbd.js';

/** Comparison modes with configuration in an adjacent pane. */
export function ModePicker() {
  const snapshot = useStore((s) => s.snapshot);
  const switchMode = useStore((s) => s.switchMode);
  const open = useStore((s) => s.modeMenuOpen);
  const setOpen = useStore((s) => s.setModeMenuOpen);
  const pane = useStore((s) => s.modePane);
  const pick = useStore((s) => s.pickModeEntry);
  const highlighted = useStore((s) => s.modeEntry);
  const highlight = useStore((s) => s.highlightModeEntry);
  const [refs, setRefs] = useState<RefsResponse | null>(null);
  const [a, setA] = useState('');
  const [b, setB] = useState('HEAD');
  const [dots, setDots] = useState<'..' | '...'>('...');
  // A range-diff's two ranges, `old..new` each; the bases follow the default branch until typed.
  const [ranges, setRanges] = useState({ oldBase: '', oldTip: '', newBase: '', newTip: 'HEAD' });
  const setRange = (key: keyof typeof ranges) => (value: string) =>
    setRanges((current) => ({ ...current, [key]: value }));
  const [oldOffsetText, setOldOffsetText] = useState('1');
  const [newOffsetText, setNewOffsetText] = useState('0');
  const commit = useStore((s) => s.modeCommit);
  const setCommit = useStore((s) => s.setModeCommit);
  const [pr, setPr] = useState('');
  const [prPending, setPrPending] = useState(false);
  const [prError, setPrError] = useState<string | null>(null);
  const prInFlight = useRef(false);
  const prView = useRef(0);
  const prInput = useRef<HTMLInputElement>(null);
  const pointerPick = useRef(false);

  useEffect(() => {
    pointerPick.current = false;
  }, [open, pane]);

  useEffect(() => {
    const view = ++prView.current;
    return () => {
      prView.current = view + 1;
    };
  }, [open, pane]);

  const wrap = useRef<HTMLDivElement>(null);
  const targetRef = useRef<HTMLInputElement>(null);
  /** The range-diff pane's tip inputs at odd indices, so Enter in one input moves to the next. */
  const rangeInputs = useRef<(HTMLInputElement | null)[]>([]);
  const comparisonToggle = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    let current = true;
    void api
      .refs()
      .then((r) => {
        if (!current) return;
        setRefs(r);
        const base = r.defaultBranch || r.branches[0] || 'HEAD';
        setA((cur) => cur || base);
        setRanges((cur) => ({ ...cur, oldBase: cur.oldBase || base, newBase: cur.newBase || base }));
      })
      .catch((e) => {
        if (current) useStore.getState().report('Loading refs', e);
      });
    const onDoc = (e: MouseEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    return () => {
      current = false;
      document.removeEventListener('mousedown', onDoc);
    };
  }, [open, setOpen]);

  const choose = (req: ModeRequest) => {
    setOpen(false);
    // Hand focus back to the diff instead of the trigger: a programmatic focus there shows the
    // native ring even though the user never navigated to the button.
    focusReview();
    void switchMode(req);
  };
  const chooseCommit = (revision: string) => choose({ kind: 'revspec', args: [`${revision}^!`] });
  const openPr = async () => {
    if (prInFlight.current) return;
    prInFlight.current = true;
    setPrPending(true);
    setPrError(null);
    const view = prView.current;
    try {
      const result = await switchMode(pr.trim() ? { kind: 'pr', pr: pr.trim() } : { kind: 'pr' });
      if (view !== prView.current) return;
      if (result === 'applied') {
        setOpen(false);
        focusReview();
      } else if (typeof result === 'object') {
        setPrError(result.error);
        prInput.current?.focus();
      }
    } finally {
      prInFlight.current = false;
      setPrPending(false);
    }
  };
  const oldOffset = parseOffset(oldOffsetText);
  const newOffset = parseOffset(newOffsetText);
  const validOffsets = oldOffset !== null && newOffset !== null;
  const swapRefs = () => {
    setA(b);
    setB(a);
  };
  const rangeArgs = Object.values(ranges).every((v) => v.trim())
    ? [`${ranges.oldBase.trim()}..${ranges.oldTip.trim()}`, `${ranges.newBase.trim()}..${ranges.newTip.trim()}`]
    : null;
  const entries = [
    { label: 'Working', icon: PencilRuler, pane: null },
    { label: 'Two refs…', icon: GitCommitHorizontal, pane: 'refs' },
    { label: 'Last commits', icon: History, pane: 'commits' },
    { label: 'PR', icon: GitPullRequest, pane: 'pr' },
    { label: 'Commit…', icon: GitCommitVertical, pane: 'commit' },
    { label: 'Range diff…', icon: GitCompareArrows, pane: 'range-diff' },
  ] as const;

  return (
    <div className="relative min-w-28" ref={wrap}>
      <Button
        variant="outline"
        onClick={() => setOpen(!open)}
        title="Change what is compared (m)"
        aria-expanded={open}
        aria-controls="mode-picker"
        className="max-w-full font-mono"
        size="sm"
      >
        <span className="truncate">{snapshot ? comparisonLabel(snapshot.mode) : '…'}</span>{' '}
        <ChevronDown size="0.875rem" />
      </Button>
      {open && (
        <div
          className="absolute top-[calc(100%_+_0.375rem)] left-0 z-20 flex w-max max-w-[calc(100vw_-_2rem)] min-w-60 rounded-[0.625rem] border border-border bg-background p-1.5 shadow-[0_0.625rem_1.875rem_rgba(0,_0,_0,_0.18)] max-[640px]:fixed max-[640px]:top-12 max-[640px]:left-2 max-[640px]:max-w-[calc(100vw_-_1rem)]"
          id="mode-picker"
          role="dialog"
          aria-label="Compare"
        >
          <div className="w-60 shrink-0 max-[640px]:w-44">
            <div className="px-2.5 pt-1 pb-1.5 text-[0.6875rem] font-semibold text-muted-foreground">Compare</div>
            {/* The accent marks an expanded configuration pane; Working has none and acts on click.
                Pointer, focus and j / k share one highlight, which Enter picks. */}
            {entries.map((entry, i) => (
              <Button
                variant="ghost"
                key={entry.label}
                className={twMerge(
                  `grid h-auto min-h-8.5 w-full grid-cols-[1.125rem_1fr_auto_1.5rem] items-center gap-2.5 rounded-[0.4375rem] px-2.5 py-1.75 text-left font-sans text-[0.8125rem] leading-[1.3] sm:h-auto [&_kbd]:ml-1 [&_kbd]:justify-self-end [&>svg]:text-muted-foreground ${highlighted === i + 1 ? 'bg-accent' : 'hover:bg-transparent'} ${entry.pane && pane === entry.pane ? '[&>span:first-of-type]:font-semibold [&>svg]:text-primary' : ''}`,
                )}
                aria-expanded={entry.pane ? pane === entry.pane : undefined}
                aria-controls={entry.pane ? 'mode-config' : undefined}
                data-mode-entry={i + 1}
                data-highlighted={highlighted === i + 1 || undefined}
                onPointerMove={() => highlight(i + 1)}
                onFocus={() => highlight(i + 1)}
                onClick={(e) => {
                  pointerPick.current = e.detail > 0;
                  pick(i + 1);
                }}
                size="sm"
              >
                <entry.icon size="0.875rem" />
                <span className="whitespace-nowrap">{entry.label}</span>
                <Kbd>{i + 1}</Kbd>
                {entry.pane ? <ChevronRight size="0.875rem" /> : <span />}
              </Button>
            ))}
          </div>
          {pane && (
            <form
              className="ml-1.5 flex w-76 min-w-0 flex-col gap-3 border-l border-l-border px-3 pt-0 pb-2"
              id="mode-config"
              aria-label={entries.find((e) => e.pane === pane)?.label}
              onKeyDown={(e) => {
                if (pane !== 'refs' || e.key !== 'x' || e.ctrlKey || e.metaKey || e.altKey) return;
                if ((e.target as HTMLElement).closest('input, textarea, select, [contenteditable]')) return;
                e.preventDefault();
                e.stopPropagation();
                if (!e.repeat) swapRefs();
              }}
              onSubmit={(e) => {
                e.preventDefault();
                if (pane === 'refs' && a.trim() && b.trim())
                  choose({ kind: 'revspec', args: [`${a.trim()}${dots}${b.trim()}`] });
                if (pane === 'commits' && validOffsets) {
                  choose(lastCommitsRequest(oldOffset, newOffset));
                }
                if (pane === 'pr') void openPr();
                if (pane === 'commit' && commit.trim()) chooseCommit(commit.trim());
                if (pane === 'range-diff' && rangeArgs) choose({ kind: 'range-diff', args: rangeArgs });
              }}
            >
              {pane === 'pr' && (
                <div className="pt-1 pb-1.5 text-[0.6875rem] font-semibold text-muted-foreground">Pull request</div>
              )}
              {pane === 'refs' && (
                <RefsPane
                  a={a}
                  b={b}
                  dots={dots}
                  refs={refs}
                  onA={setA}
                  onB={setB}
                  onDots={setDots}
                  onSwap={swapRefs}
                  autoFocus={!pointerPick.current}
                  targetRef={targetRef}
                  toggleRef={comparisonToggle}
                />
              )}
              {pane === 'commits' && (
                <div className="flex flex-col gap-2 pt-1.5">
                  <div className="flex items-baseline font-mono text-[0.8125rem] leading-[1.5]">
                    <span>HEAD~</span>
                    <CommitOffsetInput
                      label="Base offset"
                      value={oldOffsetText}
                      onChange={setOldOffsetText}
                      autoFocus
                    />
                    <span>..HEAD~</span>
                    <CommitOffsetInput label="Target offset" value={newOffsetText} onChange={setNewOffsetText} />
                  </div>
                  <CommitPreview oldOffset={oldOffset} newOffset={newOffset} version={snapshot?.version ?? 0} />
                </div>
              )}
              {pane === 'commit' && (
                <>
                  <div className="flex items-center gap-1.5 pt-1.5 font-mono text-[0.8125rem] leading-[1.5]">
                    <RefInput
                      label="Commit"
                      value={commit}
                      onChange={setCommit}
                      refs={refs}
                      worktree={false}
                      autoFocus={!pointerPick.current}
                      onAccept={chooseCommit}
                    />
                    <span>^!</span>
                  </div>
                  <p className="m-0 p-0 text-[0.75rem] whitespace-normal text-muted-foreground">
                    Show this commit against its first parent.
                  </p>
                </>
              )}
              {pane === 'range-diff' && (
                <>
                  {(['old', 'new'] as const).map((side, i) => (
                    <div key={side} className={`flex flex-col gap-1 ${i === 0 ? 'pt-1.5' : ''}`}>
                      <span className="text-[0.75rem] text-muted-foreground">
                        {side === 'old' ? 'Old range' : 'New range'}
                      </span>
                      <div className="flex items-center gap-1.5 font-mono text-[0.8125rem] leading-[1.5]">
                        <RefInput
                          label={`${side === 'old' ? 'Old' : 'New'} base`}
                          value={ranges[`${side}Base`]}
                          onChange={setRange(`${side}Base`)}
                          refs={refs}
                          worktree={false}
                          autoFocus={i === 0 && !pointerPick.current}
                          onAccept={() => rangeInputs.current[i * 2 + 1]?.focus()}
                        />
                        <span>..</span>
                        <RefInput
                          label={`${side === 'old' ? 'Old' : 'New'} tip`}
                          value={ranges[`${side}Tip`]}
                          onChange={setRange(`${side}Tip`)}
                          refs={refs}
                          worktree={false}
                          inputRef={(el) => {
                            rangeInputs.current[i * 2 + 1] = el;
                          }}
                          onAccept={() =>
                            i === 0
                              ? rangeInputs.current[3]?.focus()
                              : rangeArgs && choose({ kind: 'range-diff', args: rangeArgs })
                          }
                        />
                      </div>
                    </div>
                  ))}
                  <p className="m-0 p-0 text-[0.75rem] whitespace-normal text-muted-foreground">
                    How the old range’s commits became the new range’s, like git range-diff.
                  </p>
                </>
              )}
              {pane === 'pr' && (
                <>
                  <Label className="min-w-0 flex-col items-stretch gap-1.5 text-[0.75rem] font-normal text-muted-foreground">
                    PR number or URL
                    <Input
                      size="sm"
                      className="font-mono text-[0.75rem]"
                      ref={prInput}
                      autoFocus
                      autoComplete="off"
                      spellCheck={false}
                      readOnly={prPending}
                      aria-describedby={prError ? 'pr-hint pr-error' : 'pr-hint'}
                      aria-invalid={prError ? true : undefined}
                      value={pr}
                      onChange={(e) => {
                        setPr(e.target.value);
                        setPrError(null);
                      }}
                      placeholder="Current branch’s PR"
                    />
                  </Label>
                  <p className="m-0 p-0 text-[0.75rem] whitespace-normal text-muted-foreground" id="pr-hint">
                    Enter a PR number or URL, or leave blank for this branch.
                  </p>
                  {prError && (
                    <p className="m-0 text-[0.75rem] wrap-anywhere text-del" id="pr-error" role="alert">
                      {prError}
                    </p>
                  )}
                </>
              )}
              <Button
                className="mt-auto self-end"

                type="submit"
                aria-live={pane === 'pr' ? 'polite' : undefined}
                aria-busy={pane === 'pr' && prPending}
                disabled={
                  pane === 'pr'
                    ? prPending
                    : pane === 'refs'
                      ? !a.trim() || !b.trim()
                      : pane === 'commit'
                        ? !commit.trim()
                        : pane === 'range-diff'
                          ? !rangeArgs
                          : !validOffsets
                }
                size="sm"
              >
                {pane === 'pr'
                  ? prPending
                    ? 'Loading PR…'
                    : 'Show PR'
                  : pane === 'commit'
                    ? 'Show commit'
                    : 'Compare'}
              </Button>
            </form>
          )}
        </div>
      )}
    </div>
  );
}
