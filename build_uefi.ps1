# ============================================================================
# FastOS -- Build + Flash USB (todo en uno)
# ============================================================================
# Compila bootloader + kernel, prepara USB_boot/, y flashea al USB.
#
# Uso:
#   .\build_uefi.ps1                  # Build + flash (auto-detecta USB)
#   .\build_uefi.ps1 -DiskNumber 3    # Build + flash al disco 3
#   .\build_uefi.ps1 -BuildOnly       # Solo compilar, no flashear
#   .\build_uefi.ps1 -FlashOnly       # Solo flashear (ya compilado)
#   .\build_uefi.ps1 -Clean           # Limpiar artefactos
#
# Target: Ryzen 5 5600X + RTX 3060 12G (GA106) | UEFI Native
# ============================================================================

param(
    [int]$DiskNumber = -1,
    [switch]$BuildOnly,
    [switch]$FlashOnly,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# -- Banner ------------------------------------------------------------------
Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS -- Build and Flash (todo en uno)" -ForegroundColor Cyan
Write-Host "  Target: Ryzen 5 5600X + RTX 3060 12G | UEFI Native" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# -- Clean -------------------------------------------------------------------
if ($Clean) {
    Write-Host "[CLEAN] Eliminando artefactos..." -ForegroundColor Yellow
    Remove-Item "$Root\bootloader\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel.elf" -ErrorAction SilentlyContinue
    Remove-Item "$Root\BOOTX64.EFI" -ErrorAction SilentlyContinue
    Remove-Item "$Root\USB_boot" -Recurse -ErrorAction SilentlyContinue
    Write-Host "[CLEAN] Listo." -ForegroundColor Green
    return
}

# ============================================================================
# FASE 1: BUILD (saltar si -FlashOnly)
# ============================================================================

$efiSize = 0
$kernelSize = 0

if (!$FlashOnly) {

    # -- Step 1: Build UEFI Bootloader ----------------------------------------
    Write-Host "[1/4] Compilando UEFI Bootloader..." -ForegroundColor Cyan

    Push-Location "$Root\bootloader"
    $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $cargoOutput = & rustup run nightly cargo build --release 2>&1
    $cargoExit = $LASTEXITCODE
    $ErrorActionPreference = $savedEAP

    $cargoOutput | ForEach-Object {
        $line = $_.ToString()
        if ($line -match "error\[") { Write-Host "      $line" -ForegroundColor Red }
        elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
    }

    if ($cargoExit -ne 0) {
        $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
        Pop-Location; throw "Bootloader: fallo la compilacion"
    }

    $efiPath = Get-ChildItem "$Root\bootloader\target\x86_64-unknown-uefi\release\fastos-bootloader*.efi" -File |
               Select-Object -First 1 -ExpandProperty FullName
    if (!$efiPath) { Pop-Location; throw "No se encontro BOOTX64.EFI" }

    Copy-Item $efiPath "$Root\BOOTX64.EFI" -Force
    $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
    Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024, 1)) KB" -ForegroundColor DarkGray
    Pop-Location
    Write-Host "[1/4] Bootloader OK" -ForegroundColor Green

    # -- Step 2: Build Kernel -------------------------------------------------
    Write-Host "[2/4] Compilando Kernel (ELF)..." -ForegroundColor Cyan

    Push-Location "$Root\kernel"
    $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $cargoOutput = & rustup run nightly cargo build --release 2>&1
    $cargoExit = $LASTEXITCODE
    $ErrorActionPreference = $savedEAP

    $cargoOutput | ForEach-Object {
        $line = $_.ToString()
        if ($line -match "error\[") { Write-Host "      $line" -ForegroundColor Red }
        elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
    }

    if ($cargoExit -ne 0) {
        $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
        Pop-Location; throw "Kernel: fallo la compilacion"
    }

    $elfPath = "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel"
    if (!(Test-Path $elfPath)) {
        $elfPath = Get-ChildItem "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel*" -File |
                   Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } |
                   Select-Object -First 1 -ExpandProperty FullName
    }
    if (!$elfPath -or !(Test-Path $elfPath)) { Pop-Location; throw "No se encontro kernel.elf" }

    Copy-Item $elfPath "$Root\kernel.elf" -Force
    $kernelSize = (Get-Item "$Root\kernel.elf").Length
    Write-Host "      kernel.elf: $([math]::Round($kernelSize/1024, 1)) KB" -ForegroundColor DarkGray
    Pop-Location
    Write-Host "[2/4] Kernel OK" -ForegroundColor Green

    # -- Step 3: Preparar USB_boot/ -------------------------------------------
    Write-Host "[3/4] Preparando USB_boot/..." -ForegroundColor Cyan

    $usbDir = "$Root\USB_boot"
    $efiBootDir = "$usbDir\EFI\BOOT"
    New-Item -Path $efiBootDir -ItemType Directory -Force | Out-Null

    Copy-Item "$Root\BOOTX64.EFI" "$usbDir\BOOTX64.EFI" -Force
    Copy-Item "$Root\BOOTX64.EFI" "$efiBootDir\BOOTX64.EFI" -Force
    Copy-Item "$Root\kernel.elf"  "$usbDir\kernel.elf" -Force
    Copy-Item "$Root\kernel.elf"  "$efiBootDir\kernel.elf" -Force

    $gspPath = "$Root\gsp_ga10x.bin"
    if (Test-Path $gspPath) {
        Copy-Item $gspPath "$usbDir\gsp_ga10x.bin" -Force
        $gspSize = (Get-Item $gspPath).Length
        Write-Host "      gsp_ga10x.bin: $([math]::Round($gspSize/1MB, 1)) MB" -ForegroundColor DarkGray
    } else {
        Write-Host "      AVISO: gsp_ga10x.bin no encontrado" -ForegroundColor Yellow
    }

    $fwDir = "$Root\firmware"
    if (Test-Path $fwDir) {
        $fwUsbDir = "$usbDir\firmware"
        New-Item -Path $fwUsbDir -ItemType Directory -Force | Out-Null
        foreach ($f in @("bootloader-535.113.01.bin", "booter_load-535.113.01.bin", "vbios_rtx3060.rom")) {
            $src = Join-Path $fwDir $f
            if (Test-Path $src) {
                Copy-Item $src (Join-Path $fwUsbDir $f) -Force
                Write-Host "      $f : $((Get-Item $src).Length) bytes" -ForegroundColor DarkGray
            } else {
                Write-Host "      AVISO: $f no encontrado" -ForegroundColor Yellow
            }
        }

        # -- FWSEC: pasar ROM completo al kernel (NVGI flat blob GA106) --------
        $vbiosPath = Join-Path $fwDir "vbios_rtx3060.rom"
        if (!(Test-Path $vbiosPath)) {
            $vbiosPath = Join-Path $usbDir "firmware\vbios_rtx3060.rom"
        }
        if (Test-Path $vbiosPath) {
            $romSize = (Get-Item $vbiosPath).Length
            Write-Host "      vbios_rtx3060.rom: $romSize bytes (FWSEC blob listo)" -ForegroundColor Green
            $offBytes = [BitConverter]::GetBytes([uint64]0)
            [System.IO.File]::WriteAllBytes((Join-Path $fwUsbDir "fwsec_offset.bin"), $offBytes)
            Write-Host "      fwsec_offset.bin: offset=0 (ROM completo)" -ForegroundColor Green
        } else {
            Write-Host "      AVISO: vbios_rtx3060.rom no encontrado -- FWSEC saltado" -ForegroundColor Yellow
        }
    }

    Write-Host "[3/4] USB_boot/ listo" -ForegroundColor Green
} else {
    if (!(Test-Path "$Root\BOOTX64.EFI") -or !(Test-Path "$Root\kernel.elf")) {
        throw "No se encontraron BOOTX64.EFI o kernel.elf -- ejecuta sin -FlashOnly primero"
    }
    $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
    $kernelSize = (Get-Item "$Root\kernel.elf").Length
    Write-Host "[BUILD] Saltando compilacion (-FlashOnly)" -ForegroundColor Yellow
    Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024, 1)) KB" -ForegroundColor DarkGray
    Write-Host "      kernel.elf:  $([math]::Round($kernelSize/1024, 1)) KB" -ForegroundColor DarkGray
}

