[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $repositoryRoot
try {
    & node (Join-Path $PSScriptRoot 'verify-licenses.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw "License verification exited with code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}
