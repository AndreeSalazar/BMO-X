param(
    [int]$MaxStageKB = 4,    # each faggin stage must be <= this many KB
    [int]$MaxKernelKB = 64   # kernel must be <= this many KB
)

$root = $PSScriptRoot
$target = Join-Path $root "target"
$stage  = Join-Path $root "staging\EFI\BOOT"

function Pass { param($m) Write-Host "  ✓ $m" -ForegroundColor Green }
function Warn { param($m) Write-Host "  ! $m" -ForegroundColor Yellow }
function Fail { param($m) Write-Host "  ✗ $m" -ForegroundColor Red }

if (-not (Test-Path $stage)) {
    Write-Host ""
    Write-Host "  Ultra_kernel/validate_chain.ps1" -ForegroundColor Magenta
    Write-Host "  ──────────────────────────────" -ForegroundColor DarkGray
    Write-Host ""
    Fail "staging dir not found: $stage"
    Write-Host "    Run .\build.ps1 first." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "  Ultra_kernel/validate_chain.ps1" -ForegroundColor Magenta
Write-Host "  ──────────────────────────────" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Validating that every faggin stage is small and in the right order." -ForegroundColor White
Write-Host ""

# ── 1. Check that all 14 files exist (uefi_chain + 12 stages + kernel) ─
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
Write-Host "  ────────────────────────────────    ──────────   ────────  ──────" -ForegroundColor DarkGray

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

Write-Host "  ────────────────────────────────    ──────────   ────────  ──────" -ForegroundColor DarkGray
Write-Host ("  Total: {0} B ({1} KB)" -f $total, [math]::Round($total/1024, 2)) -ForegroundColor Yellow
Write-Host ""

# ── 2. Check the chain order matches the source code ───────────
Write-Host "  Chain order check:" -ForegroundColor White
$expected_order = @(
    "s1_serial", "s2_gdt", "s3_idt", "s4_cpuid",
    "s5_control", "s6_fpu", "s7_tsc", "s8_syscall",
    "s9_paging", "s10_heap", "s11_acpi", "s12_devices"
)

# Verify each stage's jmp target: stage N should jmp to stage N+1
# (except s12 which jumps to kernel@0x400000).
$source_dir = Join-Path $root "faggin"
for ($i = 0; $i -lt $expected_order.Count; $i++) {
    $s = $expected_order[$i]
    $main = Join-Path $source_dir (Join-Path $s "src\main.rs")
    if (-not (Test-Path $main)) {
        Fail ("  {0,-12} main.rs missing" -f $s)
        $all_ok = $false
        continue
    }
    $content = Get-Content -LiteralPath $main -Raw

    if ($i -lt $expected_order.Count - 1) {
        $next = $expected_order[$i + 1]
        if ($content -match [regex]::Escape($next)) {
            Pass ("  {0,-12} -> {1}" -f $s, $next)
        } else {
            Fail ("  {0,-12} does NOT jmp to {1}" -f $s, $next)
            $all_ok = $false
        }
    } else {
        # last stage jumps to kernel
        if ($content -match 'stage_entry\[0\]' -or $content -match '0x400000' -or $content -match 'kernel') {
            Pass ("  {0,-12} -> kernel@0x400000" -f $s)
        } else {
            Fail ("  {0,-12} does NOT jmp to kernel" -f $s)
            $all_ok = $false
        }
    }
}

Write-Host ""
if ($all_ok) {
    Write-Host "  ═══ ALL CHECKS PASSED ═══" -ForegroundColor Green
    exit 0
} else {
    Write-Host "  ═══ SOME CHECKS FAILED ═══" -ForegroundColor Red
    exit 1
}