# ============================================================================
# FASE 2: FLASH USB (saltar si -BuildOnly)
# ============================================================================

if ($BuildOnly) {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host "  BUILD COMPLETO (sin flash)" -ForegroundColor Green
    Write-Host "  Para flashear: .\build_uefi.ps1 -FlashOnly" -ForegroundColor Green
    Write-Host "================================================================" -ForegroundColor Green
    return
}

# -- Auto-elevate to Admin ---------------------------------------------------
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)

if (!$isAdmin) {
    Write-Host ""
    Write-Host "[FLASH] Se necesitan permisos de Administrador..." -ForegroundColor Yellow
    $argList = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    if ($DiskNumber -ge 0) { $argList += " -DiskNumber $DiskNumber" }
    if ($FlashOnly) { $argList += " -FlashOnly" }
    Start-Process powershell.exe -Verb RunAs -ArgumentList $argList -Wait
    exit 0
}

# -- Detectar USB ------------------------------------------------------------
Write-Host "[4/4] Flasheando al USB..." -ForegroundColor Cyan

if ($DiskNumber -lt 0) {
    Write-Host ""
    Write-Host "  Buscando discos USB..." -ForegroundColor Cyan
    $usbDisks = @(Get-Disk | Where-Object { $_.BusType -eq "USB" })

    if ($usbDisks.Count -eq 0) {
        Write-Host "  ERROR: No hay USB conectado." -ForegroundColor Red
        Read-Host "  Presiona Enter para salir"
        exit 1
    }

    Write-Host ""
    Write-Host "  Discos USB:" -ForegroundColor Green
    foreach ($d in $usbDisks) {
        $sizeGB = [math]::Round($d.Size / 1GB, 1)
        Write-Host "    [$($d.Number)]  $($d.FriendlyName)  ($sizeGB GB)" -ForegroundColor White
    }
    Write-Host ""

    if ($usbDisks.Count -eq 1) {
        $DiskNumber = $usbDisks[0].Number
        Write-Host "  Solo 1 USB -> Disco $DiskNumber seleccionado." -ForegroundColor Green
    } else {
        $sel = Read-Host "  Escribe el numero del disco USB"
        $DiskNumber = [int]$sel
    }
    Write-Host ""
}

