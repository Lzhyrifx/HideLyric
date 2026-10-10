$ErrorActionPreference = "Stop"

$manifestPath =
Join-Path $PSScriptRoot "dist\manifest.json"

if (!(Test-Path $manifestPath)) {

    Write-Host "manifest.json not found."
    Read-Host "Press Enter to exit"

    exit 1
}


$manifest =
Get-Content `
        $manifestPath `
        -Raw |
        ConvertFrom-Json


$version =
$manifest.version


if ([string]::IsNullOrWhiteSpace($version)) {

    Write-Host "Unable to read version from manifest.json."
    Read-Host "Press Enter to exit"

    exit 1
}


$tag =
"v$version"


Set-Location $PSScriptRoot


$gitRoot =
git rev-parse --show-toplevel 2>$null


if (!$gitRoot) {

    Write-Host "This folder is not a Git repository."
    Read-Host "Press Enter to exit"

    exit 1
}


$status =
git status --porcelain


if ($status) {
    Write-Host "There are uncommitted changes."
    Write-Host $status
    Write-Host "Please commit your changes before creating a release."
    Read-Host "Press Enter to exit"

    exit 1
}


$existingTag =
git tag -l $tag


if ($existingTag) {
    Write-Host "Tag $tag already exists."
    Read-Host "Press Enter to exit"

    exit 1
}


Write-Host "Creating tag $tag..."


git tag -a $tag -m "Release $tag"


Write-Host "Pushing tag $tag to GitHub..."


git push origin $tag


Write-Host "Release triggered successfully."
Write-Host "Version: $version"
Write-Host "Tag: $tag"
Write-Host "GitHub Actions should now build the new release."