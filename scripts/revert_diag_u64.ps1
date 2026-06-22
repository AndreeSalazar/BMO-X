# Revierte: cabina::X_u64, cabina::X_dispatch, etc. → bmo_core::diag::X_u64, etc.
# Solo aplica a funciones que NO existen en cabina y que el script anterior
# migró por error (por el bug del \b).
$ErrorActionPreference = 'Stop'
$root = 'C:\Users\andre\Documents\FastOS\kernel\src'

# Funciones que NO están en cabina — deben quedarse en bmo_core::diag::
$revert = @('info_u64', 'warn_u64', 'fault_u64', 'trace_u64',
            'event', 'event_u64', 'set_overlay_enabled', 'is_overlay_enabled',
            'paint_overlay', 'mark_boot_ready', 'tick_refresh',
            'read_cr3_into_serial', 'persistent_target_path',
            'persistent_pending_bytes', 'persistent_dropped_bytes',
            'copy_persistent_pending', 'ack_persistent_bytes',
            'telemetry', 'Severity')

# Pero 'Severity' sí está en cabina (re-export). Lo dejo.
$revert = @('info_u64', 'warn_u64', 'fault_u64', 'trace_u64',
            'event', 'event_u64', 'set_overlay_enabled', 'is_overlay_enabled',
            'paint_overlay', 'mark_boot_ready', 'tick_refresh',
            'read_cr3_into_serial', 'persistent_target_path',
            'persistent_pending_bytes', 'persistent_dropped_bytes',
            'copy_persistent_pending', 'ack_persistent_bytes',
            'telemetry')

$total = 0
$files = Get-ChildItem $root -Filter '*.rs' -Recurse
foreach ($f in $files) {
    $c = Get-Content $f.FullName -Raw
    $orig = $c
    foreach ($name in $revert) {
        $c = [regex]::Replace($c, "crate::cabina::${name}\b", "crate::bmo_core::diag::${name}")
        $c = [regex]::Replace($c, "\bcabina::${name}\b",       "bmo_core::diag::${name}")
    }
    if ($c -ne $orig) {
        Set-Content -LiteralPath $f.FullName -Value $c
        $total += 1
        Write-Host "  reverted $($f.FullName.Substring($root.Length))"
    }
}
Write-Host ""
Write-Host "Total files reverted: $total"
