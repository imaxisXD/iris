#!/usr/bin/env bash
# Builds the GPUI Iris: the app (Rust, GPUI) with its brain (the Swift engine, run as a helper).
#
#   gpui/scripts/build-app.sh            gpui/build/Iris GPUI.app, a development copy: bundle ID
#                                        local.iris.gpui, so it never shares settings, the
#                                        Accessibility permission or a login item with the
#                                        installed Iris (local.iris.app), and starts on ⌃⌥I.
#   gpui/scripts/build-app.sh --as-iris  build/Iris.app at the checkout's root: Iris itself, as
#                                        scripts/build-app.sh (and so update.sh, ship.sh and
#                                        release.sh) makes it.
#
# Signed with the checkout's identity (scripts/lib.sh), so macOS keeps the Accessibility
# permission across builds. IRIS_RELEASE=1 builds a release (no checkout path recorded, the
# release channel, source paths stripped); IRIS_VERSION and IRIS_SOURCE_PATH as for the Swift
# build.
set -euo pipefail

GPUI="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$GPUI/.." && pwd)"
source "$ROOT/scripts/lib.sh"
iris_use_standard_path
iris_ensure_rust
AS_IRIS=false
[[ "${1:-}" == "--as-iris" ]] && AS_IRIS=true
if $AS_IRIS; then APP="$ROOT/build/Iris.app"; else APP="$GPUI/build/Iris GPUI.app"; fi
SIGN_IDENTITY="${SIGN_IDENTITY:-$(iris_signing_identity "$ROOT")}"

echo "→ Building the brain…"
"$GPUI/brain/build.sh"
BRAIN="$GPUI/brain/.build/xcode/Build/Products/Release"
echo "→ Building (release)…"
RUSTFLAGS_EXTRA=""
# A release names no folder on the Mac that built it.
if [[ -n "${IRIS_RELEASE:-}" ]]; then RUSTFLAGS_EXTRA="--remap-path-prefix=$ROOT=/iris --remap-path-prefix=$HOME=/home"; fi
# A release builds outside the checkout: GPUI's Metal compiler records where it ran.
TARGET="$GPUI/target/app"
[[ -n "${IRIS_RELEASE:-}" ]] && TARGET="/tmp/iris-release-target"
(cd "$GPUI" && RUSTFLAGS="${RUSTFLAGS:-} $RUSTFLAGS_EXTRA" cargo build --release --no-default-features --target-dir "$TARGET" 2>&1 | tail -1)

echo "→ Assembling app bundle…"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/iris" "$APP/Contents/MacOS/Iris"
# The brain beside it, and MLX's Metal shaders where MLX looks: the bundle's Resources.
cp "$BRAIN/iris-brain" "$APP/Contents/MacOS/iris-brain"
for bundle in "$BRAIN"/*.bundle; do cp -R "$bundle" "$APP/Contents/Resources/"; done
# Swift's back-deployment libraries the brain links (mlx-swift's use of Span) come with macOS
# 26 but not macOS 15, which Iris supports: embedded, as the Swift app's build does.
TOOLCHAIN_LIB="$(dirname "$(xcrun --find swiftc)")/../lib"
for lib in $(otool -L "$APP/Contents/MacOS/iris-brain" | awk '/@rpath\/libswift/{print $1}'); do
  name="${lib#@rpath/}"
  src=""
  for candidate in "$TOOLCHAIN_LIB"/swift-*/macosx/"$name" "$TOOLCHAIN_LIB"/swift/macosx/"$name"; do
    if [ -f "$candidate" ]; then src="$candidate"; break; fi
  done
  [ -n "$src" ] || { echo "Can't find $name in the toolchain to embed."; exit 1; }
  mkdir -p "$APP/Contents/Frameworks"
  cp "$src" "$APP/Contents/Frameworks/"
done
if [ -d "$APP/Contents/Frameworks" ]; then
  install_name_tool -add_rpath "@executable_path/../Frameworks" "$APP/Contents/MacOS/iris-brain" 2>/dev/null || true
fi

PLIST="$APP/Contents/Info.plist"
cp "$ROOT/scripts/Info.plist" "$PLIST"
if ! $AS_IRIS; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier local.iris.gpui" "$PLIST"
  /usr/libexec/PlistBuddy -c "Set :CFBundleName Iris GPUI" "$PLIST"
  /usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName Iris GPUI" "$PLIST"
fi
# Build number and commit (Settings › About), and the checkout Check for Updates compares
# with and Update & reopen rebuilds from. A release goes to Macs with no checkout.
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo 1)" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :IrisCommit string ${IRIS_COMMIT:-$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)}" "$PLIST"
if [ "${IRIS_RELEASE:-}" = 1 ]; then
  /usr/libexec/PlistBuddy -c "Add :IrisChannel string release" "$PLIST"
else
  /usr/libexec/PlistBuddy -c "Add :IrisSourcePath string ${IRIS_SOURCE_PATH:-$ROOT}" "$PLIST"
fi
# Drawn as the Swift app is: it's built against the macOS 15 SDK, so macOS 26 gives its windows
# the earlier look (12-point window buttons, its menus and controls). This app is built against
# a newer SDK; this asks macOS for that same look, so the two match.
/usr/libexec/PlistBuddy -c "Add :UIDesignRequiresCompatibility bool true" "$PLIST"
# The version is the changelog's newest issue (CHANGELOG-RULES.md).
VERSION="${IRIS_VERSION:-$(sed -nE 's/^# v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' "$ROOT/Sources/Iris/Resources/Changelog.md" | head -n 1)}"
[ -n "$VERSION" ] && /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$PLIST"
if [ -f "$ROOT/assets/AppIcon.icns" ]; then cp "$ROOT/assets/AppIcon.icns" "$APP/Contents/Resources/"; fi

echo "→ Signing with identity: $SIGN_IDENTITY"
# The brain first (MLX compiles Metal kernels at runtime: the same entitlements), then the app.
codesign --force --options runtime --entitlements "$ROOT/scripts/Iris.entitlements" --sign "$SIGN_IDENTITY" "$APP/Contents/MacOS/iris-brain"
codesign --force --deep --options runtime --entitlements "$ROOT/scripts/Iris.entitlements" --sign "$SIGN_IDENTITY" "$APP"
echo "✓ $APP"
