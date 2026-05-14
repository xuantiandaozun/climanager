param(
    [string]$AppName = "CLIManager",
    [string]$StageDir = "build/msix-stage",
    [string]$OutputDir = "build",
    [string]$MakeAppxPath = ""
)

$ErrorActionPreference = "Stop"

if (-not $MakeAppxPath) {
    $searchRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "${env:ProgramFiles}\Windows Kits\10\bin",
        "D:\Windows Kits\10\bin"
    )

    $found = $searchRoots |
        Where-Object { Test-Path $_ } |
        ForEach-Object { Get-ChildItem $_ -Recurse -Filter MakeAppx.exe -ErrorAction SilentlyContinue } |
        Where-Object { $_.FullName -match '\\x64\\MakeAppx.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1

    if ($found) {
        $MakeAppxPath = $found.FullName
    }
}

if (-not $MakeAppxPath -or -not (Test-Path $MakeAppxPath)) {
    Write-Error "MakeAppx.exe not found. Install Windows SDK or provide -MakeAppxPath."
    exit 1
}

Write-Host "Using MakeAppx: $MakeAppxPath"

if (-not (Test-Path $StageDir)) {
    Write-Error "Stage directory not found: $StageDir. Run stage.ps1 first."
    exit 1
}

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$msixFile = Join-Path $OutputDir "$AppName.msix"
if (Test-Path $msixFile) {
    Remove-Item -Path $msixFile -Force
}

Write-Host "Packing $StageDir -> $msixFile"

& $MakeAppxPath pack /d $StageDir /p $msixFile /o
if ($LASTEXITCODE -ne 0) {
    Write-Error "MakeAppx failed with exit code $LASTEXITCODE"
    exit 1
}

$size = (Get-ChildItem $msixFile).Length
Write-Host "MSIX package created: $msixFile ($([math]::Round($size / 1MB, 2)) MB)"
