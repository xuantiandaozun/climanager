param(
    [string]$AppName = "CLIManager",
    [string]$MsixFile = "",
    [string]$PfxFile = "scripts/msix/dev.pfx",
    [string]$PfxPassword = "",
    [string]$SignTool = "",
    [string]$OutputDir = "build"
)

$ErrorActionPreference = "Stop"

if (-not $MsixFile) {
    $MsixFile = Join-Path $OutputDir "$AppName.msix"
}

if (-not (Test-Path $MsixFile)) {
    Write-Error "MSIX file not found: $MsixFile. Run pack.ps1 first."
    exit 1
}

if (-not $SignTool) {
    $searchRoots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "${env:ProgramFiles}\Windows Kits\10\bin",
        "D:\Windows Kits\10\bin"
    )

    $found  = $searchRoots |
        Where-Object { Test-Path $_ } |
        ForEach-Object { Get-ChildItem $_ -Recurse -Filter SignTool.exe -ErrorAction SilentlyContinue } |
        Where-Object { $_.FullName -match '\\x64\\SignTool.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1

    if ($found) {
        $SignTool = $found.FullName
    }
}

if (-not $SignTool -or -not (Test-Path $SignTool)) {
    Write-Error "SignTool.exe not found. Install Windows SDK or provide -SignTool path."
    exit 1
}

Write-Host "Using SignTool: $SignTool"

if (-not (Test-Path $PfxFile)) {
    Write-Warning "PFX file not found: $PfxFile"
    Write-Host "Creating a self-signed development certificate..."
    $dnsName = "$AppName.dev"
    & $SignTool sign /fd SHA256 /a /sm /s My /n "$dnsName" /fd SHA256 "$MsixFile"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Signing failed"
        exit 1
    }
} else {
    $signArgs = @("sign", "/fd", "SHA256")
    if ($PfxPassword) {
        $signArgs += "/p", $PfxPassword
    }
    $signArgs += "/f", (Get-ChildItem $PfxFile).FullName
    $signArgs += "/fd", "SHA256"
    $signArgs += "`"$MsixFile`""

    Write-Host "Signing with PFX: $PfxFile"
    & $SignTool $signArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Signing failed with exit code $LASTEXITCODE"
        exit 1
    }
}

Write-Host "MSIX signed successfully: $MsixFile"
