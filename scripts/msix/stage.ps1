param(
    [string]$AppName = "CLIManager",
    [string]$ExePath = "src-tauri/target/release",
    [string]$StageDir = "build/msix-stage",
    [string]$ManifestDir = "src-tauri/msix",
    [string]$MsixAssetsDir = "src-tauri/msix/Assets"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ExePath)) {
    Write-Error "Release directory not found: $ExePath"
    exit 1
}

$exe = Get-ChildItem -Path $ExePath -Filter "*.exe" | Select-Object -First 1
if (-not $exe) {
    Write-Error "No executable found in $ExePath"
    exit 1
}

Write-Host "Using executable: $($exe.Name)"

if (Test-Path $StageDir) {
    Remove-Item -Path $StageDir -Recurse -Force
}
New-Item -ItemType Directory -Path $StageDir -Force | Out-Null

Copy-Item -Path $exe.FullName -Destination $StageDir
Write-Host "  Copied: $($exe.Name)"

$manifest = Join-Path $ManifestDir "AppxManifest.xml.tpl"
if (Test-Path $manifest) {
$content = [System.IO.File]::ReadAllText($manifest)
$content = $content -replace '\{\{AppName\}\}', $AppName
$content = $content -replace '\{\{ExeName\}\}', $exe.Name
$outManifest = Join-Path $StageDir "AppxManifest.xml"
[System.IO.File]::WriteAllText($outManifest, $content, [System.Text.Encoding]::UTF8)
Write-Host "  Generated: AppxManifest.xml"
} else {
    Write-Error "Manifest template not found: $manifest"
    exit 1
}

if (Test-Path $MsixAssetsDir) {
    $assets = Join-Path $StageDir "Assets"
    New-Item -ItemType Directory -Path $assets -Force | Out-Null
    Copy-Item -Path "$MsixAssetsDir/*" -Destination $assets -Recurse
    Write-Host "  Copied: Assets/"
} else {
    Write-Warning "MSIX Assets directory not found: $MsixAssetsDir"
}

$stageSize = (Get-ChildItem -Path $StageDir -Recurse | Measure-Object -Property Length -Sum).Sum
Write-Host "  Stage size: $([math]::Round($stageSize / 1MB, 2)) MB"
Write-Host ""
Write-Host "Staging complete: $StageDir"
