# ============================================================================
# FastOS — ULTRA PIPELINE (Auto-Elevación + Formateo UEFI)
# ============================================================================
param([int]$DiskNumber = -1)

# --- 0. AUTO-ELEVACIÓN A ADMINISTRADOR ---
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "[!] Elevando privilegios para acceso a disco..." -ForegroundColor Yellow
    Start-Process powershell.exe -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
    exit
}

$Root = $PSScriptRoot
Set-Location $Root

Write-Host "`n--- FastOS v0.6.0 | Ring 0 Toolchain ---" -ForegroundColor Cyan
Write-Host "Target: GA106 (RTX 3060) | Zen 3" -ForegroundColor Cyan

# --- 1. VALIDACIÓN DE BINARIOS ---
$Files = @("BOOTX64.EFI", "kernel.elf", "gsp_ga10x.bin")
Write-Host "`n[1/3] Verificando archivos de compilación..." -ForegroundColor White
foreach ($f in $Files) {
    if (Test-Path "$Root\$f") {
        Write-Host "  [OK] $f" -ForegroundColor Green
    } else {
        Write-Host "  [ERROR] Falta $f. Compila primero con Cargo." -ForegroundColor Red
        exit
    }
}

# --- 2. DETECCIÓN DE USB ---
if ($DiskNumber -eq -1) {
    $USB = Get-Disk | Where-Object { $_.BusType -eq 'USB' -and $_.OperationalStatus -eq 'Online' }
    if (-not $USB) {
        Write-Host "[!] No se detectó el USB Kingston. Conéctalo e intenta de nuevo." -ForegroundColor Red
        exit
    }
    $DiskNumber = $USB[0].Number
}

$TargetDisk = Get-Disk -Number $DiskNumber
Write-Host "`n[2/3] DESTINO: $($TargetDisk.FriendlyName) (Disco $DiskNumber)" -ForegroundColor Yellow
$Confirm = Read-Host "ADVERTENCIA: Se BORRARÁ el disco $DiskNumber. ¿Continuar? (S/N)"
if ($Confirm -ne "S") { exit }

# --- 3. PREPARACIÓN UEFI (Limpieza y Formateo) ---
Write-Host "`n[3/3] Flasheando FastOS al USB..." -ForegroundColor White
try {
    # Limpiar disco y crear partición FAT32 (Requisito UEFI)
    Clear-Disk -Number $DiskNumber -RemoveData -Confirm:$false
    Initialize-Disk -Number $DiskNumber -PartitionStyle GPT
    $Part = New-Partition -DiskNumber $DiskNumber -UseMaximumSize -AssignDriveLetter
    Format-Volume -DriveLetter $Part.DriveLetter -FileSystem FAT32 -NewFileSystemLabel "FASTOS_BOOT" -Confirm:$false

    $USBPath = $Part.DriveLetter + ":"
    
    # Crear estructura de carpetas UEFI
    New-Item -ItemType Directory -Path "$USBPath\EFI\BOOT" -Force | Out-Null
    
    # Copiar archivos críticos
    Copy-Item "$Root\BOOTX64.EFI" -Destination "$USBPath\EFI\BOOT\BOOTX64.EFI"
    Copy-Item "$Root\kernel.elf" -Destination "$USBPath\EFI\BOOT\kernel.elf"
    Copy-Item "$Root\gsp_ga10x.bin" -Destination "$USBPath\gsp_ga10x.bin"

    Write-Host "`n[+++] ¡ÉXITO! FastOS está listo en $USBPath" -ForegroundColor Green
    Write-Host "Instrucciones: Reinicia, pulsa F11 (MSI) y elige '$($TargetDisk.FriendlyName)'" -ForegroundColor Cyan
} catch {
    Write-Host "`n[!] Error crítico: $($_.Exception.Message)" -ForegroundColor Red
}