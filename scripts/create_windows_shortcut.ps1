$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Pythonw = Join-Path $ProjectRoot ".venv\Scripts\pythonw.exe"
if (-not (Test-Path -LiteralPath $Pythonw -PathType Leaf)) {
    throw "Installed environment pythonw.exe was not found: $Pythonw"
}

$Desktop = [Environment]::GetFolderPath("Desktop")
if ([string]::IsNullOrWhiteSpace($Desktop)) {
    throw "Windows Desktop folder could not be located."
}
$ShortcutPath = Join-Path $Desktop "InstPlot.lnk"
$Shell = New-Object -ComObject WScript.Shell

if (Test-Path -LiteralPath $ShortcutPath) {
    $Existing = $Shell.CreateShortcut($ShortcutPath)
    if (
        $Existing.TargetPath -eq $Pythonw -and
        $Existing.Arguments -eq "-m InstPlot" -and
        $Existing.WorkingDirectory -eq $ProjectRoot
    ) {
        Write-Host "InstPlot desktop shortcut is already current: $ShortcutPath"
    } else {
        Write-Warning "Existing desktop shortcut was preserved: $ShortcutPath"
    }
    exit 0
}

$Shortcut = $Shell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $Pythonw
$Shortcut.Arguments = "-m InstPlot"
$Shortcut.WorkingDirectory = $ProjectRoot
$Shortcut.Description = "InstPlot experimental data visualization"
$Icon = Join-Path $ProjectRoot "logo.ico"
if (Test-Path -LiteralPath $Icon -PathType Leaf) {
    $Shortcut.IconLocation = $Icon
}
$Shortcut.Save()
Write-Host "Created InstPlot desktop shortcut: $ShortcutPath"
