# Builds the brain for Windows (iris-brain.exe): the rules (IrisCore), Windows' engine
# (IrisLlama, llama.cpp's server) and the brain itself (gpui/brain/Sources/IrisBrain), without
# MLX or anything else only the Mac has. Run from a throwaway checkout: the repository's
# Package.swift (the Mac app's, with MLX) is replaced by one with only these.
#   pwsh gpui/windows/build-brain.ps1 [-Configuration release]
param([string]$Configuration = "release")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "../..")

# CoreGraphics is the Mac's; elsewhere Foundation has CGFloat, CGRect and CGPoint.
Get-ChildItem -Recurse Sources\IrisCore\*.swift | ForEach-Object {
  $t = Get-Content -Raw $_
  $u = $t -replace '(?m)^import CoreGraphics\r?$', "#if canImport(CoreGraphics)`nimport CoreGraphics`n#else`nimport Foundation`n#endif"
  if ($u -ne $t) { Set-Content -NoNewline -Encoding utf8NoBOM $_ $u }
}
@(
  '// swift-tools-version: 6.2',
  'import PackageDescription',
  'let package = Package(',
  '    name: "IrisBrainWindows",',
  '    products: [.executable(name: "iris-brain", targets: ["IrisBrain"])],',
  '    targets: [',
  '        .target(name: "IrisCore"),',
  '        .target(name: "IrisLlama", dependencies: ["IrisCore"], linkerSettings: [.linkedLibrary("ws2_32", .when(platforms: [.windows]))]),',
  '        .executableTarget(name: "IrisBrain", dependencies: ["IrisCore", "IrisLlama"], path: "gpui/brain/Sources/IrisBrain"),',
  '    ],',
  '    swiftLanguageModes: [.v5]',
  ')'
) -join "`n" | Set-Content -NoNewline -Encoding utf8NoBOM Package.swift
Remove-Item Package.resolved -ErrorAction SilentlyContinue
# The checkout's folder (on CI, the private repository's name) is written as C:\iris in the
# brain's file names and debug info, as build-app.sh writes /iris on the Mac.
$root = (Get-Location).Path
swift build -c $Configuration --product iris-brain `
  -Xswiftc -file-prefix-map -Xswiftc "$root=C:\iris" -Xcc "-ffile-prefix-map=$root=C:\iris"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$bin = swift build -c $Configuration --show-bin-path
"brain: $bin\iris-brain.exe"
