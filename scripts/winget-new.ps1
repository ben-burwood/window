#!/usr/bin/env pwsh
<#
.SYNOPSIS  First-time ("new") WinGet submission for window apps not yet in winget-pkgs.
.DESCRIPTION
  Generates the 3 WinGet manifests (version/installer/defaultLocale) for each requested app
  from the metadata table below, computes the installer SHA256 from the published GitHub
  release asset, then opens a PR to microsoft/winget-pkgs via wingetcreate submit.

  Use this ONCE per app. `wingetcreate update` (see .github/workflows/winget.yml) only works
  once a package already exists in winget-pkgs, so brand-new packages must be submitted here
  first. After the first version merges, winget.yml keeps them updated on every release.
.PARAMETER Version  Release version without leading v, e.g. 0.1.3 (must match a published tag).
.PARAMETER Apps     App names to submit. Defaults to the apps still needing a first submission.
.PARAMETER Token    GitHub PAT (classic, public_repo). Defaults to $env:WINGET_TOKEN.
.PARAMETER DryRun   Generate + validate + print manifests, but do NOT submit.
.EXAMPLE  ./scripts/winget-new.ps1 -Version 0.1.3 -DryRun
.EXAMPLE  ./scripts/winget-new.ps1 -Version 0.1.3 -Apps doc-viewer -Token $env:WINGET_TOKEN
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Version,
  [string[]]$Apps = @('doc-viewer','map-windower','image-shutter','drawing-paner','model-glazer'),
  [string]$Token = $env:WINGET_TOKEN,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$repo = 'ben-burwood/window'

$meta = @{
  'data-framer'   = @{ Id='BenBurwood.DataFramer';   Name='Data Framer';   Desc='A tiny, fast, native CSV / Parquet viewer'; Moniker='data-framer';   Tags=@('csv','parquet','data','viewer') }
  'map-windower'  = @{ Id='BenBurwood.MapWindower';  Name='Map Windower';  Desc='A tiny, fast, native GeoJSON viewer';       Moniker='map-windower';  Tags=@('geojson','map','gis','viewer') }
  'doc-viewer'    = @{ Id='BenBurwood.DocViewer';    Name='Doc Viewer';    Desc='A tiny, fast, native PDF viewer';           Moniker='doc-viewer';    Tags=@('pdf','document','viewer') }
  'image-shutter' = @{ Id='BenBurwood.ImageShutter'; Name='Image Shutter'; Desc='A tiny, fast, native SVG viewer';           Moniker='image-shutter'; Tags=@('svg','image','viewer') }
  'drawing-paner' = @{ Id='BenBurwood.DrawingPaner'; Name='Drawing Paner'; Desc='A tiny, fast, native DXF viewer';           Moniker='drawing-paner'; Tags=@('dxf','cad','drawing','viewer') }
  'model-glazer'  = @{ Id='BenBurwood.ModelGlazer';  Name='Model Glazer';  Desc='A tiny, fast, native 3D model viewer';      Moniker='model-glazer';  Tags=@('stl','obj','3mf','3d','viewer') }
}

$publisher  = 'Ben Burwood'
$homepage   = "https://github.com/$repo"
$supportUrl = "https://github.com/$repo/issues"
$licenseUrl = "https://github.com/$repo/blob/main/LICENSE"

$wc = Join-Path $PSScriptRoot 'wingetcreate.exe'
if (-not (Test-Path $wc)) { Invoke-WebRequest https://aka.ms/wingetcreate/latest -OutFile $wc }

$workRoot = Join-Path ([System.IO.Path]::GetTempPath()) "winget-new-$Version"
New-Item -ItemType Directory -Force -Path $workRoot | Out-Null

foreach ($app in $Apps) {
  if (-not $meta.ContainsKey($app)) { throw "Unknown app '$app'. Known: $($meta.Keys -join ', ')" }
  $m = $meta[$app]
  $installerUrl = "https://github.com/$repo/releases/download/$Version/${app}_${Version}_x64-setup.exe"

  Write-Host "== $($m.Id) $Version ==" -ForegroundColor Cyan
  $tmpExe = Join-Path $workRoot "$app.exe"
  Invoke-WebRequest $installerUrl -OutFile $tmpExe
  $sha = (Get-FileHash -Algorithm SHA256 $tmpExe).Hash.ToUpper()

  $pkgDir = Join-Path $workRoot "$($m.Id)\$Version"
  New-Item -ItemType Directory -Force -Path $pkgDir | Out-Null
  $tagsYaml = ($m.Tags | ForEach-Object { "  - $_" }) -join "`n"

  @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.1.6.0.schema.json
PackageIdentifier: $($m.Id)
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
"@ | Set-Content -Encoding UTF8 (Join-Path $pkgDir "$($m.Id).yaml")

  @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json
PackageIdentifier: $($m.Id)
PackageVersion: $Version
InstallerLocale: en-US
InstallerType: nullsoft
Installers:
  - Architecture: x64
    InstallerUrl: $installerUrl
    InstallerSha256: $sha
ManifestType: installer
ManifestVersion: 1.6.0
"@ | Set-Content -Encoding UTF8 (Join-Path $pkgDir "$($m.Id).installer.yaml")

  @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.1.6.0.schema.json
PackageIdentifier: $($m.Id)
PackageVersion: $Version
PackageLocale: en-US
Publisher: $publisher
PublisherUrl: https://github.com/ben-burwood
PublisherSupportUrl: $supportUrl
PackageName: $($m.Name)
PackageUrl: $homepage
License: MIT
LicenseUrl: $licenseUrl
Copyright: Copyright (c) Ben Burwood
ShortDescription: $($m.Desc)
Moniker: $($m.Moniker)
Tags:
$tagsYaml
ManifestType: defaultLocale
ManifestVersion: 1.6.0
"@ | Set-Content -Encoding UTF8 (Join-Path $pkgDir "$($m.Id).locale.en-US.yaml")

  if ($DryRun) {
    Write-Host "DryRun: manifests in $pkgDir" -ForegroundColor Yellow
    Get-ChildItem $pkgDir | ForEach-Object { Write-Host "---- $($_.Name) ----"; Get-Content $_.FullName }
    if (Get-Command winget -ErrorAction SilentlyContinue) { winget validate --manifest $pkgDir }
  } else {
    if (-not $Token) { throw "No token. Pass -Token or set WINGET_TOKEN." }
    & $wc submit $pkgDir --token $Token
    if ($LASTEXITCODE -ne 0) { throw "wingetcreate submit failed for $($m.Id) (exit $LASTEXITCODE)." }
  }
}
