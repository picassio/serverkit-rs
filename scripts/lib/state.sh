# shellcheck shell=bash
#
# scripts/lib/state.sh — tiny install-state tracker.
#
# ServerKit makes a handful of global, host-wide changes during install
# (firewall ports, an apt lock-wait drop-in, an nginx TLS snippet). For uninstall
# to be reliable and for re-runs to be idempotent, we record exactly what we
# touched in /etc/serverkit/install-state.json and undo only those things on the
# way out — never a rule or file the operator added themselves (Goal G8).
#
# Backed by python3, which is a hard ServerKit dependency (the whole backend is
# Python), so it is present on any installed box. If python3 is somehow missing
# the helpers degrade to warn-and-no-op rather than aborting the caller. The
# same contract holds when python3 IS present but the state file can't be
# used (read-only fs, ENOSPC, unwritable parent, path-is-a-directory):
# mutating ops warn to stderr and exit 0, read ops stay quiet and print
# nothing — recording state is best-effort bookkeeping and must never abort
# a `set -e` caller.
#
# API:
#   state_set    <key> <value>     set a scalar
#   state_get    <key>             print a scalar ("" if absent)
#   state_append <key> <value>     append to an array (deduped)
#   state_list   <key>             print array items, one per line
#   state_unset  <key>             remove a key entirely
#
# Override the file location with SERVERKIT_STATE_FILE (used by tests).

SERVERKIT_STATE_FILE="${SERVERKIT_STATE_FILE:-/etc/serverkit/install-state.json}"

_state_have_py() { command -v python3 >/dev/null 2>&1; }

_state_py() {
    # Args: <action> <key> [value]
    SERVERKIT_STATE_FILE="$SERVERKIT_STATE_FILE" python3 - "$@" <<'PY'
import json, os, sys

path = os.environ["SERVERKIT_STATE_FILE"]
action = sys.argv[1] if len(sys.argv) > 1 else ""
key = sys.argv[2] if len(sys.argv) > 2 else ""
value = sys.argv[3] if len(sys.argv) > 3 else ""

def warn(msg):
    try:
        sys.stderr.write("  [state] %s\n" % msg)
    except OSError:
        pass

# Reads are quiet-empty on any I/O problem: a missing, unreadable or
# corrupt state file means "empty state" — an observation never aborts.
# We remember when an EXISTING file could not be read so a later save()
# can refuse to clobber state it never saw.
load_failed = False
try:
    with open(path) as fh:
        data = json.load(fh)
    if not isinstance(data, dict):
        data = {}
except (FileNotFoundError, ValueError):
    data = {}
except OSError:
    data = {}
    load_failed = True

def save():
    # Mutations are warn-and-no-op on failure (read-only fs, ENOSPC,
    # unwritable parent, path-is-a-directory): the header promises this
    # degrades rather than aborting a `set -e` caller.
    if load_failed:
        warn("cannot read %s — not recording %s" % (path, key))
        return
    try:
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        tmp = path + ".tmp"
        with open(tmp, "w") as fh:
            json.dump(data, fh, indent=2, sort_keys=True)
            fh.write("\n")
        os.replace(tmp, path)
    except OSError as exc:
        warn("cannot write %s (%s) — not recording %s" % (path, exc, key))

try:
    if action == "set":
        data[key] = value
        save()
    elif action == "get":
        v = data.get(key, "")
        if v is not None:
            sys.stdout.write(str(v))
    elif action == "append":
        arr = data.get(key)
        if not isinstance(arr, list):
            arr = []
        if value not in arr:
            arr.append(value)
        data[key] = arr
        save()
    elif action == "list":
        arr = data.get(key)
        if isinstance(arr, list):
            for item in arr:
                sys.stdout.write(str(item) + "\n")
    elif action == "unset":
        data.pop(key, None)
        save()
    else:
        sys.exit(2)
except BrokenPipeError:
    # A downstream consumer closed the pipe early (state_list | head ...).
    # Not an error for an observation; silence the interpreter's exit-time
    # flush and leave with 0 so a pipefail caller is unaffected.
    try:
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
    except OSError:
        pass
PY
}

state_set() {
    _state_have_py || { printf '  [state] python3 unavailable — not recording %s\n' "${1:-}" >&2; return 0; }
    _state_py set "${1:-}" "${2:-}"
}

state_get() {
    _state_have_py || return 0
    _state_py get "${1:-}"
}

state_append() {
    _state_have_py || { printf '  [state] python3 unavailable — not recording %s\n' "${1:-}" >&2; return 0; }
    _state_py append "${1:-}" "${2:-}"
}

state_list() {
    _state_have_py || return 0
    _state_py list "${1:-}"
}

state_unset() {
    _state_have_py || return 0
    _state_py unset "${1:-}"
}
