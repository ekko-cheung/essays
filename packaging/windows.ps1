param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    [string]$Compiler = (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe')
)

$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$runtimePattern = 'VC\Redist\MSVC\**\x64\Microsoft.VC*.CRT\vcruntime140.dll'
$runtime = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find $runtimePattern |
    Where-Object { $_ -notmatch '\\onecore\\' } |
    Sort-Object { [version](([IO.FileInfo]$_).Directory.Parent.Parent.Name) } -Descending |
    Select-Object -First 1
if ($LASTEXITCODE -ne 0 -or !$runtime) {
    throw 'Install the x64 MSVC redistributable files in Visual Studio Build Tools.'
}

$script = Join-Path $PSScriptRoot 'windows.iss'
& $Compiler /Qp "/DEssaysVersion=$Version" "/DRuntimeDll=$runtime" $script
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
