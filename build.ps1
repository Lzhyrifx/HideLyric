$ErrorActionPreference = "Stop"


$projectDir =
$PSScriptRoot


$pluginDir =
Join-Path $projectDir "plugin"


$releaseDir =
Join-Path $projectDir "release"


$manifestSource =
Join-Path $projectDir "manifest.json"


$mainJsSource =
Join-Path $projectDir "main.js"


$exeSource =
Join-Path $projectDir "HideLyric.exe"


if (!(Test-Path $manifestSource)) {

    Write-Error "manifest.json not found: $manifestSource"

    exit 1
}


if (!(Test-Path $mainJsSource)) {

    Write-Error "main.js not found: $mainJsSource"

    exit 1
}


if (!(Test-Path $exeSource)) {

    Write-Error "HideLyric.exe not found: $exeSource"

    exit 1
}


Write-Host "Updating plugin folder..."



if (!(Test-Path $pluginDir)) {

    New-Item `
        -ItemType Directory `
        -Path $pluginDir |
            Out-Null
}



Get-ChildItem `
    $pluginDir `
    -Force |
        Remove-Item `
        -Recurse `
        -Force



Copy-Item `
    $manifestSource `
    $pluginDir `
    -Force


Copy-Item `
    $mainJsSource `
    $pluginDir `
    -Force


Copy-Item `
    $exeSource `
    $pluginDir `
    -Force



$previewSource =
Join-Path $projectDir "preview.png"


if (Test-Path $previewSource) {

    Copy-Item `
        $previewSource `
        $pluginDir `
        -Force
}


$manifestPath =
Join-Path $pluginDir "manifest.json"


$manifest =
Get-Content `
        $manifestPath `
        -Raw |
        ConvertFrom-Json


$version =
$manifest.version


if ([string]::IsNullOrWhiteSpace($version)) {

    Write-Error "Unable to read version from manifest.json"

    exit 1
}


if (!(Test-Path $releaseDir)) {

    New-Item `
        -ItemType Directory `
        -Path $releaseDir |
            Out-Null
}


$zipPath =
Join-Path `
        $releaseDir `
        "HideLyric-v$version.zip"


$pluginPath =
Join-Path `
        $releaseDir `
        "HideLyric-v$version.plugin"



Remove-Item `
    $zipPath `
    -Force `
    -ErrorAction SilentlyContinue


Remove-Item `
    $pluginPath `
    -Force `
    -ErrorAction SilentlyContinue



Write-Host "Packaging plugin..."


Compress-Archive `
    -Path "$pluginDir\*" `
    -DestinationPath $zipPath `
    -Force


Rename-Item `
    $zipPath `
    $pluginPath `
    -Force


Write-Host "Build completed"
Write-Host "Version: $version"
Write-Host "File: $pluginPath"