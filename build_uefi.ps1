# ============================================================================
# FastOS -- Build + Flash USB (UEFI GOP path)
# ============================================================================
# Compila bootloader + kernel, prepara USB_boot/, y flashea al USB.
# El camino estable de arranque usa UEFI GOP/framebuffer. Payloads/firmware GPU
# quedan como legado/investigacion fuera del boot path.
#
# Uso:
#   .\build_uefi.ps1                  # Build + flash (auto-detecta USB)
#   .\build_uefi.ps1 -DiskNumber 3    # Build + flash al disco 3
#   .\build_uefi.ps1 -BuildOnly       # Solo compilar, no flashear
#   .\build_uefi.ps1 -FlashOnly       # Solo flashear (ya compilado)
#   .\build_uefi.ps1 -Clean           # Limpiar artefactos
#
# Si Windows bloquea scripts PowerShell por ExecutionPolicy, usa:
#   .\build_uefi.cmd                  # Wrapper con Bypass solo para esta ejecucion
#
# Target: Bootloader UEFI + kernel GOP/framebuffer
# ============================================================================

param(
    [int]$DiskNumber = -1,
    [switch]$BuildOnly,
    [switch]$FlashOnly,
    [switch]$Clean,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

function Invoke-CargoBuildWithRetry {
    param(
        [string]$TargetDir,
        [int]$MaxAttempts = 3
    )

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $cargoOutput = & rustup run nightly cargo build --release --target-dir "$TargetDir" 2>&1
        $cargoExit = $LASTEXITCODE

        $accessDenied = ($cargoOutput | ForEach-Object { $_.ToString() }) -match "Acceso denegado|Access is denied|os error 5"
        if ($cargoExit -eq 0 -or !$accessDenied -or $attempt -eq $MaxAttempts) {
            return @{
                ExitCode = $cargoExit
                Output = $cargoOutput
            }
        }

        Write-Host "      Cargo: archivo bloqueado, reintentando ($attempt/$MaxAttempts)..." -ForegroundColor Yellow
        Start-Sleep -Seconds 2
    }
}

# -- Banner ------------------------------------------------------------------
Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS -- UEFI GOP Builder" -ForegroundColor Cyan
Write-Host "  Target: bootloader UEFI + kernel GOP/framebuffer" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# -- Clean -------------------------------------------------------------------
if ($Clean) {
    Write-Host "[CLEAN] Eliminando artefactos..." -ForegroundColor Yellow
    Remove-Item "$Root\bootloader\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\target_build" -Recurse -ErrorAction SilentlyContinue
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
    Write-Host "[1/3] Compilando UEFI Bootloader..." -ForegroundColor Cyan

    Push-Location "$Root\bootloader"
    $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $bootloaderTarget = "$Root\target_build\bootloader"
    New-Item -Path $bootloaderTarget -ItemType Directory -Force | Out-Null
    $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $bootloaderTarget
    $cargoOutput = $cargoResult.Output
    $cargoExit = $cargoResult.ExitCode
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

    $efiPath = Get-ChildItem "$bootloaderTarget\x86_64-unknown-uefi\release\fastos-bootloader*.efi" -File |
               Select-Object -First 1 -ExpandProperty FullName
    if (!$efiPath) { Pop-Location; throw "No se encontro BOOTX64.EFI" }

    Copy-Item $efiPath "$Root\BOOTX64.EFI" -Force
    $efiSize = (Get-Item "$Root\BOOTX64.EFI").Length
    Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024, 1)) KB" -ForegroundColor DarkGray
    Pop-Location
    Write-Host "[1/3] Bootloader OK" -ForegroundColor Green

    # -- Step 2: Build Kernel -------------------------------------------------
    Write-Host "[2/3] Compilando Kernel (ELF)..." -ForegroundColor Cyan

    Push-Location "$Root\kernel"
    $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $kernelTarget = "$Root\target_build\kernel"
    New-Item -Path $kernelTarget -ItemType Directory -Force | Out-Null
    $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $kernelTarget
    $cargoOutput = $cargoResult.Output
    $cargoExit = $cargoResult.ExitCode
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

    $elfPath = "$kernelTarget\x86_64-unknown-none\release\fastos-kernel"
    if (!(Test-Path $elfPath)) {
        $elfPath = Get-ChildItem "$kernelTarget\x86_64-unknown-none\release\fastos-kernel*" -File |
                   Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } |
                   Select-Object -First 1 -ExpandProperty FullName
    }
    if (!$elfPath -or !(Test-Path $elfPath)) { Pop-Location; throw "No se encontro kernel.elf" }

    Copy-Item $elfPath "$Root\kernel.elf" -Force
    $kernelSize = (Get-Item "$Root\kernel.elf").Length
    Write-Host "      kernel.elf: $([math]::Round($kernelSize/1024, 1)) KB" -ForegroundColor DarkGray
    Pop-Location
    Write-Host "[2/3] Kernel OK" -ForegroundColor Green

    # -- Step 2b: Build BMO-FS CLI --------------------------------------------
    Write-Host "[2b/3] Compilando BMO-FS CLI..." -ForegroundColor Cyan
    Push-Location "$Root\bmofs"
    $savedEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $bmofsTarget = "$Root\target_build\bmofs"
    New-Item -Path $bmofsTarget -ItemType Directory -Force | Out-Null
    $cargoResult = Invoke-CargoBuildWithRetry -TargetDir $bmofsTarget
    $cargoOutput = $cargoResult.Output
    $cargoExit = $cargoResult.ExitCode
    $ErrorActionPreference = $savedEAP

    $cargoOutput | ForEach-Object {
        $line = $_.ToString()
        if ($line -match "error\[") { Write-Host "      $line" -ForegroundColor Red }
        elseif ($line -match "Compiling|Finished") { Write-Host "      $line" -ForegroundColor DarkGray }
    }

    if ($cargoExit -ne 0) {
        $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
        Pop-Location; throw "BMO-FS CLI: fallo la compilacion"
    }
    $bmofsExe = "$bmofsTarget\release\bmofs.exe"
    if (!(Test-Path $bmofsExe)) {
        $bmofsExe = Get-ChildItem "$bmofsTarget\release\bmofs*.exe" -File | Select-Object -First 1 -ExpandProperty FullName
    }
    Pop-Location
    Write-Host "[2b/3] BMO-FS CLI OK" -ForegroundColor Green

    # -- Step 3: Preparar USB_boot/ -------------------------------------------
    Write-Host "[3/3] Preparando USB_boot/..." -ForegroundColor Cyan

    # Generar imagen de disco BMO-FS inicial
    Write-Host "      Creando imagen de disco BMO-FS (bmofs.img)..." -ForegroundColor DarkGray
    $bmofsImgPath = "$Root\bmofs.img"
    # Formatear imagen de 50MB (12800 bloques de 4KB)
    & $bmofsExe format $bmofsImgPath 12800 | Out-Null
    # Añadir un archivo de bienvenida a BMO-FS
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

    Write-Host "[3/3] USB_boot/ listo" -ForegroundColor Green
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
    Write-Host "  Para flashear: .\build_uefi.cmd -FlashOnly" -ForegroundColor Green
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
    if ($Force) { $argList += " -Force" }
    Start-Process powershell.exe -Verb RunAs -ArgumentList $argList -Wait
    exit 0
}

