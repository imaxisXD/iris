#!/bin/zsh
# Updates Iris on this Mac: pulls the latest code, rebuilds, installs to /Applications and relaunches.
# Run it directly, or use "Check for Updates…" in Iris's menu bar menu.
#   scripts/update.sh          pull from GitHub, then build and install the latest commit
#   scripts/update.sh --local  build and install this checkout's latest commit (no pull)
#   scripts/update.sh --local --commit <sha>   build and install that commit (ship.sh does this)
# It always builds a commit, never the files on disk: uncommitted work (yours, or another
# session's) can't break an install or end up in the app. After committing here, use
# scripts/ship.sh, which pushes and then runs this.
# The menu item's log is IRIS_UPDATE_LOG in scripts/lib.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib.sh
iris_use_standard_path

notify() { osascript -e "display notification \"$1\" with title \"Iris\"" >/dev/null 2>&1 || true; }
# The running Iris stays untouched until the new build is ready, so a failure costs nothing.
# Every failure leaves "Update failed" in the log, which Updater.swift looks for.
trap 'echo "Update failed at line $LINENO." >&2; notify "Update failed. Iris is unchanged. Log: ${IRIS_UPDATE_LOG/#$HOME/~}"' ERR

DEST=$(iris_installed_app)

# The commit comes only as an argument, never from the environment: Iris inherits the
# environment of whatever opened it, and hands it to this script, so a variable left over from
# one ship would make every later update build that same old commit again.
LOCAL=false
PULLED=false
WANT=HEAD
while (( $# )); do
  case $1 in
    --local) LOCAL=true ;;
    # Only this script passes it, to itself, after its pull (below).
    --pulled) LOCAL=true; PULLED=true ;;
    --commit) WANT=${2:?--commit needs a commit}; shift ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done
$PULLED || echo "── $(date '+%Y-%m-%d %H:%M:%S') update"

if ! $LOCAL; then
  if ! git remote get-url origin >/dev/null 2>&1; then
    echo "No GitHub remote set up for this checkout." >&2
    exit 2
  fi
  # A stalled connection fails (and says so in the log) instead of hanging the update.
  git -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=20 pull --ff-only
  # The pull may have brought a newer copy of this script and of lib.sh, but zsh goes on running
  # the text it started with. Start again as the script now on disk, so the steps that build
  # and install are the ones written for the code being built, never an older version's.
  [[ -x scripts/update.sh ]]
  exec scripts/update.sh --pulled
fi

# One update at a time: the menu item and a session could otherwise build over each other.
LOCK=$IRIS_UPDATE_LOCK
mkdir -p "$(dirname "$LOCK")"
# An older lock was left by a run that died; Iris ignores it too.
if [[ -d "$LOCK" && -n "$(find "$LOCK" -maxdepth 0 -mmin +$IRIS_UPDATE_LOCK_STALE_MINUTES)" ]]; then rmdir "$LOCK"; fi
if ! mkdir "$LOCK" 2>/dev/null; then
  # Updater.swift looks for "Another update is running".
  echo "Another update is running (remove $LOCK if it isn't)." >&2
  exit 3
fi
# A first build (Rust, Iris's interface, the brain and MLX, from nothing) can take longer than the
# lock's 15 minutes (measured: the interface alone, 9 minutes here). Keep the lock fresh while
# this runs, so neither Iris nor another update.sh takes it for one that died. It stops with
# this run, or the moment the lock is gone.
UPDATER=$$
( while sleep 60; do kill -0 $UPDATER 2>/dev/null && [[ -d "$LOCK" ]] || exit 0; touch "$LOCK"; done ) </dev/null >/dev/null 2>&1 &
HEARTBEAT=$!
trap 'kill $HEARTBEAT 2>/dev/null; rmdir "$LOCK" 2>/dev/null || true' EXIT

# Build HEAD in its own worktree.
COMMIT=$(git rev-parse "$WANT^{commit}")   # ship.sh names the commit it pushed
TREE="$PWD/.build/install-tree"
iris_worktree_at "$TREE" "$COMMIT"
# The app points "Check for Updates…" at this checkout, and signs like every other build here
# (the worktree has no .signing-identity of its own). Only the build sees these.
IRIS_SOURCE_PATH="$PWD" SIGN_IDENTITY="$(iris_signing_identity "$PWD")" "$TREE/scripts/build-app.sh"

# Quit the running copy and wait for it to exit, so the new one can take over the shortcuts.
iris_quit

mkdir -p "$(dirname "$DEST")"
rm -rf "$DEST"
ditto "$TREE/build/Iris.app" "$DEST"
iris_open "$DEST" || true
echo "Installed $(git rev-parse --short "$COMMIT") to $DEST"
