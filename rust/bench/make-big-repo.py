#!/usr/bin/env python3
"""Generate the 'big' benchmark repository: 3,000 tracked Rust files, 1,500 of them edited in
the working tree plus 100 new untracked ones, so a working-tree review has 1,600 changed files.

usage: make-big-repo.py [directory]      (default: data/big beside this script)
The content is seeded, so every run produces the same bytes.
"""
import os, random, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MODULES, FILES_PER_MODULE, LINES = 300, 10, 200
NEW_FILES, NEW_LINES = 100, 50
EDITS_PER_FILE, EDIT_STRIDE = 5, 40
WORDS = ('alpha beta gamma delta epsilon zeta theta iota kappa lambda sigma omega '
         'merge branch commit parse render token buffer stream').split()
WORDS_PER_LINE = 8
GIT = ['git', '-c', 'user.name=bench', '-c', 'user.email=bench@example.invalid', '-c', 'commit.gpgsign=false']
FIXED_DATE = '2026-01-01T00:00:00+0000'


def body(rng, lines):
    return [f"    {' '.join(rng.choice(WORDS) for _ in range(WORDS_PER_LINE))}  // {i}" for i in range(lines)]


def write(path, lines):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as f:
        f.write('\n'.join(lines) + '\n')


def main():
    root = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'data', 'big'))
    if os.path.exists(root):
        sys.exit(f'make-big-repo.py: {root} exists; remove it first')
    rng = random.Random(4966)
    paths = {}
    for m in range(MODULES):
        for f in range(FILES_PER_MODULE):
            paths[os.path.join(root, 'src', f'mod{m:03d}', f'file{f}.rs')] = body(rng, LINES)
    for path, lines in paths.items():
        write(path, lines)
    env = dict(os.environ, GIT_AUTHOR_DATE=FIXED_DATE, GIT_COMMITTER_DATE=FIXED_DATE)
    subprocess.run(['git', 'init', '-q', '-b', 'main', root], check=True)
    subprocess.run(['git', '-C', root, 'add', '-A'], check=True)
    subprocess.run(GIT + ['-C', root, 'commit', '-q', '-m', f'initial: {len(paths)} files'], check=True, env=env)
    for path, lines in paths.items():
        if int(path[-4]) % 2 == 0:  # file0, file2, ... file8
            edited = list(lines)
            for k in range(EDITS_PER_FILE):
                at = 5 + EDIT_STRIDE * k + int(path[-4])
                edited[at] = f'    changed line {at}'
            write(path, edited)
    for n in range(NEW_FILES):
        write(os.path.join(root, 'src', f'new{n:03d}.rs'), body(rng, NEW_LINES))
    print(f'{root}: {len(paths)} tracked files, {len(paths) // 2} edited, {NEW_FILES} new')


main()
