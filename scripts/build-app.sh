#!/usr/bin/env bash
# Builds "Iris.app" into ./build (the GPUI app; IRIS_SWIFT_APP=1: the SwiftUI app, as below).
#
# Uses xcodebuild (not `swift build`) because MLX's Metal shaders are only
# compiled by Xcode's build system. The SwiftPM resource bundles, including
# mlx-swift_Cmlx.bundle with default.metallib, go in Contents/Resources,
# where MLX and Bundle.module both look.
#
# Signing: macOS ties the Accessibility permission to the code signature.
# An ad-hoc signature changes on every build, so you'd re-grant permission
# each time. iris_signing_identity in lib.sh says which identity is used.
# Keep using the same one on a Mac, or Accessibility has to be granted again.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib.sh"
# Iris is the GPUI app now (gpui/: its UI in Rust, the Swift engine as its brain). This script
# builds it as build/Iris.app, for update.sh, ship.sh and release.sh alike. IRIS_SWIFT_APP=1
# builds the earlier SwiftUI app instead, which is deprecated (CLAUDE.md): a reference only, for
# the checks that still drive it (self-check.sh, panel-check.sh).
if [[ -z "${IRIS_SWIFT_APP:-}" ]]; then
  exec "$ROOT/gpui/scripts/build-app.sh" --as-iris
fi
echo "warning: the SwiftUI app (Sources/Iris) is deprecated. Iris is the GPUI app (gpui/); this builds the old one, as a reference only." >&2
CONFIG="${CONFIG:-Release}"
DERIVED="$ROOT/.build/xcode"
PRODUCTS="$DERIVED/Build/Products/$CONFIG"
APP="$ROOT/build/Iris.app"
SIGN_IDENTITY="$(iris_signing_identity "$ROOT")"

cd "$ROOT"
mkdir -p "$ROOT/.build"   # a fresh checkout (update.sh's install tree) has none yet
# Without these, xcodebuild compiles every module of the package with coverage counters
# (-profile-generate, -fprofile-instr-generate): a slower app that writes default.profraw
# wherever it starts.
# A release names no folder on the Mac that built it: source paths compiled into the app (C++
# asserts in MLX, Swift's #file) start from "/iris" instead of this checkout's path.
PREFIX_FLAGS=()
if [[ -n "${IRIS_RELEASE:-}" ]]; then
  PREFIX_FLAGS=(
    "OTHER_CFLAGS=\$(inherited) -ffile-prefix-map=$ROOT=/iris"
    "OTHER_CPLUSPLUSFLAGS=\$(inherited) -ffile-prefix-map=$ROOT=/iris"
    "OTHER_SWIFT_FLAGS=\$(inherited) -file-prefix-map $ROOT=/iris"
  )
fi
echo "→ Building ($CONFIG)…"
xcodebuild -scheme Iris -configuration "$CONFIG" \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$DERIVED" \
  -skipMacroValidation -skipPackagePluginValidation \
  CLANG_COVERAGE_MAPPING=NO CLANG_ENABLE_CODE_COVERAGE=NO \
  ${PREFIX_FLAGS[@]+"${PREFIX_FLAGS[@]}"} \
  build > "$ROOT/.build/xcode-app.log" 2>&1 || {
    grep -E "error:" "$ROOT/.build/xcode-app.log" | head -20
    echo "Build failed. Full log: .build/xcode-app.log"; exit 1; }

echo "→ Assembling app bundle…"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$PRODUCTS/Iris" "$APP/Contents/MacOS/Iris"
for bundle in "$PRODUCTS"/*.bundle; do
  cp -R "$bundle" "$APP/Contents/Resources/"
done
# Swift's back-deployment libraries (libswiftCompatibilitySpan.dylib, from mlx-swift's use of
# Span) come with macOS 26 but not macOS 15, which Iris supports. Xcode embeds them in app
# targets; this bundle is assembled by hand, so copy them into Contents/Frameworks. The
# binary looks in /usr/lib/swift first, so on macOS 26 the system's copy still wins.
TOOLCHAIN_LIB="$(dirname "$(xcrun --find swiftc)")/../lib"
for lib in $(otool -L "$APP/Contents/MacOS/Iris" | awk '/@rpath\/libswift/{print $1}'); do
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
  install_name_tool -add_rpath "@executable_path/../Frameworks" "$APP/Contents/MacOS/Iris"
fi
cp "$ROOT/scripts/Info.plist" "$APP/Contents/Info.plist"
# Build number and commit for Settings → Privacy → About, and the checkout path that
# "Check for Updates…" pulls and rebuilds from.
PLIST="$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo 1)" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :IrisCommit string $(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)" "$PLIST"
# update.sh builds in a clean worktree of the commit, but updates come from the checkout.
# A release build (scripts/release.sh, IRIS_RELEASE=1) goes to other Macs, which have no
# checkout, so it records none.
if [ "${IRIS_RELEASE:-}" = 1 ]; then
  /usr/libexec/PlistBuddy -c "Add :IrisChannel string release" "$PLIST"
else
  /usr/libexec/PlistBuddy -c "Add :IrisSourcePath string ${IRIS_SOURCE_PATH:-$ROOT}" "$PLIST"
fi
# The version is the changelog's newest issue (CHANGELOG-RULES.md), so Settings › Updates
# marks the right one as yours; release.sh passes the same one in IRIS_VERSION.
VERSION="${IRIS_VERSION:-$(sed -nE 's/^# v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' "$ROOT/Sources/Iris/Resources/Changelog.md" | head -n 1)}"
if [ -n "$VERSION" ]; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$PLIST"
fi
if [ -f "$ROOT/assets/AppIcon.icns" ]; then cp "$ROOT/assets/AppIcon.icns" "$APP/Contents/Resources/"; fi

echo "→ Signing with identity: $SIGN_IDENTITY"
codesign --force --deep --options runtime \
  --entitlements "$ROOT/scripts/Iris.entitlements" \
  --sign "$SIGN_IDENTITY" "$APP"

echo "✓ $APP"
echo "  Run: open \"$APP\""