# -- Validar disco -----------------------------------------------------------
$disk = Get-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
if (!$disk) { throw "Disco $DiskNumber no encontrado" }

$sysDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
if ($DiskNumber -eq $sysDisk) {
    Write-Host "  ERROR: El disco $DiskNumber es el DISCO DEL SISTEMA. Cancelado." -ForegroundColor Red
    Read-Host "  Presiona Enter para salir"
    exit 1
}

$diskSizeGB = [math]::Round($disk.Size / 1GB, 1)

Write-Host ""
Write-Host "  BOOTX64.EFI  $([math]::Round($efiSize/1024))KB" -ForegroundColor White
Write-Host "  kernel.elf   $([math]::Round($kernelSize/1024))KB" -ForegroundColor White
if (Test-Path "$Root\gsp_ga10x.bin") {
    $gs = [math]::Round((Get-Item "$Root\gsp_ga10x.bin").Length/1MB, 1)
    Write-Host "  gsp_ga10x   ${gs}MB (GSP-RM firmware)" -ForegroundColor White
}
Write-Host "  Disco [$DiskNumber] $($disk.FriendlyName) -- $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
Write-Host ""
Write-Host "  ESTO FORMATEARA EL DISCO $DiskNumber POR COMPLETO" -ForegroundColor Red
Write-Host ""
$confirm = Read-Host "  Escribe FLASH para continuar"
if ($confirm -ne "FLASH") {
    Write-Host "  Cancelado." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 0
}

# -- Formatear USB: GPT + FAT32 ----------------------------------------------
Write-Host ""
Write-Host "  [FLASH 1/3] Formateando GPT + FAT32..." -ForegroundColor Cyan

