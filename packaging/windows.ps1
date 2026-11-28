param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    [string]$Compiler = (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe')
)

$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$runtimePattern = 'VC\Redist\MSVC\**\x64\Microsoft.VC*.CRT\vcruntime140.dll'
$candidates = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find $runtimePattern
if ($LASTEXITCODE -ne 0) {
    throw "Could not locate the x64 MSVC redistributable files (vswhere exit code $LASTEXITCODE)."
}
$runtime = $candidates | ForEach-Object {
    # Accept only the desktop x64 layout directly under a numbered redistributable.
    if ($_ -match '\\VC\\Redist\\MSVC\\(?<Version>\d+(?:\.\d+){1,3})\\x64\\Microsoft\.VC[^\\]+\.CRT\\vcruntime140\.dll$') {
        [pscustomobject]@{ Version = [version]$Matches.Version; Path = $_ }
    }
} | Sort-Object Version -Descending | Select-Object -First 1 -ExpandProperty Path
if (!$runtime) {
    throw 'Install the x64 MSVC redistributable files in Visual Studio Build Tools.'
}

$script = Join-Path $PSScriptRoot 'windows.iss'
& $Compiler /Qp "/DEssaysVersion=$Version" "/DRuntimeDll=$runtime" $script
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
