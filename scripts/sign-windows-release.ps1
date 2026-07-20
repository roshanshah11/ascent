$ErrorActionPreference = "Stop"
$payload = $args[0]
$signed = Join-Path $payload "signed"
New-Item -ItemType Directory -Force $signed | Out-Null
$pfx = Join-Path $env:RUNNER_TEMP "ascent.pfx"
[IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX))
$installer = Get-ChildItem $payload -Recurse -Include *.exe,*.msi | Select-Object -First 1
if (-not $installer) { throw "no Windows installer in unsigned payload" }
Copy-Item $installer.FullName $signed
$target = Join-Path $signed $installer.Name
& signtool sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD $target
& signtool verify /pa /all $target
& cargo tauri signer sign -k $env:TAURI_SIGNING_PRIVATE_KEY -p $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD $target
Get-ChildItem (Join-Path $payload "release-evidence") -File | Copy-Item -Destination $signed
Get-ChildItem $payload -Recurse -File -Filter *.cdx.json | Copy-Item -Destination $signed
Get-ChildItem $signed -File | Where-Object { $_.Name -notlike "SHA256SUMS*" } | Get-FileHash -Algorithm SHA256 | ForEach-Object { "$($_.Hash.ToLower())  $($_.Path | Split-Path -Leaf)" } | Set-Content (Join-Path $signed "SHA256SUMS.windows")
