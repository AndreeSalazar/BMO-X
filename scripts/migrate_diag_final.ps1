# Migración final: bmo_core::diag::* → cabina::*
# Cabina ya tiene la API completa (info, warn, fault, info_u64, etc.)
# Eliminar el shim bmo_core::diag.rs al final.

$ErrorActionPreference = 'Stop'
$root = 'C:\Users\andre\Documents\FastOS\kernel\src'

# Archivos a migrar (todos los que importan bmo_core::diag)
$files = @(
    'bmo_core\bef\loader\mod.rs',
    'bmo_core\bef\loader\native.rs',
    'bmo_core\bef\loader\runtime.rs',
    'bmo_core\bmo_api\syscall.rs',
    'bmo_core\desktop\commands.rs',
    'bmo_core\desktop\input.rs',
    'bmo_core\desktop\mod.rs',
    'bmo_core\desktop\render.rs',
    'bmo_core\desktop\welcome.rs',
    'ring0\arch\idt.rs',
    'ring0\arch\syscall.rs',
    'ring0\boot\log.rs',
    'ring0\log.rs',
    'ring0\mem\virt.rs',
    'ring0\panic.rs',
    'ring0\proc\mod.rs',
    'ring0\proc\process.rs',
    'ring0\proc\user_init.rs'
)

$patched = 0
foreach ($rel in $files) {
    $f = Join-Path $root $rel
    if (-not (Test-Path $f)) { continue }
    $c = Get-Content $f -Raw
    $orig = $c
    # Reemplazo simple: el path bmo_core::diag:: se va a cabina::
    $c = $c -replace 'bmo_core::diag::', 'cabina::'
    if ($c -ne $orig) {
        Set-Content -LiteralPath $f -Value $c
        $patched += 1
        Write-Host "  patched: $rel"
    }
}

Write-Host ""
Write-Host "Total files patched: $patched"
