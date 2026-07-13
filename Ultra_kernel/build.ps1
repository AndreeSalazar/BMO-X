param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$BuildOnly,
    [string]$Drive = "S",
    [switch]$Yes
)

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }

function Step   { param($m) Write-Host "  => $m" -ForegroundColor Cyan }
function Fail   { param($m) Write-Host "  [X] $m" -ForegroundColor Red; exit 1 }

$target = Join-Path $root "target"
$stage  = Join-Path $root "staging\EFI\BOOT"

if ($Clean) {
    Step "Cleaning"
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    if (Test-Path $stage)  { Remove-Item $stage  -Recurse -Force }
}

Write-Host ""
Write-Host "  ═══ BMO Ultra Kernel v2 Build (5-layer UEFI chain) ═══" -ForegroundColor Magenta
Write-Host ""

# ── Build uefi_chain (5 UEFI layers in one EFI binary) ──
Step "Building uefi_chain (5 UEFI layers)..."
Push-Location $root
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $target 2>&1
    $out | ForEach-Object {
        if ($_ -match "Compiling|Finished|error") { Write-Host "    [uefi_chain] $_" -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail "uefi_chain build failed" }
} finally { Pop-Location }

# ── Build stage1-3 + kernel (bare metal) ──
$stages = @("stage1_arch", "stage2_mm", "stage3_dev", "kernel")
foreach ($s in $stages) {
    Step "Building $s..."
    $stageDir = Join-Path $root $s
    Push-Location $stageDir
    try {
        $out = cargo build --release --target x86_64-unknown-none --target-dir (Join-Path $target $s) 2>&1
        $out | ForEach-Object {
            if ($_ -match "Compiling|Finished|error") { Write-Host "    [$s] $_" -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail "$s build failed" }
    } finally { Pop-Location }
}

# ── Validate outputs ──
Step "Validating outputs"
$uefi_chain = Join-Path $target "x86_64-unknown-uefi\release\uefi_chain.efi"
$stage1 = Join-Path $target "stage1_arch\x86_64-unknown-none\release\stage1-arch"
$stage2 = Join-Path $target "stage2_mm\x86_64-unknown-none\release\stage2-mm"
$stage3 = Join-Path $target "stage3_dev\x86_64-unknown-none\release\stage3-dev"
$kernel = Join-Path $target "kernel\x86_64-unknown-none\release\bmo-kernel-v2"

$all_binaries = @($uefi_chain, $stage1, $stage2, $stage3, $kernel)
foreach ($f in $all_binaries) {
    if (-not (Test-Path $f)) { Fail "Not found: $f" }
    $sz = (Get-Item $f).Length
    Write-Host "    $(Split-Path $f -Leaf): $([math]::Round($sz/1024,1)) KB" -ForegroundColor White
}

# ── Stage ──
Step "Staging to $stage"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage "modules") -Force | Out-Null

Copy-Item $uefi_chain (Join-Path $stage "BOOTX64.EFI")
$llvmObjcopy = Get-ChildItem -Path "C:\Users\andre\.rustup" -Filter "llvm-objcopy.exe" -Recurse | Select-Object -First 1 -ExpandProperty FullName
if ($llvmObjcopy) {
    & $llvmObjcopy -O binary $stage1 (Join-Path $stage "stage1.bin")
    & $llvmObjcopy -O binary $stage2 (Join-Path $stage "stage2.bin")
    & $llvmObjcopy -O binary $stage3 (Join-Path $stage "stage3.bin")
    & $llvmObjcopy -O binary $kernel (Join-Path $stage "kernel.bin")
} else {
    Copy-Item $stage1 (Join-Path $stage "stage1.bin")
    Copy-Item $stage2 (Join-Path $stage "stage2.bin")
    Copy-Item $stage3 (Join-Path $stage "stage3.bin")
    Copy-Item $kernel (Join-Path $stage "kernel.bin")
}

Write-Host ""
Write-Host "  ═══ BUILD COMPLETE ═══" -ForegroundColor Green
Write-Host "  Staged to: $stage" -ForegroundColor White
Write-Host ""

if ($BuildOnly) { exit 0 }

# ── Flash ──
if ($Flash) {
    $targetLetter = $Drive.TrimEnd([char]':',[char]'\').ToUpper()
    $targetRoot = "${targetLetter}:\"
    if (-not (Test-Path $targetRoot)) { Fail "Drive $targetLetter not found" }

    if (-not $Yes) {
        $c = Read-Host "  Flash to ${targetLetter}:? (YES)"
        if ($c -ne "YES") { Write-Host "  Aborted."; exit 0 }
    }

    Step "Flashing to $targetLetter`:\EFI\BOOT"
    $efiDest = Join-Path $targetRoot "EFI\BOOT"
    New-Item -ItemType Directory -Path $efiDest -Force | Out-Null
    Copy-Item (Join-Path $stage "BOOTX64.EFI") -Destination (Join-Path $efiDest "BOOTX64.EFI") -Force
    Copy-Item (Join-Path $stage "stage1.bin")   -Destination (Join-Path $efiDest "stage1.bin") -Force
    Copy-Item (Join-Path $stage "stage2.bin")   -Destination (Join-Path $efiDest "stage2.bin") -Force
    Copy-Item (Join-Path $stage "stage3.bin")   -Destination (Join-Path $efiDest "stage3.bin") -Force
    Copy-Item (Join-Path $stage "kernel.bin")   -Destination (Join-Path $efiDest "kernel.bin") -Force

    try {
        foreach ($f in @("BOOTX64.EFI", "stage1.bin", "stage2.bin", "stage3.bin", "kernel.bin")) {
            $fs = [System.IO.File]::Open("$targetRoot\EFI\BOOT\$f", 'Open', 'Write')
            $fs.Flush(1); $fs.Close()
        }
    } catch { Write-Host "  [WARN] Flush failed: $_" -ForegroundColor Yellow }

    Write-Host ""
    Write-Host "  ═══ FLASH OK ═══" -ForegroundColor Green
    Write-Host "  Reboot from SSD to test." -ForegroundColor White
    Write-Host ""
}
