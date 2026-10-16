import { Button } from '@/client/ui/button.js';
import { Input } from '@/client/ui/input.js';
import { Toggle } from '@/client/ui/toggle.js';
import { FileDiff, FileSearch, Link2, Search, WholeWord, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useStore } from '@/client/store.js';

/** The option toggles are compact: a mono glyph that lights up in the accent while on. */
const TOGGLE = 'font-mono text-[0.6875rem] text-muted-foreground data-pressed:bg-primary/12 data-pressed:text-primary';

/** Content search (/ in the current file, g/ across changed files), the references of a symbol (gA), or a word's occurrences (* / #). Enter runs a search; n / N step through matches. */
export function SearchBar({ path }: { path?: string }) {
  const visible = useStore((s) => {
    const local = s.search.kind === 'text' && s.search.scope === 'file';
    return s.search.open && (local ? s.search.path === path : path === undefined);
  });
  return visible ? <SearchForm /> : null;
}

function SearchForm() {
  const search = useStore((s) => s.search);
  const runSearch = useStore((s) => s.runSearch);
  const closeSearch = useStore((s) => s.closeSearch);
  const setSearchOptions = useStore((s) => s.setSearchOptions);
  const setSearchInput = useStore((s) => s.setSearchInput);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Virtualization can remount this input during a wheel gesture; focus must not move the viewport.
    if (!search.editing) return;
    ref.current?.focus({ preventScroll: true });
    if (search.selected) ref.current?.select();
  }, [search.editing, search.focusNonce, search.selected]);

  const n = search.matches.length;
  if (search.kind !== 'text') {
    return (
      <div className="flex items-center gap-2 border-b border-b-border bg-muted px-2.5 py-1.5">
        {search.kind === 'references' ? <Link2 size="0.875rem" /> : <WholeWord size="0.875rem" />}
        <span className="flex-1 font-mono text-[0.75rem] leading-[normal] text-muted-foreground [&_code]:text-foreground">
          {search.kind === 'references'
            ? 'references to'
            : search.direction === 1
              ? 'occurrences of'
              : 'occurrences (upwards) of'}{' '}
          <code>{search.query}</code>
        </span>
        <span className="min-w-17.5 text-right font-mono text-[0.75rem] leading-[normal] text-muted-foreground">
          {search.loading ? 'searching…' : n ? `${search.index + 1} / ${n}${search.truncated ? '+' : ''}` : 'none'}
        </span>
        <Button type="button" variant="ghost" size="icon-sm" onClick={closeSearch} title="Close (Esc)">
          <X size="0.875rem" />
        </Button>
      </div>
    );
  }
  const fullFile = search.content[search.scope] === 'full';
  const ScopeIcon = fullFile ? FileSearch : FileDiff;
  return (
    <form
      className="flex items-center gap-2 border-b border-b-border bg-muted px-2.5 py-1.5"
      onClick={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        void runSearch(search.input).then(() => {
          ref.current?.blur();
          document.querySelector<HTMLElement>('main[tabindex]')?.focus({ preventScroll: true });
        });
      }}
    >
      <Search size="0.875rem" />
      <Input
        data-content-search=""
        size="sm"
        className="min-w-0 flex-1 font-mono text-[0.75rem]"
        ref={ref}
        value={search.input}
        onFocus={() => {
          if (!search.editing) setSearchInput(search.input);
        }}
        onChange={(e) => setSearchInput(e.target.value)}
        placeholder={
          search.scope === 'file' ? 'Search this file… (Enter, then n / N)' : 'Search all files… (Enter, then n / N)'
        }
        spellCheck={false}
      />
      <Toggle
        size="sm"
        className={TOGGLE}
        pressed={!search.ignoreCase}
        onPressedChange={(pressed) => setSearchOptions({ ignoreCase: !pressed })}
        title={search.ignoreCase ? 'Ignoring case; click to match case' : 'Matching case; click to ignore case'}
      >
        Aa
      </Toggle>
      <Toggle
        size="sm"
        className={TOGGLE}
        pressed={search.regex}
        onPressedChange={(pressed) => setSearchOptions({ regex: pressed })}
        title={search.regex ? 'Extended regex; click for plain text' : 'Plain text; click for extended regex'}
      >
        .*
      </Toggle>
      <Toggle
        size="sm"
        className={TOGGLE}
        aria-label="Search full file"
        pressed={fullFile}
        onPressedChange={(pressed) => setSearchOptions({ content: pressed ? 'full' : 'diff' })}
        title={
          fullFile
            ? 'Searching full file; click to search diff + context'
            : 'Searching diff + context; click to search full file'
        }
      >
        <ScopeIcon size="0.75rem" />
      </Toggle>
      <span className="min-w-17.5 text-right font-mono text-[0.75rem] leading-[normal] text-muted-foreground">
        {search.loading
          ? 'searching…'
          : n === 0 && search.query
            ? search.scope === 'file'
              ? 'none in file'
              : fullFile
                ? 'no matches'
                : 'none in diff'
            : n
              ? `${search.index + 1} / ${n}${search.truncated ? '+' : ''}`
              : ''}
      </span>
      <Button type="button" variant="ghost" size="icon-sm" onClick={closeSearch} title="Close (Esc)">
        <X size="0.875rem" />
      </Button>
    </form>
  );
}
