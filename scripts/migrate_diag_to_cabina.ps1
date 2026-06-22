# Migración masiva: `crate::bmo_core::diag::*` → `crate::cabina::*`.
# Solo cambiamos los nombres que cabina SÍ tiene:
#   info, warn, fault, trace, emit, event, panic_msg
# Las que cabina NO tiene (info_u64, telemetry, set_overlay_enabled, etc.) se
# quedan con `bmo_core::diag::` para revisión posterior.
$ErrorActionPreference = 'Stop'
$root = 'C:\Users\andre\Documents\FastOS\kernel\src'

# Funciones que cabina SÍ tiene (mismo nombre).
$keep = @('info', 'warn', 'fault', 'trace', 'emit', 'init',
         'is_ready', 'boot_ready', 'toggle_overlay', 'cycle_tab',
         'cycle_query', 'tick', 'overlay_enabled', 'current_tab',
         'current_query', 'active_query', 'active_query_name',
         'build_query', 'query_id_name')

# Patrón: \b evita que 'info' matchee dentro de 'info_u64'
$total = 0
$files = Get-ChildItem $root -Filter '*.rs' -Recurse
foreach ($f in $files) {
    $c = Get-Content $f.FullName -Raw
    $orig = $c
    foreach ($name in $keep) {
        $c = [regex]::Replace($c, "crate::bmo_core::diag::${name}\b", "crate::cabina::${name}")
        $c = [regex]::Replace($c, "\bbmo_core::diag::${name}\b",       "cabina::${name}")
    }
    # Renombres puntuales:
    $c = $c -replace 'crate::bmo_core::diag::Severity::Warn', 'crate::cabina::Severity::Warning'
    $c = $c -replace '\bbmo_core::diag::Severity::Warn',       'cabina::Severity::Warning'
    $c = $c -replace 'crate::bmo_core::diag::panic_event',   'crate::cabina::panic_msg'
    $c = $c -replace '\bbmo_core::diag::panic_event',          'cabina::panic_msg'

    if ($c -ne $orig) {
        Set-Content -LiteralPath $f.FullName -Value $c
        $total += 1
        Write-Host "  patched $($f.FullName.Substring($root.Length))"
    }
}
Write-Host ""
Write-Host "Total files patched: $total"
