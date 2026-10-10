$ErrorActionPreference = "Stop"


$projectDir = $PSScriptRoot


$distDir = Join-Path $projectDir "dist"


$releaseDir = Join-Path $projectDir "release"


$exeSource = Join-Path $projectDir "target\release\HideLyric.exe"


$manifestPath = Join-Path $distDir "manifest.json"
$mainJsPath = Join-Path $distDir "main.js"
$previewPath = Join-Path $distDir "preview.png"
$exeDestination = Join-Path $distDir "HideLyric.exe"


if (!(Test-Path $distDir -PathType Container)) {
    throw "dist directory not found: $distDir"
}


foreach ($path in @($manifestPath, $mainJsPath, $previewPath)) {
    if (!(Test-Path $path -PathType Leaf)) {
        throw "Plugin file not found: $path"
    }
}


if (!(Test-Path $exeSource -PathType Leaf)) {
    throw "HideLyric.exe not found: $exeSource. Run cargo build --release first."
}


$manifest = Get-Content $manifestPath -Raw |
        ConvertFrom-Json

$version = $manifest.version


if ([string]::IsNullOrWhiteSpace($version)) {
    throw "Unable to read version from: $manifestPath"
}


Write-Host "Updating HideLyric.exe..."

Copy-Item `
    -LiteralPath $exeSource `
    -Destination $exeDestination `
    -Force


if (!(Test-Path $releaseDir -PathType Container)) {
    New-Item `
        -ItemType Directory `
        -Path $releaseDir |
            Out-Null
}


$zipPath = Join-Path $releaseDir "HideLyric-v$version.zip"
$pluginPath = Join-Path $releaseDir "HideLyric-v$version.plugin"


Remove-Item `
    -LiteralPath $zipPath, $pluginPath `
    -Force `
    -ErrorAction SilentlyContinue

Write-Host "Packaging plugin v$version..."


Compress-Archive `
    -Path "$distDir\*" `
    -DestinationPath $zipPath `
    -Force


Move-Item `
    -LiteralPath $zipPath `
    -Destination $pluginPath `
    -Force


Write-Host "Build completed"
Write-Host "Version: $version"
Write-Host "File: $pluginPath"