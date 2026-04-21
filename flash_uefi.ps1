# ============================================================================
# FastOS — Flash to USB (UEFI Native, auto-elevates to Admin)
# ============================================================================
# Usage:  .\flash_uefi.ps1
#         .\flash_uefi.ps1 -DiskNumber 2
#
# If no DiskNumber given, auto-detects USB drives and asks you to pick.
# Formats USB as GPT + FAT32 (full size ESP) and copies EFI files.
# Auto-elevates to Administrator.
# ============================================================================

param(
    [int]$DiskNumber = -1
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# ── Auto-elevate to Administrator ────────────────────────────────────────────
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (!$isAdmin) {
    Write-Host "Elevating to Administrator..." -ForegroundColor Yellow
    $argList = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    if ($DiskNumber -ge 0) { $argList += " -DiskNumber $DiskNumber" }
    Start-Process powershell.exe -Verb RunAs -ArgumentList $argList -Wait
    exit 0
}

# ── Auto-detect USB if no DiskNumber given ───────────────────────────────────
if ($DiskNumber -lt 0) {
    Write-Host ""
    Write-Host "  Buscando discos USB..." -ForegroundColor Cyan
    $usbDisks = @(Get-Disk | Where-Object { $_.BusType -eq "USB" })
    
    if ($usbDisks.Count -eq 0) {
        Write-Host "  ERROR: No se encontro ningun USB conectado." -ForegroundColor Red
        Write-Host "  Conecta tu USB y vuelve a ejecutar." -ForegroundColor Yellow
        Read-Host "  Presiona Enter para salir"
        exit 1
    }
    
    Write-Host ""
    Write-Host "  Discos USB encontrados:" -ForegroundColor Green
    Write-Host ""
    foreach ($d in $usbDisks) {
        $sizeGB = [math]::Round($d.Size / 1GB, 1)
        Write-Host "    [$($d.Number)]  $($d.FriendlyName)  ($sizeGB GB)" -ForegroundColor White
    }
    Write-Host ""
    
    if ($usbDisks.Count -eq 1) {
        $DiskNumber = $usbDisks[0].Number
        Write-Host "  Solo hay 1 USB -> Disco $DiskNumber seleccionado automaticamente." -ForegroundColor Green
    } else {
        $sel = Read-Host "  Escribe el numero del disco USB"
        $DiskNumber = [int]$sel
    }
    Write-Host ""
}

# ── Find EFI files ───────────────────────────────────────────────────────────
$efiPath = "$Root\BOOTX64.EFI"
$kernelPath = "$Root\kernel.elf"

if (!(Test-Path $efiPath)) {
    Write-Host "ERROR: BOOTX64.EFI no encontrado en $Root" -ForegroundColor Red
    Write-Host "  Ejecuta build_uefi.ps1 primero." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 1
}

if (!(Test-Path $kernelPath)) {
    Write-Host "ERROR: kernel.elf no encontrado en $Root" -ForegroundColor Red
    Write-Host "  Ejecuta build_uefi.ps1 primero." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 1
}

$efiSize = (Get-Item $efiPath).Length
$kernelSize = (Get-Item $kernelPath).Length

# ── Validate target ─────────────────────────────────────────────────────────
$disk = Get-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
if (!$disk) {
    Write-Host "ERROR: Disk $DiskNumber not found" -ForegroundColor Red
    exit 1
}

# Safety: refuse system disk
$sysDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
if ($DiskNumber -eq $sysDisk) {
    Write-Host "ERROR: Disco $DiskNumber es el DISCO DEL SISTEMA. Cancelado." -ForegroundColor Red
    Read-Host "  Presiona Enter para salir"
    exit 1
}

$diskSizeGB = [int]($disk.Size / 1GB)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  FastOS USB Flash (UEFI Native)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Bootloader: BOOTX64.EFI ($([math]::Round($efiSize/1024))KB)" -ForegroundColor White
Write-Host "  Kernel:    kernel.elf ($([math]::Round($kernelSize/1024))KB)" -ForegroundColor White
Write-Host "  Disco  : [$DiskNumber] $($disk.FriendlyName)" -ForegroundColor White
Write-Host "  Tamano : $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
Write-Host "  Modo   : GPT + FAT32 (Full Size ESP)" -ForegroundColor White
Write-Host ""
Write-Host "  ESTO VA A FORMATEAR EL DISCO $DiskNumber" -ForegroundColor Red
Write-Host ""
$confirm = Read-Host "  Escribe FLASH para continuar"
if ($confirm -ne "FLASH") {
    Write-Host "  Cancelado." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 0
}

# ── Clear disk and create GPT + ESP (full size) ────────────────────────────
Write-Host ""
Write-Host "[1/3] Formateando USB como GPT + FAT32 (full ESP)..." -ForegroundColor Cyan

# Clear partition table
Write-Host "      Limpiando particiones..." -ForegroundColor DarkGray
$disk | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false

# Convert to GPT
Write-Host "      Convirtiendo a GPT..." -ForegroundColor DarkGray
$disk | Set-Disk -PartitionStyle GPT

# Create EFI System Partition using FULL USB size for maximum compatibility
Write-Host "      Creando EFI System Partition (tamano completo)..." -ForegroundColor DarkGray
$partition = $disk | New-Partition -UseMaximumSize -GptType '{C12A7328-F81F-11D2-BA4B-00A0C93EC93B}' -AssignDriveLetter

# Format as FAT32
Write-Host "      Formateando como FAT32..." -ForegroundColor DarkGray
$partition | Format-Volume -FileSystem FAT32 -NewFileSystemLabel "FastOS-ESP" -Confirm:$false

$driveLetter = $partition.DriveLetter
Write-Host "      Unidad: ${driveLetter}:" -ForegroundColor DarkGray

Write-Host "[1/3] Formato completado" -ForegroundColor Green

# ── Copy EFI files ──────────────────────────────────────────────────────────
Write-Host "[2/3] Copiando archivos EFI..." -ForegroundColor Cyan

# Create EFI\BOOT\ directory
$efiBootPath = "${driveLetter}:\EFI\BOOT"
if (!(Test-Path $efiBootPath)) {
    New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null
}

# Copy BOOTX64.EFI
Write-Host "      Copiando BOOTX64.EFI a EFI\BOOT\BOOTX64.EFI..." -ForegroundColor DarkGray
Copy-Item $efiPath "$efiBootPath\BOOTX64.EFI" -Force

# Copy kernel.elf to root AND to EFI\BOOT (bootloader loads from \EFI\BOOT\kernel.elf)
Write-Host "      Copiando kernel.elf al root y a EFI\BOOT\..." -ForegroundColor DarkGray
Copy-Item $kernelPath "${driveLetter}:\kernel.elf" -Force
Copy-Item $kernelPath "$efiBootPath\kernel.elf" -Force

# Copy GSP firmware if available
$gspPath = "$Root\gsp_ga10x.bin"
if (Test-Path $gspPath) {
    Write-Host "      Copiando gsp_ga10x.bin al root (GPU firmware)..." -ForegroundColor DarkGray
    Copy-Item $gspPath "${driveLetter}:\gsp_ga10x.bin" -Force
}

Write-Host "[2/3] Archivos copiados" -ForegroundColor Green

# ── Verify ──────────────────────────────────────────────────────────────────
Write-Host "[3/3] Verificando..." -ForegroundColor Cyan

if (!(Test-Path "$efiBootPath\BOOTX64.EFI")) {
    Write-Host "      ERROR: BOOTX64.EFI no se copio!" -ForegroundColor Red
    Read-Host "  Presiona Enter para salir"
    exit 1
}

if (!(Test-Path "${driveLetter}:\kernel.elf")) {
    Write-Host "      ERROR: kernel.elf no se copio!" -ForegroundColor Red
    Read-Host "  Presiona Enter para salir"
    exit 1
}

$copiedEfiSize = (Get-Item "$efiBootPath\BOOTX64.EFI").Length
$copiedKernelSize = (Get-Item "${driveLetter}:\kernel.elf").Length

Write-Host "      BOOTX64.EFI: $copiedEfiSize bytes" -ForegroundColor DarkGray
Write-Host "      kernel.elf: $copiedKernelSize bytes" -ForegroundColor DarkGray

# Verify GSP firmware if it was copied
if (Test-Path "${driveLetter}:\gsp_ga10x.bin") {
    $copiedGspSize = (Get-Item "${driveLetter}:\gsp_ga10x.bin").Length
    $gspOrigSize = (Get-Item "$Root\gsp_ga10x.bin").Length
    Write-Host "      gsp_ga10x.bin: $copiedGspSize bytes" -ForegroundColor DarkGray
}

if ($copiedEfiSize -eq $efiSize -and $copiedKernelSize -eq $kernelSize) {
    Write-Host "      VERIFICADO OK: Archivos copiados correctamente" -ForegroundColor Green
} else {
    Write-Host "      WARNING: Tamanos no coinciden!" -ForegroundColor Yellow
}

Write-Host "[3/3] Verificacion completa" -ForegroundColor Green

# ── Done ─────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  FLASH COMPLETO!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Ahora:" -ForegroundColor Yellow
Write-Host "    1. Reinicia el PC" -ForegroundColor White
Write-Host "    2. Entra al BIOS (DEL o F2)" -ForegroundColor White
Write-Host "    3. Deshabilita CSM (configura UEFI Only)" -ForegroundColor White
Write-Host "    4. Deshabilita Secure Boot" -ForegroundColor White
Write-Host "    5. Boot -> UEFI -> USB" -ForegroundColor White
Write-Host "    6. Guardar y reiniciar" -ForegroundColor White
Write-Host ""
Read-Host "  Presiona Enter para cerrar"
