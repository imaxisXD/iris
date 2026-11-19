#!/bin/zsh
# Builds a prebuilt Iris for other Macs: dist/Iris-<version>.zip, which runs without Xcode,
# the Metal Toolchain or a checkout. It doesn't publish anything; it prints the command that
# would. Signing is $SIGN_IDENTITY (ad hoc by default; a Developer ID with NOTARY_PROFILE notarizes).
#   scripts/release.sh          the changelog's newest issue (CHANGELOG-RULES.md)
#   scripts/release.sh 0.2.0    the same, checked: it must be the newest issue
#
# Like update.sh it builds a commit, never the files on disk: HEAD (or $IRIS_COMMIT), in a
# clean worktree of its own. Git-ignored eval work (IRIS_EVAL_PATHS in lib.sh) isn't in that
# worktree, so it can't end up in the app.
#
# Signing, in $SIGN_IDENTITY (default: ad hoc, "-"):
#   "-"                              ad hoc. Gatekeeper blocks the first open, and macOS asks
#                                    for Accessibility again after every update.
#   "Iris Local Signing"             the self-signed identity from make-signing-cert.sh, the
#                                    same on every release, so Accessibility should survive
#                                    updates. Gatekeeper still blocks the first open.
#   "Developer ID Application: …"    Apple-issued. With NOTARY_PROFILE set (a keychain profile
#                                    from `xcrun notarytool store-credentials`), the zip is
#                                    notarized and stapled, and opens without a warning.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/lib.sh
iris_use_standard_path
ROOT="$PWD"

fail() { print -u2 "release: $1"; exit 1; }

COMMIT=$(git rev-parse "${IRIS_COMMIT:-HEAD}")
# A release is the changelog's newest issue at that commit, and releasing it freezes it.
NEWEST=$(git show "$COMMIT:Sources/Iris/Resources/Changelog.md" | sed -nE 's/^# v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' | head -n 1)
VERSION="${1:-$NEWEST}"
[[ "$VERSION" =~ '^[0-9]+\.[0-9]+\.[0-9]+$' ]] || fail "give a version like 0.2.0 (got '${VERSION}')."
[[ "$VERSION" == "$NEWEST" ]] || fail "the changelog's newest issue is $NEWEST, not $VERSION. Write the issue first (CHANGELOG-RULES.md)."
git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && fail "v$VERSION is already released. Changes since then go in a new issue."
SHORT=$(git rev-parse --short "$COMMIT")
export SIGN_IDENTITY="${SIGN_IDENTITY:--}"

TREE="$ROOT/.build/release-tree"
iris_worktree_at "$TREE" "$COMMIT"
# Tracked, untracked or ignored: --others without --exclude-standard lists them all.
[[ -z "$(git -C "$TREE" ls-files --cached --others -- "${IRIS_EVAL_PATHS[@]}")" ]] || fail "eval files are in the release tree."

echo "→ Iris $VERSION from $SHORT, signed: $SIGN_IDENTITY"
IRIS_RELEASE=1 IRIS_VERSION="$VERSION" "$TREE/scripts/build-app.sh"
APP="$TREE/build/Iris.app"

echo "→ Checking the app needs nothing from Xcode…"
# Every library it loads must come with macOS or sit inside the bundle.
outside=$(otool -L "$APP/Contents/MacOS/Iris" | tail -n +2 | awk '{print $1}' \
  | grep -vE '^(/System/Library/|/usr/lib/|@rpath/|@executable_path/|@loader_path/)' || true)
[[ -z "$outside" ]] || fail "the app links libraries outside macOS:
$outside"
rpath_libs=$(otool -L "$APP/Contents/MacOS/Iris" | awk '/@rpath\//{print $1}')
for lib in ${(f)rpath_libs}; do
  [[ -e "$APP/Contents/Frameworks/${lib#@rpath/}" ]] || fail "the app needs $lib, which isn't in the bundle."
done
metallib=$(find "$APP/Contents/Resources" -name default.metallib | head -n 1)
[[ -n "$metallib" ]] || fail "MLX's default.metallib isn't in the bundle."
otool -l "$APP/Contents/MacOS/Iris" | grep -q __llvm_prf_cnts \
  && fail "the app was built with code-coverage counters (see build-app.sh)."
codesign --verify --deep --strict "$APP" || fail "the signature doesn't verify."
# No developer paths baked into Info.plist (a release has no checkout to update from).
! /usr/libexec/PlistBuddy -c "Print :IrisSourcePath" "$APP/Contents/Info.plist" >/dev/null 2>&1 \
  || fail "Info.plist still points at a checkout."

DIST="$ROOT/dist"
ZIP="$DIST/Iris-$VERSION.zip"
mkdir -p "$DIST"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"

if [[ "$SIGN_IDENTITY" == "Developer ID Application:"* && -n "${NOTARY_PROFILE:-}" ]]; then
  echo "→ Notarizing (this waits for Apple, usually a few minutes)…"
  xcrun notarytool submit "$ZIP" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$APP"
  rm -f "$ZIP"
  ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
fi

# Nothing in the app names the Mac or the person that built it.
if grep -a -q -e "$HOME" -e "$ROOT" "$APP/Contents/MacOS/Iris"; then
  fail "the app contains this Mac's paths ($HOME). build-app.sh should map them to /iris."
fi
echo "→ What Gatekeeper will say on another Mac:"
spctl --assess --type execute --verbose=2 "$APP" 2>&1 | sed 's/^/    /' || true

SHA=$(shasum -a 256 "$ZIP" | awk '{print $1}')
SIZE=$(du -h "$ZIP" | awk '{print $1}')
# The same zip under a name that never changes, for the README's link to the latest download
# (releases/latest/download/Iris.zip).
cp "$ZIP" "$DIST/Iris.zip"
echo "✓ $ZIP ($SIZE), and the same as $DIST/Iris.zip"
echo "  sha256 $SHA"
echo "  Nothing is published. To publish it as a GitHub release:"
echo "    git tag v$VERSION $SHORT && git push origin v$VERSION"
echo "    gh release create v$VERSION \"$ZIP\" \"$DIST/Iris.zip\" --draft --title \"Iris $VERSION\" --notes \"…\""
