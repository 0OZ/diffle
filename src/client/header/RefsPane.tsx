import { ArrowLeftRight } from 'lucide-react';
import type { Dispatch, RefObject, SetStateAction } from 'react';
import { Segment, SegmentedControl } from '@/client/components/SegmentedControl.js';
import type { RefsResponse } from '@/shared/protocol.js';
import { Button } from '@/client/ui/button.js';
import { RefInput } from './RefInput.js';

type Dots = '..' | '...';

/** The two-refs pane of the compare menu: base and target, how they relate, and a swap. */
export function RefsPane({
  a,
  b,
  dots,
  refs,
  onA,
  onB,
  onDots,
  onSwap,
  autoFocus,
  targetRef,
  toggleRef,
}: {
  a: string;
  b: string;
  dots: Dots;
  refs: RefsResponse | null;
  onA: (value: string) => void;
  onB: (value: string) => void;
  onDots: Dispatch<SetStateAction<Dots>>;
  onSwap: () => void;
  autoFocus: boolean;
  targetRef: RefObject<HTMLInputElement | null>;
  toggleRef: RefObject<HTMLDivElement | null>;
}) {
  return (
    <>
      <div className="flex items-center gap-1.5 pt-1.5 font-mono text-[0.8125rem] leading-[1.5]">
        <RefInput
          label="Base ref"
          value={a}
          onChange={onA}
          refs={refs}
          autoFocus={autoFocus}
          onAccept={() => targetRef.current?.focus()}
        />
        <span>{dots}</span>
        <RefInput
          label="Target ref"
          value={b}
          onChange={onB}
          refs={refs}
          inputRef={targetRef}
          onAccept={() => toggleRef.current?.focus()}
        />
      </div>
      <div className="flex items-center gap-2">
        <SegmentedControl
          className="self-start focus-visible:outline-2 focus-visible:outline-offset-[2px] focus-visible:outline-primary focus-visible:outline-solid"
          ref={toggleRef}
          value={dots}
          onValueChange={onDots}
          aria-label={`Comparison: ${dots === '..' ? 'Direct' : 'Merge base'}. Space to toggle.`}
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              e.stopPropagation();
              if (!e.repeat) e.currentTarget.closest('form')?.requestSubmit();
              return;
            }
            if (e.key !== ' ') return;
            e.preventDefault();
            e.stopPropagation();
            if (!e.repeat) onDots((current) => (current === '..' ? '...' : '..'));
          }}
        >
          <Segment value="..." tabIndex={-1}>
            Merge base
          </Segment>
          <Segment value=".." tabIndex={-1}>
            Direct
          </Segment>
        </SegmentedControl>
        <Button
          variant="ghost"
          size="icon-xs"
          id="swap-refs"
          className="text-muted-foreground"
          type="button"
          tabIndex={-1}
          aria-label="Swap refs"
          aria-keyshortcuts="x"
          title="Swap refs (x)"
          onClick={onSwap}
        >
          <ArrowLeftRight size="0.75rem" />
        </Button>
      </div>
      <p className="m-0 p-0 text-[0.75rem] whitespace-normal text-muted-foreground">
        {dots === '..' ? 'Compare these two revisions.' : 'Compare changes since their common ancestor.'}
      </p>
    </>
  );
}
