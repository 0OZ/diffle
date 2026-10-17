#!/bin/sh
# Tests for diffle-server. ssh, fzf, curl, nc and open are stubs that record how they were called,
# so the whole run (listing, choosing, tunnelling, opening the page) is judged without a network.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/diffle-server
failures=0

fail() { printf 'FAIL %s\n' "$1"; failures=$((failures + 1)); }
check() { # name expected actual
  if [ "$2" = "$3" ]; then printf 'ok   %s\n' "$1"; else fail "$1"; printf '  expected: %s\n  actual:   %s\n' "$2" "$3"; fi
}

new_sandbox() {
  box=$(mktemp -d)
  mkdir "$box/bin"
  : > "$box/ssh.log"
  : > "$box/open.log"
  cat > "$box/bin/ssh" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$BOX/ssh.log"
case "$*" in
  *"sh -s"*) cat > /dev/null; [ -n "${LISTING_EMPTY-}" ] || printf '%s\n' /srv/alpha /srv/beta ;;
  *) # a server that runs until the page has been opened
     i=0; while [ ! -s "$BOX/open.log" ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
     exit "${SSH_STATUS:-0}" ;;
esac
STUB
  cat > "$box/bin/fzf" <<'STUB'
#!/bin/sh
tee "$BOX/fzf.in" | { [ -n "${FZF_CANCEL-}" ] && cat > /dev/null || head -n 1; }
STUB
  printf '#!/bin/sh\nexit 0\n' > "$box/bin/curl"
  printf '#!/bin/sh\nexit 1\n' > "$box/bin/nc"
  printf '#!/bin/sh\nprintf "%%s\\n" "$1" >> "$BOX/open.log"\n' > "$box/bin/open"
  chmod +x "$box"/bin/*
}

# Runs the script in the sandbox with a fixed port and remote binary; stdout+stderr to $box/out.
run() {
  BOX=$box PATH="$box/bin:$PATH" DIFFLE_PORT=34567 DIFFLE_REMOTE_PORT=45678 DIFFLE_REMOTE_BIN=/opt/diffle DIFFLE_OPEN=open \
    sh "$script" "$@" > "$box/out" 2>&1
  status=$?
}
ssh_calls() { cat "$box/ssh.log"; }
wait_for_page() { i=0; while [ ! -s "$box/open.log" ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done; cat "$box/open.log"; }

export BOX
new_sandbox
run hopper.sh /srv/alpha
check 'a given directory is tunnelled from the local port to the one the server listens on' \
  '-t -o ExitOnForwardFailure=yes -L 34567:127.0.0.1:45678 hopper.sh /opt/diffle -C /srv/alpha -p 45678 --no-open' "$(ssh_calls)"
check 'the page opens once the server answers' 'http://127.0.0.1:34567' "$(wait_for_page)"
check 'success is success' 0 "$status"

new_sandbox
i=0; clashes=0
while [ $i -lt 10 ]; do
  : > "$box/ssh.log"; : > "$box/open.log"
  BOX=$box PATH="$box/bin:$PATH" DIFFLE_PORT=34567 DIFFLE_REMOTE_BIN=/opt/diffle DIFFLE_OPEN=open \
    sh "$script" hopper.sh /srv/alpha > "$box/out" 2>&1
  remote=$(sed -n 's/.*-L 34567:127.0.0.1:\([0-9]*\) .*/\1/p' "$box/ssh.log")
  { [ -n "$remote" ] && [ "$remote" != 34567 ] && [ "$remote" -ge 20000 ] && [ "$remote" -lt 60000 ]; } || clashes=$((clashes + 1))
  i=$((i + 1))
done
check 'by default the server listens on another port than the tunnel (one machine can be both ends)' 0 "$clashes"

new_sandbox
run hopper.sh
check 'without a directory the remote repositories are listed, then served' \
  '-o BatchMode=no hopper.sh sh -s -- $HOME' "$(ssh_calls | sed -n 1p | sed 's/^/-o BatchMode=no /')"
check 'fzf is offered every repository' '/srv/alpha
/srv/beta' "$(cat "$box/fzf.in")"
check 'the first choice is served' \
  '-t -o ExitOnForwardFailure=yes -L 34567:127.0.0.1:45678 hopper.sh /opt/diffle -C /srv/alpha -p 45678 --no-open' "$(ssh_calls | sed -n 2p)"

new_sandbox
run hopper.sh /srv/alpha -- main...HEAD --no-watch
check 'what follows -- goes to the remote diffle' \
  '-t -o ExitOnForwardFailure=yes -L 34567:127.0.0.1:45678 hopper.sh /opt/diffle -C /srv/alpha -p 45678 --no-open main...HEAD --no-watch' "$(ssh_calls)"

new_sandbox
DIFFLE_SSH_OPTS='-o BatchMode=yes -p 2222' run hopper.sh /srv/alpha
check 'ssh options come first' '-o BatchMode=yes -p 2222 -t' "$(ssh_calls | cut -d' ' -f1-5)"

new_sandbox
run hopper.sh '/srv/a b'
check 'a path the remote shell would split is refused' 2 "$status"
check 'and ssh never runs' '' "$(ssh_calls)"

new_sandbox
run hopper.sh '/srv/a;rm'
check 'shell syntax in a path is refused' 2 "$status"

new_sandbox
run
check 'no host is a usage error' 2 "$status"
check 'the usage names the command' 1 "$(grep -c 'usage: diffle-server' "$box/out")"
run -h
check '-h prints the usage and succeeds' 0 "$status"

new_sandbox
DIFFLE_FZF=no-such-fzf run hopper.sh
check 'no fzf and no directory is an error that says what to do' 2 "$status"
check 'it names fzf' 1 "$(grep -c fzf "$box/out")"
check 'and asks nothing of the remote' '' "$(ssh_calls)"

new_sandbox
FZF_CANCEL=1 run hopper.sh
check 'leaving fzf without a choice ends quietly with a failure' 1 "$status"
check 'and starts no server' 1 "$(ssh_calls | wc -l | tr -d ' ')"

new_sandbox
LISTING_EMPTY=1 run hopper.sh
check 'a remote without repositories says so' 1 "$status"
check 'naming the host' 1 "$(grep -c hopper.sh "$box/out")"

new_sandbox
SSH_STATUS=7 run hopper.sh /srv/alpha
check 'the exit status of the remote server is the exit status' 7 "$status"

new_sandbox
run init sh
check 'init sh prints a function that routes `diffle server`' 1 "$(grep -c 'diffle-server "\$@"' "$box/out")"
run init fish
check 'init fish prints one too' 1 "$(grep -c 'diffle-server' "$box/out")"
run init csh
check 'an unknown shell is refused' 2 "$status"

[ "$failures" -eq 0 ] && echo 'all passed' || { echo "$failures failed"; exit 1; }
