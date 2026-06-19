# ============================================================================
# FastOS -- Build + Flash USB (UEFI GOP path) - ROBUST VERSION
# ============================================================================
# Compila bootloader + kernel, prepara USB_boot/, y flashea al USB.
# Soporta cambios fuertes (deps, refactors) con -Rebuild / -ForceRebuild.
#
# Uso:
#   .\build_uefi.ps1                       # Build + flash (incremental)
#   .\build_uefi.ps1 -Rebuild              # Clean + build (cambios fuertes)
#   .\build_uefi.ps1 -ForceRebuild         # Clean + force cargo build
#   .\build_uefi.ps1 -BuildOnly -Rebuild   # Clean + build, no flash
#   .\build_uefi.ps1 -FlashOnly            # Solo flashear (ya compilado)
#   .\build_uefi.ps1 -KernelOnly           # Build rapido solo del kernel
#   .\build_uefi.ps1 -BootloaderOnly       # Build rapido solo del bootloader
#   .\build_uefi.ps1 -Verbose              # Mostrar todos los warnings
#   .\build_uefi.ps1 -SkipDepsCheck        # Saltar verificacion de toolchain
#   .\build_uefi.ps1 -Clean                # Limpiar artefactos
#   .\build_uefi.ps1 -Rollback             # Restaurar kernel.elf anterior
#
# Si Windows bloquea scripts PowerShell por ExecutionPolicy, usa:
#   .\build_uefi.cmd                       # Wrapper con Bypass solo para esta ejecucion
#
# Target: Bootloader UEFI + kernel GOP/framebuffer
# ============================================================================

