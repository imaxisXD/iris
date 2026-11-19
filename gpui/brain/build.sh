#!/usr/bin/env bash
# Builds the brain (iris-brain) with Xcode's build system: MLX's Metal shaders are only compiled
# by it. The program lands in .build/xcode/Build/Products/Release, beside MLX's shader bundle.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p .build
ROOT="$(cd ../.. && pwd)"
# A release names no folder on the Mac that built it (as the Swift app's build does).
PREFIX_FLAGS=()
if [[ -n "${IRIS_RELEASE:-}" ]]; then
  PREFIX_FLAGS=(
    "OTHER_CFLAGS=\$(inherited) -ffile-prefix-map=$ROOT=/iris"
    "OTHER_CPLUSPLUSFLAGS=\$(inherited) -ffile-prefix-map=$ROOT=/iris"
    "OTHER_SWIFT_FLAGS=\$(inherited) -file-prefix-map $ROOT=/iris"
  )
fi
xcodebuild -scheme IrisBrain -configuration Release -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath .build/xcode -skipMacroValidation -skipPackagePluginValidation \
  CLANG_COVERAGE_MAPPING=NO CLANG_ENABLE_CODE_COVERAGE=NO ${PREFIX_FLAGS[@]+"${PREFIX_FLAGS[@]}"} build > .build/build.log 2>&1 || {
    grep -E "error:" .build/build.log | head -20
    echo "Build failed. Full log: gpui/brain/.build/build.log"; exit 1; }
echo "✓ $(pwd)/.build/xcode/Build/Products/Release/iris-brain"
