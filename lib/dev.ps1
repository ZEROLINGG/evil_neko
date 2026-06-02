#!pwsh
param (
    [string]$Target = "x86_64-pc-windows-gnullvm"
)
$env:RUST_TEST_NOCAPTURE=1
$cmd = "cargo test --target $Target"
Write-Output $cmd
iex $cmd