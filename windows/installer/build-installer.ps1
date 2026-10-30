[CmdletBinding()]
param(
    [string]$Sign = $env:EDUTOOLS_CERT_THUMBPRINT,
    [string]$TimestampUrl = $env:EDUTOOLS_TIMESTAMP_URL
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Find-Iscc {
    $command = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $uninstallKeys = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup 6_is1'
    )
    foreach ($key in $uninstallKeys) {
        $install = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue
        if ($null -ne $install -and -not [string]::IsNullOrWhiteSpace($install.InstallLocation)) {
            $candidate = Join-Path $install.InstallLocation 'ISCC.exe'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                return $candidate
            }
        }
    }

    $programRoots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Select-Object -Unique
    foreach ($root in $programRoots) {
        $candidate = Join-Path $root 'Inno Setup 6\ISCC.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }
    throw 'ISCC.exe를 찾지 못했습니다. Inno Setup 6 설치 또는 PATH/설치 레지스트리를 확인하세요.'
}

function Find-SignTool {
    $command = Get-Command 'signtool.exe' -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $roots = @()
    foreach ($key in @(
        'HKLM:\Software\Microsoft\Windows Kits\Installed Roots',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows Kits\Installed Roots'
    )) {
        $properties = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue
        if ($null -ne $properties -and -not [string]::IsNullOrWhiteSpace($properties.KitsRoot10)) {
            $roots += $properties.KitsRoot10
        }
    }
    foreach ($root in ($roots | Select-Object -Unique)) {
        $bin = Join-Path $root 'bin'
        $versions = Get-ChildItem -LiteralPath $bin -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match '^10\.0\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending
        foreach ($version in $versions) {
            $candidate = Join-Path $version.FullName 'x64\signtool.exe'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                return $candidate
            }
        }
    }
    throw 'signtool.exe를 찾지 못했습니다. Windows SDK 설치와 Installed Roots 레지스트리를 확인하세요.'
}

function Invoke-CodeSign([string]$Path, [string]$Thumbprint, [string]$TimeUrl) {
    if ([string]::IsNullOrWhiteSpace($TimeUrl)) {
        throw '서명 시 -TimestampUrl 또는 EDUTOOLS_TIMESTAMP_URL이 필요합니다.'
    }
    $signTool = Find-SignTool
    & $signTool sign /fd SHA256 /sha1 $Thumbprint /tr $TimeUrl /td SHA256 $Path
    if ($LASTEXITCODE -ne 0) {
        throw "서명 실패: $Path"
    }
}

$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$windowsRoot = Join-Path $repositoryRoot 'windows'
$crateManifest = Join-Path $windowsRoot 'crates\edutools\Cargo.toml'
$versionMatch = Select-String -LiteralPath $crateManifest -Pattern '^version\s*=\s*"([^"]+)"\s*$' |
    Select-Object -First 1
if ($null -eq $versionMatch) {
    throw "패키지 버전을 읽지 못했습니다: $crateManifest"
}
$appVersion = $versionMatch.Matches[0].Groups[1].Value
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'

Push-Location $windowsRoot
try {
    & cargo build --release --locked --offline -p edutools
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo build --release 실패'
    }
}
finally {
    Pop-Location
}

$exePath = [IO.Path]::GetFullPath((Join-Path $windowsRoot 'target\release\edutools.exe'))
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw "release exe를 찾지 못했습니다: $exePath"
}
if (-not [string]::IsNullOrWhiteSpace($Sign)) {
    # 공인 인증서로 교체할 때도 -Sign에 새 인증서 지문만 전달한다.
    Invoke-CodeSign $exePath $Sign $TimestampUrl
}

$iscc = Find-Iscc
$issPath = Join-Path $PSScriptRoot 'edutools.iss'
$isccArguments = @("/DAppVersion=$appVersion", "/DExePath=$exePath")
$koreanMessages = Join-Path (Split-Path -Parent $iscc) 'Languages\Korean.isl'
if (Test-Path -LiteralPath $koreanMessages -PathType Leaf) {
    $isccArguments += '/DUseKorean=1'
    Write-Output "Inno Setup language: Korean ($koreanMessages)"
}
else {
    Write-Warning 'Korean.isl을 찾지 못해 영어 기본 UI와 한국어 핵심 메시지로 빌드합니다.'
}
$isccArguments += $issPath
& $iscc @isccArguments
if ($LASTEXITCODE -ne 0) {
    throw 'Inno Setup 컴파일 실패'
}

$installerPath = Join-Path $PSScriptRoot "output\EduTools-$appVersion-setup.exe"
if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw "설치기 산출물을 찾지 못했습니다: $installerPath"
}
if (-not [string]::IsNullOrWhiteSpace($Sign)) {
    Invoke-CodeSign $installerPath $Sign $TimestampUrl
}

foreach ($artifact in @($exePath, $installerPath)) {
    $hash = Get-FileHash -LiteralPath $artifact -Algorithm SHA256
    Write-Output ("Artifact: {0}" -f $artifact)
    Write-Output ("SHA-256: {0}" -f $hash.Hash)
}