# -- Detectar USB ------------------------------------------------------------
Write-Host "[FLASH] Flasheando al USB..." -ForegroundColor Cyan

if ($DiskNumber -lt 0) {
    Write-Host ""
    Write-Host "  Buscando discos USB..." -ForegroundColor Cyan
    $usbDisks = @(Get-Disk | Where-Object { $_.BusType -eq "USB" })

    if ($usbDisks.Count -eq 0) {
        Write-Host "  ERROR: No hay USB conectado." -ForegroundColor Red
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
    Write-Host "  ERROR: El disco $DiskNumber es el DISCO DEL SISTEMA. Cancelado." -ForegroundColor Red
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

# -- Formatear USB: GPT + FAT32 (Estándar UEFI) --------------------------------
Write-Host ""
Write-Host "  [FLASH 1/3] Formateando GPT + FAT32 (UEFI)..." -ForegroundColor Cyan

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
    # Obtener el objeto particion actualizado para leer la letra asignada
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
    
    # Limpiar script temporal
    Remove-Item $dpScriptPath -Force -ErrorAction SilentlyContinue

    if ($process.ExitCode -ne 0) {
        throw "Diskpart fallo con codigo de salida $($process.ExitCode)"
    }

    Write-Host "      Buscando nueva letra de unidad..." -ForegroundColor DarkGray
    Start-Sleep -Seconds 3 # Dar tiempo a que Windows monte la particion
    
    # Buscar la letra de la partición formateada en ese disco
    $disk = Get-Disk -Number $DiskNumber
    $partition = Get-Partition -DiskNumber $DiskNumber | Where-Object { $_.DriveLetter } | Select-Object -First 1
    if (!$partition) {
        # Intentar buscar volumen por etiqueta
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
Write-Host "  [FLASH 1/3] OK" -ForegroundColor Green

# -- Copiar archivos ---------------------------------------------------------
Write-Host "  [FLASH 2/3] Copiando archivos desde USB_boot..." -ForegroundColor Cyan

$efiBootPath = "${dl}:\EFI\BOOT"

# Ensure we have the latest kernel and bootloader in the root USB_boot folder before copying
Copy-Item "$Root\BOOTX64.EFI" "$Root\USB_boot\BOOTX64.EFI" -Force
Copy-Item "$Root\BOOTX64.EFI" "$Root\USB_boot\EFI\BOOT\BOOTX64.EFI" -Force
Copy-Item "$Root\kernel.elf" "$Root\USB_boot\EFI\BOOT\kernel.elf" -Force
Copy-Item "$Root\kernel.elf" "$Root\USB_boot\kernel.elf" -Force
Copy-Item "$Root\bmofs.img"  "$Root\USB_boot\bmofs.img" -Force

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

Write-Host "  [FLASH 2/3] OK" -ForegroundColor Green

# -- Verificar ---------------------------------------------------------------
Write-Host "  [FLASH 3/3] Verificando..." -ForegroundColor Cyan

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
            Write-Host "      FALLO: origen $($c.Name) no existe" -ForegroundColor Red
            $ok = $false
        } else {
            Write-Host "      $($c.Name): omitido (legacy opcional)" -ForegroundColor DarkGray
        }
        continue
    }

    if (!(Test-Path $c.Path)) {
        if ($c.Required) {
            Write-Host "      FALLO: $($c.Name) no se copio" -ForegroundColor Red
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
            Write-Host "      $($c.Name): no coincide con origen ($copied vs $orig bytes / hashOk=$hashOk)" -ForegroundColor Yellow
            $ok = $false
        }
    }
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
