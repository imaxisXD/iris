# Assembles what the installer carries, in -Out: iris.exe and the helper beside it.
#   pwsh gpui/windows/bundle.ps1 -Iris <iris.exe> -BrainBin <swift build bin dir> -Out <folder>
# The helper is iris-brain.exe with Swift's runtime, and llama.cpp's server (hash-checked) with
# Microsoft's C++ runtime under llama\: a fresh PC has none of them (measured in a Windows 11 VM:
# llama-server doesn't start without the C++ runtime; the DLLs beside it are enough).
param([string]$Iris, [string]$BrainBin, [string]$Out)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $Out, "$Out\llama" | Out-Null
Copy-Item $Iris "$Out\iris.exe"
Copy-Item "$BrainBin\iris-brain.exe" $Out
Get-ChildItem "$BrainBin\*.dll" -ErrorAction SilentlyContinue | Copy-Item -Destination $Out
$rt = Split-Path (Get-Command swiftCore.dll).Source
Get-ChildItem "$rt\*.dll" | Copy-Item -Destination $Out
$zip = Join-Path $env:RUNNER_TEMP $env:LLAMA_ZIP
if (-not (Test-Path $zip)) { Invoke-WebRequest "https://github.com/ggml-org/llama.cpp/releases/download/$env:LLAMA_TAG/$env:LLAMA_ZIP" -OutFile $zip }
if ((Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower() -ne $env:LLAMA_SHA256) { throw "llama.cpp zip hash" }
Expand-Archive -Force $zip "$Out\llama"
$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath
$crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory | Sort-Object FullName | Select-Object -Last 1
$omp = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.OpenMP" -Directory | Sort-Object FullName | Select-Object -Last 1
foreach ($d in @("$Out\llama", $Out)) {
  Copy-Item "$($crt.FullName)\*.dll" $d
  if ($omp) { Copy-Item "$($omp.FullName)\*.dll" $d }
}
"{0:N0} MB in {1}" -f ((Get-ChildItem -Recurse $Out | Measure-Object Length -Sum).Sum / 1MB), $Out
