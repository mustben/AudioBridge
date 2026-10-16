param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $root 'Cargo.toml'
$metadataRaw = & cargo metadata --locked --no-deps --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$metadata = $metadataRaw | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq 'audio-bridge' })
if ($package.Count -ne 1) { throw 'Unexpected package identity' }
$version = $package[0].version
if ($version -notmatch '^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$') { throw 'Invalid package version' }
if (-not $SkipBuild) {
    & cargo build --locked --release --manifest-path $manifest --bin audio-bridge-tray --bin audio-bridge
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
}
$target = $metadata.target_directory
$files = [ordered]@{
    (Join-Path $target 'release/audio-bridge-tray.exe') = 'AudioBridge/AudioBridge.exe'
    (Join-Path $target 'release/audio-bridge.exe') = 'AudioBridge/AudioBridge-CLI.exe'
    (Join-Path $root 'README.md') = 'AudioBridge/README.md'
    (Join-Path $root 'README.fr.md') = 'AudioBridge/README.fr.md'
    (Join-Path $root 'LICENSE') = 'AudioBridge/LICENSE'
    (Join-Path $root 'CHANGELOG.md') = 'AudioBridge/CHANGELOG.md'
    (Join-Path $root 'DIAGNOSTICS.md') = 'AudioBridge/DIAGNOSTICS.md'
    (Join-Path $root 'PRIVACY.md') = 'AudioBridge/PRIVACY.md'
    (Join-Path $root 'VERIFICATION.md') = 'AudioBridge/VERIFICATION.md'
    (Join-Path $root 'CORRECTIONS.md') = 'AudioBridge/CORRECTIONS.md'
    (Join-Path $root 'CONTRIBUTING.md') = 'AudioBridge/CONTRIBUTING.md'
    (Join-Path $root 'PUBLISHING.md') = 'AudioBridge/PUBLISHING.md'
    (Join-Path $root 'SECURITY.md') = 'AudioBridge/SECURITY.md'
    (Join-Path $root 'THIRD_PARTY_NOTICES.md') = 'AudioBridge/THIRD_PARTY_NOTICES.md'
    (Join-Path $root 'assets/screenshots/tray-menu.png') = 'AudioBridge/assets/screenshots/tray-menu.png'
}
$licenseRoot = Join-Path $root 'THIRD_PARTY_LICENSES'
if (-not (Test-Path -LiteralPath $licenseRoot -PathType Container)) { throw 'Dependency licenses are missing' }
Get-ChildItem -LiteralPath $licenseRoot -File -Recurse | ForEach-Object {
    $relative = $_.FullName.Substring($licenseRoot.Length).TrimStart([char]92, [char]47).Replace([char]92, [char]47)
    $files[$_.FullName] = 'AudioBridge/THIRD_PARTY_LICENSES/' + $relative
}
foreach ($source in $files.Keys) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing package input: $source" }
}
function Assert-X64Pe([string]$Path, [int]$ExpectedSubsystem) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 96 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90) { throw "Invalid PE input: $Path" }
    $pe = [System.BitConverter]::ToInt32($bytes, 60)
    if ($pe -lt 64 -or $pe -gt ($bytes.Length - 94)) { throw "Invalid PE header offset: $Path" }
    if ([System.BitConverter]::ToUInt32($bytes, $pe) -ne 17744) { throw "Missing PE signature: $Path" }
    if ([System.BitConverter]::ToUInt16($bytes, $pe + 4) -ne 34404) { throw "Package requires x64 PE binaries: $Path" }
    $subsystem = [System.BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
    if ($subsystem -ne $ExpectedSubsystem) { throw "Unexpected executable subsystem $subsystem : $Path" }
}
Assert-X64Pe (Join-Path $target 'release/audio-bridge-tray.exe') 2
Assert-X64Pe (Join-Path $target 'release/audio-bridge.exe') 3
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$output = Join-Path $dist "AudioBridge-v$version-windows-x64.zip"
$temp = $output + '.' + [Guid]::NewGuid().ToString('N') + '.new'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::Open($temp, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($entry in $files.GetEnumerator()) {
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $entry.Key, $entry.Value, [System.IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $archive.Dispose() }
Move-Item -LiteralPath $temp -Destination $output -Force
$hash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
$checksum = Join-Path $dist 'SHA256SUMS.txt'
[System.IO.File]::WriteAllText($checksum, "$hash  $([System.IO.Path]::GetFileName($output))`n", [System.Text.UTF8Encoding]::new($false))
Write-Output "Package: $output"
Write-Output "SHA256: $hash"
