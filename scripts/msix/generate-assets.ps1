param(
    [string]$SourceLogo = "logo.png",
    [string]$MsixAssetsDir = "src-tauri/msix/Assets",
    [string]$TauriIconsDir = "src-tauri/icons"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $SourceLogo)) {
    Write-Error "Source logo not found: $SourceLogo"
    exit 1
}

Add-Type -AssemblyName System.Drawing

$logo = [System.Drawing.Image]::FromFile((Get-ChildItem $SourceLogo).FullName)

$msixAssets = @(
    ,@("StoreLogo.png", 50, 50)
    ,@("Square44x44Logo.png", 44, 44)
    ,@("Square71x71Logo.png", 71, 71)
    ,@("Square150x150Logo.png", 150, 150)
    ,@("Square310x310Logo.png", 310, 310)
    ,@("Wide310x150Logo.png", 310, 150)
    ,@("SplashScreen.png", 620, 300)
)

$scales = @(100, 125, 150, 200, 400)

if (-not (Test-Path $MsixAssetsDir)) {
    New-Item -ItemType Directory -Path $MsixAssetsDir -Force | Out-Null
}

function Resize-Image {
    param([System.Drawing.Image]$Image, [int]$Width, [int]$Height)
    $bmp = [System.Drawing.Bitmap]::new($Width, $Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($Image, 0, 0, $Width, $Height)
    $g.Dispose()
    return $bmp
}

foreach ($asset in $msixAssets) {
    $name = $asset[0]
    $w = $asset[1]
    $h = $asset[2]

    foreach ($scale in $scales) {
        $sw = [math]::Round($w * $scale / 100)
        $sh = [math]::Round($h * $scale / 100)
        $filename = $name -replace '\.png$', ".scale-$scale.png"
        $outPath = Join-Path $MsixAssetsDir $filename
        $resized = Resize-Image $logo $sw $sh
        $resized.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
        $resized.Dispose()
        Write-Host "  Generated: $filename (${sw}x${sh})"
    }

    $outPath = Join-Path $MsixAssetsDir $name
    $resized = Resize-Image $logo $w $h
    $resized.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $resized.Dispose()
    Write-Host "  Generated: $name (${w}x${h})"
}

$tauriSizes = @(
    ,@("32x32.png", 32)
    ,@("64x64.png", 64)
    ,@("128x128.png", 128)
    ,@("128x128@2x.png", 256)
    ,@("icon.png", 512)
    ,@("source.png", 1024)
)

if (-not (Test-Path $TauriIconsDir)) {
    New-Item -ItemType Directory -Path $TauriIconsDir -Force | Out-Null
}

foreach ($icon in $tauriSizes) {
    $name = $icon[0]
    $size = $icon[1]
    $outPath = Join-Path $TauriIconsDir $name
    $resized = Resize-Image $logo $size $size
    $resized.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $resized.Dispose()
    Write-Host "  Generated: $name (${size}x${size})"
}

$logo.Dispose()
Write-Host ""
Write-Host "All assets generated successfully."