Write-Host "      Limpiando disco..." -ForegroundColor DarkGray
$disk | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false

Write-Host "      Aplicando GPT..." -ForegroundColor DarkGray
$disk | Set-Disk -PartitionStyle GPT

Write-Host "      Creando ESP..." -ForegroundColor DarkGray
$partition = $disk | New-Partition -UseMaximumSize `
    -GptType '{C12A7328-F81F-11D2-BA4B-00A0C93EC93B}' -AssignDriveLetter

Write-Host "      Formateando FAT32..." -ForegroundColor DarkGray
$partition | Format-Volume -FileSystem FAT32 -NewFileSystemLabel "FastOS" -Confirm:$false

$dl = $partition.DriveLetter
Write-Host "      Unidad asignada: ${dl}:\" -ForegroundColor DarkGray
Write-Host "  [FLASH 1/3] OK" -ForegroundColor Green

# -- Copiar archivos ---------------------------------------------------------
Write-Host "  [FLASH 2/3] Copiando archivos..." -ForegroundColor Cyan

$efiBootPath = "${dl}:\EFI\BOOT"
New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null

Copy-Item "$Root\BOOTX64.EFI" "$efiBootPath\BOOTX64.EFI" -Force
Write-Host "      EFI\BOOT\BOOTX64.EFI" -ForegroundColor DarkGray

Copy-Item "$Root\kernel.elf" "${dl}:\kernel.elf" -Force
Copy-Item "$Root\kernel.elf" "$efiBootPath\kernel.elf" -Force
Write-Host "      kernel.elf (root + EFI\BOOT\)" -ForegroundColor DarkGray

if (Test-Path "$Root\gsp_ga10x.bin") {
    Copy-Item "$Root\gsp_ga10x.bin" "${dl}:\gsp_ga10x.bin" -Force
    Write-Host "      gsp_ga10x.bin (GSP-RM firmware)" -ForegroundColor DarkGray
}

$fwDir = "$Root\firmware"
if (Test-Path $fwDir) {
    $fwUsbDir = "${dl}:\firmware"
    New-Item -Path $fwUsbDir -ItemType Directory -Force | Out-Null
    foreach ($f in @("bootloader-535.113.01.bin", "booter_load-535.113.01.bin", "vbios_rtx3060.rom",
                     "fwsec_ucode.bin", "fwsec_offset.bin")) {
        $src = Join-Path $fwDir $f
        if (!(Test-Path $src)) {
            $src = Join-Path "$Root\USB_boot\firmware" $f
        }
        if (Test-Path $src) {
            Copy-Item $src (Join-Path $fwUsbDir $f) -Force
            Write-Host "      firmware\$f" -ForegroundColor DarkGray
        }
    }
}

Write-Host "  [FLASH 2/3] OK" -ForegroundColor Green

# -- Verificar ---------------------------------------------------------------
Write-Host "  [FLASH 3/3] Verificando..." -ForegroundColor Cyan

$ok = $true
$checks = @(
    @{ Path = "$efiBootPath\BOOTX64.EFI"; Name = "BOOTX64.EFI"; Orig = "$Root\BOOTX64.EFI" },
    @{ Path = "${dl}:\kernel.elf";        Name = "kernel.elf";   Orig = "$Root\kernel.elf" }
)

foreach ($c in $checks) {
    if (!(Test-Path $c.Path)) {
        Write-Host "      FALLO: $($c.Name) no se copio" -ForegroundColor Red
        $ok = $false
    } else {
        $copied = (Get-Item $c.Path).Length
        $orig   = (Get-Item $c.Orig).Length
        if ($copied -eq $orig) {
            Write-Host "      $($c.Name): $copied bytes OK" -ForegroundColor DarkGray
        } else {
            Write-Host "      $($c.Name): tamano no coincide ($copied vs $orig bytes)" -ForegroundColor Yellow
            $ok = $false
        }
    }
}

if (Test-Path "${dl}:\gsp_ga10x.bin") {
    $gCopied = (Get-Item "${dl}:\gsp_ga10x.bin").Length
    $gOrig   = (Get-Item "$Root\gsp_ga10x.bin").Length
    if ($gCopied -eq $gOrig) {
        Write-Host "      gsp_ga10x.bin: $gCopied bytes OK" -ForegroundColor DarkGray
    } else {
        Write-Host "      gsp_ga10x.bin: tamano no coincide" -ForegroundColor Yellow
    }
}

# Verificar FWSEC
if (Test-Path "${dl}:\firmware\fwsec_ucode.bin") {
    $fwsecSize = (Get-Item "${dl}:\firmware\fwsec_ucode.bin").Length
    Write-Host "      fwsec_ucode.bin: $fwsecSize bytes OK" -ForegroundColor Green
} else {
    Write-Host "      AVISO: fwsec_ucode.bin no presente (FWSEC no extraido)" -ForegroundColor Yellow
}

if ($ok) {
    Write-Host "  [FLASH 3/3] Verificado OK" -ForegroundColor Green
} else {
    Write-Host "  [FLASH 3/3] Hubo errores -- revisa arriba" -ForegroundColor Red
}

# -- Resultado final ---------------------------------------------------------
$buildDate = Get-Date -Format 'yyyy-MM-dd HH:mm'

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  FASTOS LISTO EN USB (${dl}:\)" -ForegroundColor Green
Write-Host "  Build: $buildDate" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Contenido del USB:" -ForegroundColor White
Write-Host "    ${dl}:\EFI\BOOT\BOOTX64.EFI  (bootloader UEFI)" -ForegroundColor White
Write-Host "    ${dl}:\EFI\BOOT\kernel.elf    (kernel FastOS)" -ForegroundColor White
Write-Host "    ${dl}:\kernel.elf              (copia en root)" -ForegroundColor White
if (Test-Path "${dl}:\gsp_ga10x.bin") {
    Write-Host "    ${dl}:\gsp_ga10x.bin           (GSP-RM firmware)" -ForegroundColor White
}
if (Test-Path "${dl}:\firmware") {
    Write-Host "    ${dl}:\firmware\               (bootloader + booter_load + FWSEC)" -ForegroundColor White
}
Write-Host ""
Write-Host "  Pasos:" -ForegroundColor Yellow
Write-Host "    1. Reinicia el PC" -ForegroundColor White
Write-Host "    2. BIOS: CSM = DISABLED, Secure Boot = DISABLED" -ForegroundColor White
Write-Host "    3. Boot desde USB (UEFI)" -ForegroundColor White
Write-Host ""
Write-Host "  GSP SEC2 Boot -- que esperar en pantalla:" -ForegroundColor Cyan
Write-Host "    ANTES (roto):" -ForegroundColor Red
Write-Host "      WPR2_HI = 0x0  <- faltaba HS manifest parsing" -ForegroundColor DarkGray
Write-Host ""
Write-Host "    AHORA (HS manifest boot):" -ForegroundColor Green
Write-Host "      [7/11]  FWSEC-FRTS on SEC2 OK" -ForegroundColor DarkGray
Write-Host "      [8/11]  HS booter_load on SEC2..." -ForegroundColor DarkGray
Write-Host "        [HS] bin_hdr: magic=0x10de ..." -ForegroundColor DarkGray
Write-Host "        [HS] load_hdr: os_code/os_data parsed" -ForegroundColor DarkGray
Write-Host "        [HS-BOOT] IMEM loaded OK" -ForegroundColor DarkGray
Write-Host "        [HS-BOOT] Patched DMEM+0x... = WPR meta PA" -ForegroundColor DarkGray
Write-Host "        [HS-BOOT] DMEM loaded OK" -ForegroundColor DarkGray
Write-Host "        [HS-BOOT] SEC2 HS regs: dmem_sign, engine_id, ucode_id" -ForegroundColor DarkGray
Write-Host "      [11/11] WPR2_HI=0x... (WPR2 SET -- good!)  <- EXITO" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Si WPR2 sigue en 0x0: revisar patch_loc/patch_sig" -ForegroundColor Yellow
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Read-Host "  Presiona Enter para cerrar"