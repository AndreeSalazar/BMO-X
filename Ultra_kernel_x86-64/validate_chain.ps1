param(
    [int]$MaxStageKB = 16,
    [int]$MaxKernelKB = 64
)

$root = $PSScriptRoot
$target = Join-Path $root "target"
$stage  = Join-Path $root "staging\EFI\BOOT"

function Pass { param($m) Write-Host ("  v " + $m) -ForegroundColor Green }
function Warn { param($m) Write-Host ("  ! " + $m) -ForegroundColor Yellow }
function Fail { param($m) Write-Host ("  x " + $m) -ForegroundColor Red }

if (-not (Test-Path $stage)) {
    Write-Host ""
    Write-Host "  Ultra_kernel/validate_chain.ps1" -ForegroundColor Magenta
    Write-Host "  ------------------------------" -ForegroundColor DarkGray
    Write-Host ""
    Fail "staging dir not found: $stage"
    Write-Host "    Run .\build.ps1 first." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "  Ultra_kernel/validate_chain.ps1" -ForegroundColor Magenta
Write-Host "  ------------------------------" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Validating that every faggin stage is small and in the right order." -ForegroundColor White
Write-Host ""

$expected = @(
    "BOOTX64.EFI",
    "s1_serial.bin", "s2_gdt.bin", "s3_idt.bin", "s4_cpuid.bin",
    "s5_control.bin", "s6_fpu.bin", "s7_tsc.bin", "s8_syscall.bin",
    "s9_paging.bin", "s10_heap.bin", "s11_acpi.bin", "s12_devices.bin",
    "kernel.bin"
)

$all_ok = $true
$total = 0
$base = 0x100000

Write-Host "  Step                                Address     Size      Status" -ForegroundColor White
Write-Host "  ----                                -------     ----      ------" -ForegroundColor DarkGray

foreach ($f in $expected) {
    $path = Join-Path $stage $f
    if (-not (Test-Path $path)) {
        Fail ("  {0,-35} {1,-12} {2,-10}  MISSING" -f $f, "", "")
        $all_ok = $false
        continue
    }
    $sz = (Get-Item $path).Length
    $szKB = [math]::Round($sz / 1024, 2)
    $total += $sz

    if ($f -eq "BOOTX64.EFI") {
        $addr = "0xFE000000"
        $status = "UEFI loader"
    } elseif ($f -eq "kernel.bin") {
        $addr = "0x400000"
        $ok = $szKB -le $MaxKernelKB
        $status = if ($ok) { "OK ($szKB KB)" } else { "TOO BIG (max $MaxKernelKB KB)" }
        if (-not $ok) { $all_ok = $false }
    } else {
        $addr = ("0x{0:X6}" -f $base)
        $ok = $szKB -le $MaxStageKB
        $status = if ($ok) { "OK ($szKB KB)" } else { "TOO BIG (max $MaxStageKB KB)" }
        if (-not $ok) { $all_ok = $false }
        $base += 0x10000
    }
    $status_color = if ($status.StartsWith("OK")) { "Green" } elseif ($status.StartsWith("TOO")) { "Red" } else { "Cyan" }
    Write-Host ("  {0,-35} {1,-12} {2,5} B   " -f $f, $addr, $sz) -NoNewline
    Write-Host $status -ForegroundColor $status_color
}

Write-Host "  ----                                -------     ----     ------" -ForegroundColor DarkGray
Write-Host ("  Total: {0} B ({1} KB)" -f $total, [math]::Round($total/1024, 2)) -ForegroundColor Yellow
Write-Host ""

# Chain order: each stage jumps via NEXT_ADDR constant to the next stage's load addr
Write-Host "  Chain order check:" -ForegroundColor White
$expected_addrs = @(
    "s1_serial", 0x110000,
    "s2_gdt",    0x120000,
    "s3_idt",    0x130000,
    "s4_cpuid",  0x140000,
    "s5_control",0x150000,
    "s6_fpu",    0x160000,
    "s7_tsc",    0x170000,
    "s8_syscall",0x180000,
    "s9_paging", 0x190000,
    "s10_heap",  0x1A0000,
    "s11_acpi",  0x1B0000
)

$source_dir = Join-Path $root "faggin"
for ($i = 0; $i -lt $expected_addrs.Count; $i += 2) {
    $s = $expected_addrs[$i]
    $nextAddr = $expected_addrs[$i + 1]
    $main = Join-Path $source_dir (Join-Path $s "src\main.rs")
    if (-not (Test-Path $main)) {
        Fail ("  {0,-12} main.rs missing" -f $s)
        $all_ok = $false
        continue
    }
    $content = Get-Content -LiteralPath $main -Raw
    $addrHex = "0x{0:X}" -f $nextAddr
    if ($content -match [regex]::Escape($addrHex)) {
        Pass ("  {0,-12} -> {1}" -f $s, $addrHex)
    } else {
        Fail ("  {0,-12} does NOT jmp to {1}" -f $s, $addrHex)
        $all_ok = $false
    }
}
# s12_devices jumps via the shared kernel stage index.
$main = Join-Path $source_dir "s12_devices\src\main.rs"
if (Test-Path $main) {
    $content = Get-Content -LiteralPath $main -Raw
    if ($content -match 'stage_entry\[KERNEL_STAGE_INDEX\]') {
        Pass ("  s12_devices  -> kernel (KERNEL_STAGE_INDEX)")
    } else {
        Fail ("  s12_devices  does NOT jmp to kernel")
        $all_ok = $false
    }
} else {
    Fail ("  s12_devices  main.rs missing")
    $all_ok = $false
}

Write-Host ""
if ($all_ok) {
    Write-Host "  === ALL CHECKS PASSED ===" -ForegroundColor Green
    exit 0
} else {
    Write-Host "  === SOME CHECKS FAILED ===" -ForegroundColor Red
    exit 1
}
