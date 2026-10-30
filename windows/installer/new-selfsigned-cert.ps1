[CmdletBinding()]
param(
    [string]$Subject = 'CN=EduTools Dev (self-signed)',
    [switch]$Trust
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$certificate = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -CertStoreLocation 'Cert:\CurrentUser\My' `
    -KeyAlgorithm RSA `
    -KeyLength 3072 `
    -HashAlgorithm SHA256

if ($Trust) {
    Write-Warning '이 옵션은 셀프 서명 인증서를 현재 사용자의 신뢰 루트와 신뢰할 수 있는 게시자 저장소에 추가합니다. 개발 PC에서만 사용하세요.'
    $temporaryCertificate = Join-Path ([IO.Path]::GetTempPath()) ("edutools-{0}.cer" -f [guid]::NewGuid().ToString('N'))
    try {
        Export-Certificate -Cert $certificate -FilePath $temporaryCertificate -Force | Out-Null
        Import-Certificate -FilePath $temporaryCertificate -CertStoreLocation 'Cert:\CurrentUser\Root' | Out-Null
        Import-Certificate -FilePath $temporaryCertificate -CertStoreLocation 'Cert:\CurrentUser\TrustedPublisher' | Out-Null
    }
    finally {
        if (Test-Path -LiteralPath $temporaryCertificate -PathType Leaf) {
            Remove-Item -LiteralPath $temporaryCertificate -Force
        }
    }
}

Write-Output ("Subject: {0}" -f $certificate.Subject)
Write-Output ("Thumbprint: {0}" -f $certificate.Thumbprint)
Write-Output 'Store: Cert:\CurrentUser\My'
