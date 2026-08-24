$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$InstallerRoot = Join-Path $ProjectRoot ".installer"
$UvRoot = Join-Path $InstallerRoot "uv"
$Uv = Join-Path $UvRoot "uv.exe"
$InstallerUrl = "https://astral.sh/uv/0.12.5/install.ps1"
$InstallerSha256 = "ca1ad558c65d31e2d3a24464638aff90bfb81d6c72428b4e71d6f55944a68541"

foreach ($Directory in @($InstallerRoot, $UvRoot)) {
    if (Test-Path -LiteralPath $Directory) {
        $Item = Get-Item -LiteralPath $Directory -Force
        if (-not $Item.PSIsContainer -or ($Item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Unsafe uv bootstrap directory: $Directory"
        }
    }
}

if (Test-Path -LiteralPath $Uv -PathType Leaf) {
    $Item = Get-Item -LiteralPath $Uv -Force
    if ($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Unsafe uv bootstrap executable: $Uv"
    }
    exit 0
}

New-Item -ItemType Directory -Path $UvRoot -Force | Out-Null
$Temporary = Join-Path ([IO.Path]::GetTempPath()) ("instplot-uv-installer-{0}.ps1" -f [Guid]::NewGuid().ToString("N"))
try {
    Invoke-WebRequest -UseBasicParsing -Uri $InstallerUrl -OutFile $Temporary
    $Actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Temporary).Hash.ToLowerInvariant()
    if ($Actual -ne $InstallerSha256) {
        throw "uv installer checksum mismatch; refusing to execute it."
    }
    $env:UV_UNMANAGED_INSTALL = $UvRoot
    $env:UV_NO_MODIFY_PATH = "1"
    & powershell -NoProfile -ExecutionPolicy Bypass -File $Temporary
    if ($LASTEXITCODE -ne 0) {
        throw "Verified uv bootstrap exited with code $LASTEXITCODE"
    }
} finally {
    Remove-Item -LiteralPath $Temporary -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path -LiteralPath $Uv -PathType Leaf)) {
    throw "Verified uv bootstrap did not create the expected executable."
}