param(
    [int]$DiskNumber = -1,
    [switch]$BuildOnly,
    [switch]$FlashOnly,
    [switch]$Clean,
    [switch]$Force,
    [switch]$Rebuild,           # Clean target/ antes de build
    [switch]$ForceRebuild,      # Clean + force cargo build
    [switch]$KernelOnly,        # Solo compilar kernel (no bootloader/bmofs)
    [switch]$BootloaderOnly,    # Solo compilar bootloader
    [switch]$Verbose,           # Mostrar todos los warnings
    [switch]$SkipDepsCheck,     # Saltar verificacion de toolchain
    [switch]$Rollback           # Restaurar kernel.elf anterior desde backup
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# ============================================================================
# Utilidades
# ============================================================================

function Write-Step {
    param([string]$Msg, [string]$Color = "Cyan")
    Write-Host "[$([DateTime]::Now.ToString('HH:mm:ss'))] $Msg" -ForegroundColor $Color
}

function Write-Success { param([string]$Msg) Write-Host "  [OK] $Msg" -ForegroundColor Green }
function Write-Warn    { param([string]$Msg) Write-Host "  [!]  $Msg" -ForegroundColor Yellow }
function Write-Err     { param([string]$Msg) Write-Host "  [X]  $Msg" -ForegroundColor Red }

function Test-Toolchain {
    if ($SkipDepsCheck) { return $true }

    Write-Step "Verificando toolchain..." "Cyan"

    # Verificar rustup
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if (!$rustup) {
        Write-Err "rustup no encontrado en PATH"
        return $false
    }

    # Verificar nightly
    $nightlyList = & rustup toolchain list 2>&1
    $hasNightly = $false
    foreach ($line in $nightlyList) {
        if ($line -match "nightly") { $hasNightly = $true; break }
    }

    if (!$hasNightly) {
        Write-Warn "Toolchain 'nightly' no instalado. Instalando..."
        $installOutput = & rustup toolchain install nightly 2>&1
        $installExit = $LASTEXITCODE
        Write-Host $installOutput
        if ($installExit -ne 0) {
            Write-Err "No se pudo instalar nightly"
            return $false
        }
    } else {
        Write-Host "      nightly: instalado" -ForegroundColor DarkGray
    }

    # Verificar target x86_64-unknown-none
    $targetsOutput = & rustup target list --installed --toolchain nightly 2>&1
    $hasKernelTarget = $false
    $hasUefiTarget = $false
    foreach ($line in $targetsOutput) {
        if ($line -match "x86_64-unknown-none") { $hasKernelTarget = $true }
        if ($line -match "x86_64-unknown-uefi") { $hasUefiTarget = $true }
    }

    if (!$hasKernelTarget) {
        Write-Warn "Target 'x86_64-unknown-none' no instalado. Instalando..."
        & rustup target add x86_64-unknown-none --toolchain nightly 2>&1 | Out-Null
    }

    if (!$hasUefiTarget) {
        Write-Warn "Target 'x86_64-unknown-uefi' no instalado. Instalando..."
        & rustup target add x86_64-unknown-uefi --toolchain nightly 2>&1 | Out-Null
    }

    Write-Success "Toolchain OK"
    return $true
}

function Get-CargoArtifacts {
    # Devuelve los paths esperados de los artefactos
    return @{
        Bootloader = "$Root\target_build\bootloader\x86_64-unknown-uefi\release\fastos-bootloader.efi"
        Kernel     = "$Root\target_build\kernel\x86_64-unknown-none\release\fastos-kernel"
        Bmofs      = "$Root\target_build\bmofs\release\bmofs.exe"
    }
}

function Get-FileMTime {
    param([string]$Path)
    if (Test-Path $Path) {
        return (Get-Item $Path).LastWriteTime
    }
    return [DateTime]::MinValue
}

function Test-NeedsRebuild {
    # Detecta si Cargo.toml o archivos .rs cambiaron desde el ultimo build
    # (heuristica simple: comparar timestamp de src/ vs target/)

    $artifacts = Get-CargoArtifacts
    $cargoToml = Join-Path $Root "kernel\Cargo.toml"
    $cargoTomlBoot = Join-Path $Root "bootloader\Cargo.toml"

    if (!(Test-Path $artifacts.Kernel) -or !(Test-Path $artifacts.Bootloader)) {
        return $true
    }

    $kernelTime = Get-FileMTime $artifacts.Kernel
    $bootTime   = Get-FileMTime $artifacts.Bootloader
    $tomlTime   = if (Test-Path $cargoToml) { Get-FileMTime $cargoToml } else { [DateTime]::MinValue }
    $tomlTimeBoot = if (Test-Path $cargoTomlBoot) { Get-FileMTime $cargoTomlBoot } else { [DateTime]::MinValue }

    # Si Cargo.toml es mas reciente que el artefacto, rebuild necesario
    if ($tomlTime -gt $kernelTime -or $tomlTimeBoot -gt $bootTime) {
        return $true
    }

    return $false
}

function Backup-Kernel {
    # Guarda el kernel.elf actual como .prev para rollback
    $prev = "$Root\kernel.elf.prev"
    if (Test-Path "$Root\kernel.elf") {
        Copy-Item "$Root\kernel.elf" $prev -Force
        Write-Success "Backup: kernel.elf -> kernel.elf.prev"
    }
    $prevBoot = "$Root\BOOTX64.EFI.prev"
    if (Test-Path "$Root\BOOTX64.EFI") {
        Copy-Item "$Root\BOOTX64.EFI" $prevBoot -Force
    }
}

function Restore-Rollback {
    $prev = "$Root\kernel.elf.prev"
    $prevBoot = "$Root\BOOTX64.EFI.prev"
    if (!(Test-Path $prev) -or !(Test-Path $prevBoot)) {
        Write-Err "No hay backup .prev para hacer rollback"
        return $false
    }
    Copy-Item $prev "$Root\kernel.elf" -Force
    Copy-Item $prev "$Root\kernel.elf" -Force
    Copy-Item $prevBoot "$Root\BOOTX64.EFI" -Force
    Write-Success "Rollback: kernel.elf y BOOTX64.EFI restaurados"
    return $true
}

function Invoke-CargoBuildWithRetry {
    param(
        [string]$TargetDir,
        [string]$ManifestPath = ".",
        [string]$Target = "",
        [int]$MaxAttempts = 3,
        [switch]$Force
    )

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $forceFlag = ""
        if ($Force) { $forceFlag = " --force" }

        $targetFlag = ""
        if ($Target) { $targetFlag = " --target $Target" }

        $cargoCmd = "rustup run nightly cargo build --release --target-dir `"$TargetDir`" --manifest-path `"$ManifestPath`"$targetFlag$forceFlag 2>&1"
        $cargoOutput = cmd.exe /c $cargoCmd
        $cargoExit = $LASTEXITCODE

        $accessDenied = ($cargoOutput | ForEach-Object { $_.ToString() }) -match "Acceso denegado|Access is denied|os error 5"
        $lockError = ($cargoOutput | ForEach-Object { $_.ToString() }) -match "Blocking waiting for file lock"

        if ($cargoExit -eq 0) {
            return @{
                ExitCode = 0
                Output = $cargoOutput
            }
        }

        if (($accessDenied -or $lockError) -and $attempt -lt $MaxAttempts) {
            Write-Host "      Cargo: archivo bloqueado, reintentando ($attempt/$MaxAttempts)..." -ForegroundColor Yellow
            Start-Sleep -Seconds 3
            # Matar procesos cargo.exe colgados
            Get-Process cargo -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 1
            continue
        }

        return @{
            ExitCode = $cargoExit
            Output = $cargoOutput
        }
    }
}

