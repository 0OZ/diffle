import { GitPullRequest, ExternalLink } from 'lucide-react';
import { useStore } from '@/client/store.js';
import { Button } from '@/client/ui/button.js';
import { Popover, PopoverPopup, PopoverTrigger } from '@/client/ui/popover.js';

/** GitHub metadata loads independently of the diff. */
export function GithubMenu() {
  const open = useStore((s) => s.githubMenuOpen);
  const setOpen = useStore((s) => s.setGithubMenuOpen);
  const github = useStore((s) => s.github);
  const metadata = github.data;
  const repository = metadata?.repository;
  const pr = metadata?.pullRequest;
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={<Button variant="ghost" title="GitHub information (o)" aria-label="GitHub information" size="sm" />}
      >
        <GitPullRequest size="1rem" /> <span className="max-lg:hidden">GitHub</span>
      </PopoverTrigger>
      <PopoverPopup
        id="github-menu"
        aria-label="GitHub information"
        align="start"
        sideOffset={6}
        className="w-80 max-w-[85vw] text-[0.8125rem]"
      >
        <div>
          <strong>Repository</strong>
          <p className="mt-1 mb-3 wrap-anywhere">
            {repository ? (
              <a
                className="inline-flex items-center gap-1 text-primary hover:underline"
                href={`https://github.com/${repository}`}
                target="_blank"
                rel="noreferrer"
              >
                {repository}
                <ExternalLink size="0.75rem" />
              </a>
            ) : (
              <span className="text-muted-foreground">
                {github.status === 'loading' ? 'Loading…' : metadata ? 'No GitHub origin' : 'Unavailable'}
              </span>
            )}
          </p>
          <strong>Pull request</strong>
          <div aria-live="polite" className="mt-1">
            {github.status === 'loading' ? (
              <p className="text-muted-foreground">Loading…</p>
            ) : github.status === 'error' ? (
              <p className="text-warn">Lookup failed: {github.error}</p>
            ) : pr ? (
              <>
                <a className="text-primary hover:underline" href={pr.url} target="_blank" rel="noreferrer">
                  #{pr.number} {pr.title}
                </a>
                <p className="mt-1 text-muted-foreground">
                  {pr.state === 'OPEN' ? (pr.isDraft ? 'Draft' : 'Open') : pr.state === 'MERGED' ? 'Merged' : 'Closed'}
                </p>
                {pr.repository !== repository && <p className="text-muted-foreground">{pr.repository}</p>}
                {metadata?.reason && <p className="mt-2 text-muted-foreground">{metadata?.reason}</p>}
              </>
            ) : (
              <p className="text-muted-foreground">{metadata?.reason ?? 'No matching pull request'}</p>
            )}
          </div>
        </div>
      </PopoverPopup>
    </Popover>
  );
}
