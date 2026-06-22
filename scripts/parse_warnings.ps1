# Limpia imports/variables no usados.
# Estrategia:
# 1. Parsear cada warning `unused import: <NAME>` con su file:line.
# 2. Leer el archivo, eliminar la línea completa del import o el item del bloque use.
# 3. Guardar y reportar.

$ErrorActionPreference = 'Stop'
$root = 'C:\Users\andre\Documents\FastOS\kernel\src'

# Compilar y parsear warnings.
$log = cargo build --release --target x86_64-unknown-none 2>&1
$warnings = $log | Select-String -Pattern "^warning: unused (imports?|variable)" |
    Where-Object { $_.Line -notmatch "tests\.rs" }  # dejar tests intactos

# Construir lista de (file, line, type, name).
$jobs = @()
foreach ($w in $warnings) {
    $msg = $w.Line
    # Extraer path de la línea siguiente (formato: ` --> path:line:col`).
    $ctx = ($log | Select-String -Pattern "src.*\.rs:\d+" -SimpleMatch) 
    $next = $log[($log.IndexOf($w) + 1)..($log.IndexOf($w) + 5)] -join "`n"
    if ($next -match "src\\(\S+\.rs):(\d+):(\d+)") {
        $rel = $matches[1] -replace '\\', '/'
        $line = [int]$matches[2]
        $jobs += [pscustomobject]@{ File = $rel; Line = $line; Msg = $msg }
    }
}

Write-Host "Total warnings a procesar: $($jobs.Count)"
$jobs | Group-Object File | ForEach-Object {
    Write-Host "  $($_.Name): $($_.Count) warnings"
}