function Invoke-CleanTarget {
    param([string]$Component)

    switch ($Component) {
        "kernel" {
            if (Test-Path "$Root\target_build\kernel") {
                Write-Host "      Limpiando target_build\kernel..." -ForegroundColor DarkGray
                Remove-Item "$Root\target_build\kernel" -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        "bootloader" {
            if (Test-Path "$Root\target_build\bootloader") {
                Write-Host "      Limpiando target_build\bootloader..." -ForegroundColor DarkGray
                Remove-Item "$Root\target_build\bootloader" -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        "bmofs" {
            if (Test-Path "$Root\target_build\bmofs") {
                Write-Host "      Limpiando target_build\bmofs..." -ForegroundColor DarkGray
                Remove-Item "$Root\target_build\bmofs" -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        default {
            if (Test-Path "$Root\target_build") {
                Write-Host "      Limpiando target_build..." -ForegroundColor DarkGray
                Remove-Item "$Root\target_build" -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

# ============================================================================
# Banner
# ============================================================================

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS -- UEFI GOP Builder (ROBUST)" -ForegroundColor Cyan
Write-Host "  Target: bootloader UEFI + kernel GOP/framebuffer" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# ============================================================================
# Rollback (salir despues)
# ============================================================================
if ($Rollback) {
    if (Restore-Rollback) {
        exit 0
    } else {
        exit 1
    }
}

# ============================================================================
# Clean
# ============================================================================
if ($Clean) {
    Write-Step "[CLEAN] Eliminando artefactos..." "Yellow"
    Invoke-CleanTarget "all"
    Remove-Item "$Root\kernel.elf"        -ErrorAction SilentlyContinue
    Remove-Item "$Root\BOOTX64.EFI"       -ErrorAction SilentlyContinue
    Remove-Item "$Root\USB_boot"          -Recurse -ErrorAction SilentlyContinue
    Write-Success "Clean completo"
    return
}

# ============================================================================
# Verificar toolchain
# ============================================================================
if (!(Test-Toolchain)) {
    Write-Err "Toolchain invalida. Aborta."
    exit 1
}

# ============================================================================
# Backup antes de cambios fuertes
# ============================================================================
if (($Rebuild -or $ForceRebuild) -and (Test-Path "$Root\kernel.elf")) {
    Write-Step "Backup de seguridad (cambio fuerte detectado)..." "Yellow"
    Backup-Kernel
}

# ============================================================================
# FASE 1: BUILD
# ============================================================================
$efiSize = 0
$kernelSize = 0

if (!$FlashOnly) {

    # Decidir si hacer clean
    $needsClean = $false
    if ($ForceRebuild) {
        $needsClean = $true
    } elseif ($Rebuild) {
        $needsClean = $true
    } elseif (Test-NeedsRebuild) {
        Write-Warn "Cambios estructurales detectados (Cargo.toml) -- recomendado -Rebuild"
        if (!$Force) {
            $ans = Read-Host "  Forzar rebuild? (s/N)"
            if ($ans -eq "s" -or $ans -eq "S") {
                $needsClean = $true
            }
        }
    }

    # -- Step 1: Build UEFI Bootloader ----------------------------------------
    if (!$KernelOnly) {
        Write-Step "[1/3] Compilando UEFI Bootloader..." "Cyan"

        if ($needsClean) {
            Invoke-CleanTarget "bootloader"
        }

        $bootloaderTarget = "$Root\target_build\bootloader"
        New-Item -Path $bootloaderTarget -ItemType Directory -Force | Out-Null
        $manifestPath = "$Root\bootloader\Cargo.toml"

        $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $bootloaderTarget -ManifestPath $manifestPath -Target "x86_64-unknown-uefi" -Force:$ForceRebuild
        $cargoOutput = $cargoResult.Output
        $cargoExit = $cargoResult.ExitCode

        if ($Verbose) {
            $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor DarkGray }
        } else {
            $cargoOutput | ForEach-Object {
                $line = $_.ToString()
                if ($line -match "error\[")     { Write-Host "      $line" -ForegroundColor Red }
                elseif ($line -match "warning:") { Write-Host "      $line" -ForegroundColor Yellow }
                elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
            }
        }

        if ($cargoExit -ne 0) {
            $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
            throw "Bootloader: fallo la compilacion"
        }

        $efiPath = Get-ChildItem "$bootloaderTarget\x86_64-unknown-uefi\release\fastos-bootloader*.efi" -File |
                   Select-Object -First 1 -ExpandProperty FullName
        if (!$efiPath) { throw "No se encontro BOOTX64.EFI" }

        Copy-Item $efiPath "$Root\BOOTX64.EFI" -Force
        $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
        Write-Success "BOOTX64.EFI: $([math]::Round($efiSize/1024, 1)) KB"
    } else {
        # KernelOnly: usar el BOOTX64.EFI existente
        if (Test-Path "$Root\BOOTX64.EFI") {
            $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
            Write-Success "BOOTX64.EFI (existente): $([math]::Round($efiSize/1024, 1)) KB"
        }
    }

    # -- Step 2: Build Kernel -------------------------------------------------
    if (!$BootloaderOnly) {
        Write-Step "[2/3] Compilando Kernel (ELF)..." "Cyan"

        if ($needsClean) {
            Invoke-CleanTarget "kernel"
        }

        $kernelTarget = "$Root\target_build\kernel"
        New-Item -Path $kernelTarget -ItemType Directory -Force | Out-Null
        $manifestPath = "$Root\kernel\Cargo.toml"

        $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $kernelTarget -ManifestPath $manifestPath -Target "x86_64-unknown-none" -Force:$ForceRebuild
        $cargoOutput = $cargoResult.Output
        $cargoExit = $cargoResult.ExitCode

        if ($Verbose) {
            $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor DarkGray }
        } else {
            $cargoOutput | ForEach-Object {
                $line = $_.ToString()
                if ($line -match "error\[")     { Write-Host "      $line" -ForegroundColor Red }
                elseif ($line -match "warning:") { Write-Host "      $line" -ForegroundColor Yellow }
                elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
            }
        }

        if ($cargoExit -ne 0) {
            $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
            throw "Kernel: fallo la compilacion"
        }

        $elfPath = "$kernelTarget\x86_64-unknown-none\release\fastos-kernel"
        if (!(Test-Path $elfPath)) {
            $elfPath = Get-ChildItem "$kernelTarget\x86_64-unknown-none\release\fastos-kernel*" -File |
                       Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } |
                       Select-Object -First 1 -ExpandProperty FullName
        }
        if (!$elfPath -or !(Test-Path $elfPath)) { throw "No se encontro kernel.elf" }

        Copy-Item $elfPath "$Root\kernel.elf" -Force
        $kernelSize = (Get-Item "$Root\kernel.elf").Length
        Write-Success "kernel.elf: $([math]::Round($kernelSize/1024, 1)) KB"
    } else {
        # BootloaderOnly: usar el kernel.elf existente
        if (Test-Path "$Root\kernel.elf") {
            $kernelSize = (Get-Item "$Root\kernel.elf").Length
            Write-Success "kernel.elf (existente): $([math]::Round($kernelSize/1024, 1)) KB"
        }
    }

    # -- Step 2b: Build BMO-FS CLI --------------------------------------------
    if (!$KernelOnly -and !$BootloaderOnly) {
        Write-Step "[2b/3] Compilando BMO-FS CLI..." "Cyan"
        Push-Location "$Root\bmofs"
        $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
        $bmofsTarget = "$Root\target_build\bmofs"
        New-Item -Path $bmofsTarget -ItemType Directory -Force | Out-Null
        $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $bmofsTarget -ManifestPath "$Root\bmofs\Cargo.toml" -Force:$ForceRebuild
        $cargoOutput = $cargoResult.Output
        $cargoExit = $cargoResult.ExitCode
        $ErrorActionPreference = $savedEAP

        $cargoOutput | ForEach-Object {
            $line = $_.ToString()
            if ($line -match "error\[")     { Write-Host "      $line" -ForegroundColor Red }
            elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
        }

        if ($cargoExit -ne 0) {
            $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
            throw "BMO-FS CLI: fallo la compilacion"
        }
        $bmofsExe = "$bmofsTarget\release\bmofs.exe"
        if (!(Test-Path $bmofsExe)) {
            $bmofsExe = Get-ChildItem "$bmofsTarget\release\bmofs*.exe" -File | Select-Object -First 1 -ExpandProperty FullName
        }
        Pop-Location
        Write-Success "BMO-FS CLI OK"

        # -- Step 3: Preparar USB_boot/ -------------------------------------------
        Write-Step "[3/3] Preparando USB_boot/..." "Cyan"

        Write-Host "      Creando imagen de disco BMO-FS (bmofs.img)..." -ForegroundColor DarkGray
        $bmofsImgPath = "$Root\bmofs.img"
        & $bmofsExe format $bmofsImgPath 12800 | Out-Null
        $readmeTemp = Join-Path $Root "readme_bmofs.txt"
        Set-Content -Path $readmeTemp -Value "¡Bienvenido a BMO-FS! Este archivo vive dentro de la imagen nativa en tu USB." -Encoding UTF8
        & $bmofsExe add $bmofsImgPath $readmeTemp "readme.txt" | Out-Null
        Remove-Item $readmeTemp -Force -ErrorAction SilentlyContinue

        $usbDir = "$Root\USB_boot"
        $efiBootDir = "$usbDir\EFI\BOOT"
        New-Item -Path $efiBootDir -ItemType Directory -Force | Out-Null

        Copy-Item "$Root\BOOTX64.EFI" "$usbDir\BOOTX64.EFI" -Force
        Copy-Item "$Root\BOOTX64.EFI" "$efiBootDir\BOOTX64.EFI" -Force
        Copy-Item "$Root\kernel.elf"  "$usbDir\kernel.elf" -Force
        Copy-Item "$Root\kernel.elf"  "$efiBootDir\kernel.elf" -Force
        Copy-Item "$bmofsImgPath"     "$usbDir\bmofs.img" -Force

        Write-Success "USB_boot/ listo"
    }
} else {
    if (!(Test-Path "$Root\BOOTX64.EFI") -or !(Test-Path "$Root\kernel.elf")) {
        throw "No se encontraron BOOTX64.EFI o kernel.elf -- ejecuta sin -FlashOnly primero"
    }
    $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
    $kernelSize = (Get-Item "$Root\kernel.elf").Length
    Write-Warn "Saltando compilacion (-FlashOnly)"
    Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024, 1)) KB" -ForegroundColor DarkGray
    Write-Host "      kernel.elf:  $([math]::Round($kernelSize/1024, 1)) KB" -ForegroundColor DarkGray
}

# ============================================================================
# Diff visual (mostrar que cambio)
# ============================================================================
$prevMarker = "$Root\USB_boot\FASTOS_BUILD_MARKER.txt"
$newHash = (Get-FileHash "$Root\kernel.elf" -Algorithm SHA256).Hash
$newEfiHash = (Get-FileHash "$Root\BOOTX64.EFI" -Algorithm SHA256).Hash
$prevHash = $null
$prevEfiHash = $null
if (Test-Path $prevMarker) {
    Get-Content $prevMarker | ForEach-Object {
        if ($_ -match "kernel\.elf\.SHA256=(.+)") { $prevHash = $matches[1] }
        if ($_ -match "BOOTX64\.EFI\.SHA256=(.+)") { $prevEfiHash = $matches[1] }
    }
}

Write-Host ""
Write-Step "Cambios desde el ultimo flash:" "Cyan"
if ($prevHash -and $prevHash -ne $newHash) {
    Write-Host "      kernel.elf:   CAMBIO  ($prevHash -> $newHash)" -ForegroundColor Yellow
} elseif ($prevHash) {
    Write-Host "      kernel.elf:   sin cambios" -ForegroundColor DarkGray
} else {
    Write-Host "      kernel.elf:   (primera compilacion)" -ForegroundColor DarkGray
}
if ($prevEfiHash -and $prevEfiHash -ne $newEfiHash) {
    Write-Host "      BOOTX64.EFI:  CAMBIO  ($prevEfiHash -> $newEfiHash)" -ForegroundColor Yellow
} elseif ($prevEfiHash) {
    Write-Host "      BOOTX64.EFI:  sin cambios" -ForegroundColor DarkGray
} else {
    Write-Host "      BOOTX64.EFI:  (primera compilacion)" -ForegroundColor DarkGray
}

# ============================================================================
# Salir si solo build
# ============================================================================
if ($BuildOnly) {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host "  BUILD COMPLETO (sin flash)" -ForegroundColor Green
    Write-Host "  Para flashear: .\build_uefi.cmd -FlashOnly" -ForegroundColor Green
    Write-Host "================================================================" -ForegroundColor Green
    return
}

# ============================================================================
# FASE 2: FLASH USB
# ============================================================================

# -- Auto-elevate to Admin ---------------------------------------------------
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)

if (!$isAdmin) {
    Write-Host ""
    Write-Step "Se necesitan permisos de Administrador..." "Yellow"
    $argList = "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    if ($DiskNumber -ge 0) { $argList += " -DiskNumber $DiskNumber" }
    if ($FlashOnly) { $argList += " -FlashOnly" }
    if ($BuildOnly) { $argList += " -BuildOnly" }
    if ($Rebuild) { $argList += " -Rebuild" }
    if ($ForceRebuild) { $argList += " -ForceRebuild" }
    if ($KernelOnly) { $argList += " -KernelOnly" }
    if ($BootloaderOnly) { $argList += " -BootloaderOnly" }
    if ($Verbose) { $argList += " -Verbose" }
    if ($SkipDepsCheck) { $argList += " -SkipDepsCheck" }
    if ($Force) { $argList += " -Force" }
    Start-Process powershell.exe -Verb RunAs -ArgumentList $argList -Wait
    exit 0
}

# -- Detectar USB ------------------------------------------------------------
Write-Step "[FLASH] Flasheando al USB..." "Cyan"

if ($DiskNumber -lt 0) {
    Write-Host ""
    Write-Host "  Buscando discos USB..." -ForegroundColor Cyan
    $usbDisks = @(Get-Disk | Where-Object { $_.BusType -eq "USB" })

    if ($usbDisks.Count -eq 0) {
        Write-Err "No hay USB conectado."
        if (!$Force) { Read-Host "  Presiona Enter para salir" }
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
    Write-Err "El disco $DiskNumber es el DISCO DEL SISTEMA. Cancelado."
    if (!$Force) { Read-Host "  Presiona Enter para salir" }
    exit 1
}

$diskSizeGB = [math]::Round($disk.Size / 1GB, 1)

Write-Host ""
Write-Host "  BOOTX64.EFI  $([math]::Round($efiSize/1024))KB" -ForegroundColor White
Write-Host "  kernel.elf   $([math]::Round($kernelSize/1024))KB" -ForegroundColor White
Write-Host "  Disco [$DiskNumber] $($disk.FriendlyName) -- $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
Write-Host ""
Write-Host "  ESTO FORMATEARA EL DISCO $DiskNumber POR COMPLETO" -ForegroundColor Red
Write-Host ""
if (!$Force) {
    $confirm = Read-Host "  Escribe FLASH para continuar"
    if ($confirm -ne "FLASH") {
        Write-Host "  Cancelado." -ForegroundColor Yellow
        Read-Host "  Presiona Enter para salir"
        exit 0
    }
} else {
    Write-Host "  [FORCE] Saltando confirmacion interactiva..." -ForegroundColor Yellow
}

# -- Formatear USB: GPT + FAT32 (Estandar UEFI) --------------------------------
Write-Host ""
Write-Step "[FLASH 1/3] Formateando GPT + FAT32 (UEFI)..." "Cyan"

$dl = $null
try {
    Write-Host "      Limpiando disco (PowerShell)..." -ForegroundColor DarkGray
    $disk | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false

    Write-Host "      Aplicando GPT (GUID Partition Table)..." -ForegroundColor DarkGray
    $disk | Set-Disk -PartitionStyle GPT

    Write-Host "      Creando particion EFI (ESP)..." -ForegroundColor DarkGray
    $partition = $disk | New-Partition -GptType '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}' -UseMaximumSize -AssignDriveLetter

    Write-Host "      Formateando FAT32..." -ForegroundColor DarkGray
    $partition | Format-Volume -FileSystem FAT32 -NewFileSystemLabel "FastOS" -Confirm:$false
    Start-Sleep -Seconds 2
    $updatedPartition = Get-Partition -DiskNumber $DiskNumber | Where-Object { $_.DriveLetter } | Select-Object -First 1
    if ($updatedPartition) {
        $dl = $updatedPartition.DriveLetter
    } else {
        $dl = $partition.DriveLetter
    }
} catch {
    Write-Host "      [!] Error formateando con PowerShell: $_" -ForegroundColor Yellow
    Write-Host "      [!] Intentando con Diskpart (metodo alternativo ultra-robusto)..." -ForegroundColor Cyan

    $dpScriptPath = Join-Path $Root "diskpart_temp.txt"
    $dpScript = @"
select disk $DiskNumber
clean
convert gpt
create partition efi
format fs=fat32 quick label="FastOS"
assign
"@
    Set-Content -Path $dpScriptPath -Value $dpScript -Encoding ASCII

    Write-Host "      Ejecutando diskpart..." -ForegroundColor DarkGray
    $process = Start-Process diskpart.exe -ArgumentList "/s `"$dpScriptPath`"" -NoNewWindow -PassThru -Wait

    Remove-Item $dpScriptPath -Force -ErrorAction SilentlyContinue

    if ($process.ExitCode -ne 0) {
        throw "Diskpart fallo con codigo de salida $($process.ExitCode)"
    }

    Write-Host "      Buscando nueva letra de unidad..." -ForegroundColor DarkGray
    Start-Sleep -Seconds 3

    $disk = Get-Disk -Number $DiskNumber
    $partition = Get-Partition -DiskNumber $DiskNumber | Where-Object { $_.DriveLetter } | Select-Object -First 1
    if (!$partition) {
        $vol = Get-Volume | Where-Object { $_.FileSystemLabel -eq "FastOS" } | Select-Object -First 1
        if ($vol -and $vol.DriveLetter) {
            $dl = $vol.DriveLetter
        } else {
            throw "No se encontro la letra de unidad asignada tras Diskpart"
        }
    } else {
        $dl = $partition.DriveLetter
    }
}

if (!$dl) {
    throw "No se pudo obtener la letra de unidad del USB"
}

Write-Host "      Unidad asignada: ${dl}:\" -ForegroundColor DarkGray
Write-Success "[FLASH 1/3] OK"

# -- Copiar archivos ---------------------------------------------------------
Write-Step "[FLASH 2/3] Copiando archivos desde USB_boot..." "Cyan"

$efiBootPath = "${dl}:\EFI\BOOT"

Copy-Item "$Root\BOOTX64.EFI" "$Root\USB_boot\BOOTX64.EFI" -Force
Copy-Item "$Root\BOOTX64.EFI" "$Root\USB_boot\EFI\BOOT\BOOTX64.EFI" -Force
Copy-Item "$Root\kernel.elf" "$Root\USB_boot\EFI\BOOT\kernel.elf" -Force
Copy-Item "$Root\kernel.elf" "$Root\USB_boot\kernel.elf" -Force
if (Test-Path "$Root\bmofs.img") {
    Copy-Item "$Root\bmofs.img"  "$Root\USB_boot\bmofs.img" -Force
}

$markerPath = "$Root\USB_boot\FASTOS_BUILD_MARKER.txt"
$efiHash = (Get-FileHash "$Root\BOOTX64.EFI" -Algorithm SHA256).Hash
$kernelHash = (Get-FileHash "$Root\kernel.elf" -Algorithm SHA256).Hash
Set-Content -Path $markerPath -Encoding ASCII -Value @(
    "FastOS USB build marker",
    "Date=$((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))",
    "BOOTX64.EFI.Size=$((Get-Item "$Root\BOOTX64.EFI").Length)",
    "BOOTX64.EFI.SHA256=$efiHash",
    "kernel.elf.Size=$((Get-Item "$Root\kernel.elf").Length)",
    "kernel.elf.SHA256=$kernelHash"
)

# Copy the entire USB_boot directory to the USB flash drive
Copy-Item -Path "$Root\USB_boot\*" -Destination "${dl}:\" -Recurse -Force
Write-Host "      Copiado todo el contenido de USB_boot (BOOTX64.EFI, kernel.elf, bmofs.img, firmware, etc.)" -ForegroundColor DarkGray

Write-Success "[FLASH 2/3] OK"

# -- Verificar ---------------------------------------------------------------
Write-Step "[FLASH 3/3] Verificando..." "Cyan"

$ok = $true
$checks = @(
    @{ Path = "$efiBootPath\BOOTX64.EFI"; Name = "BOOTX64.EFI"; Orig = "$Root\BOOTX64.EFI"; Required = $true },
    @{ Path = "${dl}:\BOOTX64.EFI";       Name = "BOOTX64.EFI root"; Orig = "$Root\BOOTX64.EFI"; Required = $true },
    @{ Path = "$efiBootPath\kernel.elf";  Name = "kernel.elf EFI"; Orig = "$Root\kernel.elf"; Required = $true },
    @{ Path = "${dl}:\kernel.elf";        Name = "kernel.elf";   Orig = "$Root\kernel.elf"; Required = $true },
    @{ Path = "${dl}:\bmofs.img";         Name = "bmofs.img";    Orig = "$Root\bmofs.img";  Required = $true },
    @{ Path = "${dl}:\FASTOS_BUILD_MARKER.txt"; Name = "FASTOS_BUILD_MARKER.txt"; Orig = "$Root\USB_boot\FASTOS_BUILD_MARKER.txt"; Required = $true },
    @{ Path = "${dl}:\fastos_boot.bin";   Name = "fastos_boot.bin"; Orig = "$Root\USB_boot\fastos_boot.bin"; Required = $false }
)

foreach ($c in $checks) {
    if (!(Test-Path $c.Orig)) {
        if ($c.Required) {
            Write-Err "$($c.Name): origen no existe"
            $ok = $false
        } else {
            Write-Host "      $($c.Name): omitido (legacy opcional)" -ForegroundColor DarkGray
        }
        continue
    }

    if (!(Test-Path $c.Path)) {
        if ($c.Required) {
            Write-Err "$($c.Name): no se copio"
            $ok = $false
        } else {
            Write-Host "      $($c.Name): no presente (legacy opcional)" -ForegroundColor DarkGray
        }
    } else {
        $copied = (Get-Item $c.Path).Length
        $orig   = (Get-Item $c.Orig).Length
        $hashOk = $true
        if ($c.Required) {
            $copiedHash = (Get-FileHash $c.Path -Algorithm SHA256).Hash
            $origHash = (Get-FileHash $c.Orig -Algorithm SHA256).Hash
            $hashOk = ($copiedHash -eq $origHash)
        }
        if ($copied -eq $orig -and $hashOk) {
            Write-Host "      $($c.Name): $copied bytes SHA256 OK" -ForegroundColor DarkGray
        } else {
            Write-Err "$($c.Name): no coincide ($copied vs $orig bytes)"
            $ok = $false
        }
    }
}

if ($ok) {
    Write-Success "[FLASH 3/3] Verificado OK"
} else {
    Write-Err "[FLASH 3/3] Hubo errores -- revisa arriba"
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
Write-Host "    ${dl}:\fastos_boot.bin         (payload legacy opcional; no requerido para GOP)" -ForegroundColor White
Write-Host "    ${dl}:\firmware\               (Binarios extras de hardware)" -ForegroundColor White
Write-Host ""
Write-Host "  Pasos:" -ForegroundColor Yellow
Write-Host "    1. Reinicia el PC" -ForegroundColor White
Write-Host "    2. BIOS: CSM = DISABLED, Secure Boot = DISABLED" -ForegroundColor White
Write-Host "    3. Boot desde USB (UEFI)" -ForegroundColor White
Write-Host ""
Write-Host "  FastOS GOP boot -- que esperar en pantalla:" -ForegroundColor Cyan
Write-Host "    1. El Bootloader lanzara FastOS de inmediato." -ForegroundColor White
Write-Host "    2. El kernel usara UEFI GOP framebuffer." -ForegroundColor White
Write-Host "    3. Veras la pantalla de bienvenida BMO." -ForegroundColor White
Write-Host "    4. Escribe Run para entrar al escritorio Ring 0." -ForegroundColor White
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
if (!$Force) {
    Read-Host "  Presiona Enter para cerrar"
}
