#!/usr/bin/env bash
# Run in a PTY (e.g. script -q -e -c 'bash tools/benchmarks/console-terminal-smoke.sh' LOG).
set -u
if [[ ! -t 0 || ! -t 1 ]]; then
    printf '%s\n' 'This terminal restoration check requires a PTY.' >&2
    exit 2
fi
before=$(stty -g) || exit 2
status=0
"${TERRA_SERVER_BIN:-target/debug/terra-server}" --no-save --bind 127.0.0.1:0 || status=$?
after=$(stty -g) || exit 2
if [[ "$before" != "$after" ]]; then
    printf '%s\n' 'TERMINAL_RESTORE_FAILED' >&2
    stty "$before"
    exit 1
fi
printf '%s\n' 'TERMINAL_RESTORED'
exit "$status"
