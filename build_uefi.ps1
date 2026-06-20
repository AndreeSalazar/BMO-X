# ============================================================================
# FastOS -- Build + Flash USB (UEFI GOP) - MODERN VERSION
# ============================================================================
# Reescrito v1.7.3: modular, robusto, idempotente. Compila los 3 crates,
# prepara USB_boot/ y flashea al USB con confirmación.
#
# Uso:
#   .\build_uefi.ps1                            # Build + flash (incremental)
#   .\build_uefi.ps1 -Rebuild                   # Clean target_build + build
#   .\build_uefi.ps1 -ForceRebuild              # Clean + force cargo build
#   .\build_uefi.ps1 -BuildOnly                 # Solo compilar, sin flash
#   .\build_uefi.ps1 -BuildOnly -Rebuild        # Clean + build, no flash
#   .\build_uefi.ps1 -FlashOnly                 # Solo flashear (ya compilado)
#   .\build_uefi.ps1 -KernelOnly                # Build rapido solo kernel
#   .\build_uefi.ps1 -BootloaderOnly            # Build rapido solo bootloader
#   .\build_uefi.ps1 -BmofsOnly                 # Build rapido solo bmofs
#   .\build_uefi.ps1 -Clean                     # Solo limpiar artefactos
#   .\build_uefi.ps1 -SkipDepsCheck             # Saltar verificacion de toolchain
#   .\build_uefi.ps1 -ShowAllOutput            # Mostrar todos los warnings
#   .\build_uefi.ps1 -Force                     # Saltar confirmacion de flash
#   .\build_uefi.ps1 -DiskNumber 2             # Elegir disco especifico
#
# Si Windows bloquea scripts por ExecutionPolicy, usa:
#   .\build_uefi.cmd                            # Wrapper con Bypass
#
# Crates:
#   bootloader  -> target x86_64-unknown-uefi
#   kernel      -> target x86_64-unknown-none
#   bmofs       -> target host (x86_64-pc-windows-msvc)
# ============================================================================

[CmdletBinding()]
param(
    [int]   $DiskNumber  = -1,
    [switch] $BuildOnly,
    [switch] $FlashOnly,
    [switch] $Clean,
    [switch] $Rebuild,
    [switch] $ForceRebuild,
    [switch] $KernelOnly,
    [switch] $BootloaderOnly,
    [switch] $BmofsOnly,
    [switch] $SkipDepsCheck,
    [switch] $ShowAllOutput,
    [switch] $Force
)

# ============================================================================
# CONSTANTES
# ============================================================================

$Script:ScriptVersion = "1.7.3"
$Script:Root          = (Resolve-Path $PSScriptRoot).Path
$Script:TargetBuild    = Join-Path $Script:Root "target_build"
$Script:UsbBoot        = Join-Path $Script:Root "USB_boot"
$Script:BootloaderDir  = Join-Path $Script:Root "bootloader"
$Script:KernelDir      = Join-Path $Script:Root "kernel"
$Script:BmofsDir       = Join-Path $Script:Root "bmofs"
$Script:BootloaderOut  = Join-Path $Script:TargetBuild "bootloader\x86_64-unknown-uefi\release\fastos-bootloader.efi"
$Script:KernelOut      = Join-Path $Script:TargetBuild "kernel\x86_64-unknown-none\release\fastos-kernel"
$Script:BmofsOut       = Join-Path $Script:TargetBuild "bmofs\release\bmofs.exe"
$Script:EfiTarget      = "BOOTX64.EFI"

# ============================================================================
# OUTPUT HELPERS
# ============================================================================

function Write-Banner {
    Clear-Host
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host "  FastOS Build + Flash Script v$Script:ScriptVersion" -ForegroundColor Cyan
    Write-Host "  Root: $Script:Root" -ForegroundColor DarkGray
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Step {
    param([string]$Msg, [string]$Color = "Cyan")
    Write-Host ""
    Write-Host "  [STEP] $Msg" -ForegroundColor $Color
    Write-Host "  ----------------------------------------------------------------" -ForegroundColor DarkGray
}

function Write-OK   { param([string]$Msg) Write-Host "  [OK]   $Msg" -ForegroundColor Green }
function Write-Info { param([string]$Msg) Write-Host "  [i]    $Msg" -ForegroundColor Gray }
function Write-Warn { param([string]$Msg) Write-Host "  [!]    $Msg" -ForegroundColor Yellow }
function Write-Err  { param([string]$Msg) Write-Host "  [ERR]  $Msg" -ForegroundColor Red }
function Write-Sub  { param([string]$Msg) Write-Host "          $Msg" -ForegroundColor DarkGray }

function Get-FileMTime {
    param([string]$Path)
    if (Test-Path $Path) { return (Get-Item $Path).LastWriteTimeUtc }
    return [DateTime]::MinValue
}

function Test-FileNewer {
    param([string]$Path, [DateTime]$RefTime)
    if (!(Test-Path $Path)) { return $false }
    return (Get-Item $Path).LastWriteTimeUtc -gt $RefTime
}

function Read-Confirm {
    param([string]$Prompt, [string]$Expected, [switch]$Force)
    if ($Force) {
        Write-Host "  [FORCE] Saltando confirmacion..." -ForegroundColor Yellow
        return $true
    }
    $resp = Read-Host "  $Prompt"
    return ($resp -eq $Expected)
}

function Exit-With {
    param([int]$Code, [string]$Msg = "")
    if ($Msg) { Write-Err $Msg }
    Write-Host ""
    if ($Code -ne 0) { Read-Host "  Presiona Enter para salir" | Out-Null }
    exit $Code
}

# ============================================================================
# TOOLCHAIN CHECK
# ============================================================================

function Test-Toolchain {
    if ($SkipDepsCheck) {
        Write-Info "Saltando verificacion de toolchain (-SkipDepsCheck)"
        return $true
    }

    Write-Step "Verificando toolchain" "Cyan"

    # rustc
    try {
        $rustcVer = & rustc --version 2>&1
        Write-Sub "rustc: $rustcVer"
    } catch {
        Exit-With 1 "rustc no encontrado. Instala Rust: https://rustup.rs"
    }

    # nightly
    $hasNightly = (& rustup toolchain list 2>&1) -match "nightly"
    if (!$hasNightly) {
        Write-Warn "Toolchain 'nightly' no instalado. Instalando..."
        & rustup toolchain install nightly 2>&1 | Out-Null
    }
    Write-Sub "nightly: OK"

    # targets
    $installed = & rustup target list --installed --toolchain nightly 2>&1
    $needTargets = @("x86_64-unknown-none", "x86_64-unknown-uefi")
    foreach ($t in $needTargets) {
        if ($installed -notcontains $t) {
            Write-Warn "Target '$t' no instalado. Instalando..."
            & rustup target add $t --toolchain nightly 2>&1 | Out-Null
        }
    }
    Write-Sub "targets x86_64-unknown-none + x86_64-unknown-uefi: OK"

    # cargo
    $cargoVer = & cargo --version 2>&1
    Write-Sub "cargo: $cargoVer"

    Write-OK "Toolchain OK"
    return $true
}

# ============================================================================
# CLEAN
# ============================================================================

function Invoke-Clean {
    param([string]$What = "all")

    Write-Step "Limpiando artefactos" "Yellow"

    switch ($What) {
        "all" {
            $dirs = @($Script:TargetBuild, $Script:UsbBoot)
            foreach ($d in $dirs) {
                if (Test-Path $d) {
                    Write-Sub "Borrando $d"
                    Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue
                }
            }
        }
        "build" {
            if (Test-Path $Script:TargetBuild) {
                Write-Sub "Borrando $Script:TargetBuild"
                Remove-Item $Script:TargetBuild -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
        "usb" {
            if (Test-Path $Script:UsbBoot) {
                Write-Sub "Borrando $Script:UsbBoot"
                Remove-Item $Script:UsbBoot -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }

    # Root-level artifacts (que estan en .gitignore pero el script los regenera)
    $artifacts = @(
        (Join-Path $Script:Root $Script:EfiTarget),
        (Join-Path $Script:Root "kernel.elf"),
        (Join-Path $Script:Root "bmofs.img")
    )
    foreach ($a in $artifacts) {
        if (Test-Path $a) {
            Write-Sub "Borrando $a"
            Remove-Item $a -Force -ErrorAction SilentlyContinue
        }
    }

    Write-OK "Clean completo"
}

# ============================================================================
# BUILDERS
# ============================================================================

function Invoke-CargoBuild {
    param(
        [string]   $ManifestPath,
        [string]   $TargetDir,
        [string]   $Target       = "",
        [string]   $Label,
        [switch]   $Force
    )

    Write-Step "[BUILD] $Label" "Cyan"

    $targetFlag = if ($Target) { " --target $Target" } else { "" }
    $forceFlag  = if ($Force) { " --force" } else { "" }
    $cmd        = "rustup run nightly cargo build --release --target-dir `"$TargetDir`" --manifest-path `"$ManifestPath`"$targetFlag$forceFlag"

    Write-Sub "cmd: $cmd"

    # 1 intento + 1 retry (mitigacion contra file locks transitorios)
    $lastError = $null
    for ($attempt = 1; $attempt -le 2; $attempt++) {
        try {
            $output = & rustup run nightly cargo build --release `
                --target-dir $TargetDir `
                --manifest-path $ManifestPath `
                $(if ($Target) { "--target" } else { "" }) `
                $(if ($Target) { $Target } else { "" }) `
                $(if ($Force) { "--force" } else { "" }) `
                2>&1

            if ($LASTEXITCODE -eq 0) {
                if ($ShowAllOutput -or $output | Select-String -Pattern "warning|error") {
                    $output | ForEach-Object { Write-Host "          $_" -ForegroundColor DarkYellow }
                }
                Write-OK "${Label}: build OK"
                return $true
            }

            $output | ForEach-Object { Write-Host "          $_" -ForegroundColor Yellow }
            $lastError = "cargo fallo con exit code $LASTEXITCODE"
        } catch {
            $lastError = $_.Exception.Message
        }
        if ($attempt -eq 1) {
            Write-Warn "${Label}: intento 1 fallo -- reintentando..."
            Start-Sleep -Seconds 1
        }
    }

    Write-Err "${Label}: build FALLO -- $lastError"
    return $false
}

function Test-NeedsRebuild {
    param(
        [string]   $SourceDir,
        [string]   $ArtifactPath,
        [string[]] $ExtraWatch = @()
    )

    if (!(Test-Path $ArtifactPath)) { return $true }
    $artifactTime = Get-FileMTime $ArtifactPath

    # Si el manifest (Cargo.toml) o cualquier .rs o .toml en SourceDir es más nuevo
    $manifest = Join-Path $SourceDir "Cargo.toml"
    if (Test-FileNewer $manifest $artifactTime) { return $true }

    $changed = Get-ChildItem -Path $SourceDir -Recurse -Include "*.rs","*.toml","*.ld" -File -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTimeUtc -gt $artifactTime } |
        Select-Object -First 1
    if ($changed) {
        Write-Sub "Source mas nuevo: $($changed.FullName.Substring($SourceDir.Length))"
        return $true
    }
    foreach ($w in $ExtraWatch) {
        if (Test-FileNewer $w $artifactTime) { return $true }
    }
    return $false
}

function Build-Bootloader {
    param([switch]$Force)
    if (!(Test-NeedsRebuild -SourceDir $Script:BootloaderDir -ArtifactPath $Script:BootloaderOut -Force:$Force)) {
        Write-Info "bootloader: sin cambios, saltando"
        return $true
    }
    return Invoke-CargoBuild `
        -ManifestPath (Join-Path $Script:BootloaderDir "Cargo.toml") `
        -TargetDir   (Join-Path $Script:TargetBuild "bootloader") `
        -Target      "x86_64-unknown-uefi" `
        -Label       "bootloader" `
        -Force:$Force
}

function Build-Kernel {
    param([switch]$Force)
    if (!(Test-NeedsRebuild -SourceDir $Script:KernelDir -ArtifactPath $Script:KernelOut -Force:$Force)) {
        Write-Info "kernel: sin cambios, saltando"
        return $true
    }
    return Invoke-CargoBuild `
        -ManifestPath (Join-Path $Script:KernelDir "Cargo.toml") `
        -TargetDir   (Join-Path $Script:TargetBuild "kernel") `
        -Target      "x86_64-unknown-none" `
        -Label       "kernel" `
        -Force:$Force
}

function Build-Bmofs {
    param([switch]$Force)
    if (!(Test-NeedsRebuild -SourceDir $Script:BmofsDir -ArtifactPath $Script:BmofsOut -Force:$Force)) {
        Write-Info "bmofs: sin cambios, saltando"
        return $true
    }
    return Invoke-CargoBuild `
        -ManifestPath (Join-Path $Script:BmofsDir "Cargo.toml") `
        -TargetDir   (Join-Path $Script:TargetBuild "bmofs") `
        -Label       "bmofs" `
        -Force:$Force
}

# ============================================================================
# ARTIFACT STAGING (USB_boot/)
# ============================================================================

function Get-KernelVersion {
    # Lee la version declarada en kernel/Cargo.toml
    $toml = Join-Path $Script:KernelDir "Cargo.toml"
    if (Test-Path $toml) {
        $match = Select-String -Path $toml -Pattern '^version\s*=\s*"([^"]+)"' -AllMatches
        if ($match) { return $match.Matches[0].Groups[1].Value }
    }
    return "unknown"
}

function Get-BootloaderVersion {
    $toml = Join-Path $Script:BootloaderDir "Cargo.toml"
    if (Test-Path $toml) {
        $match = Select-String -Path $toml -Pattern '^version\s*=\s*"([^"]+)"' -AllMatches
        if ($match) { return $match.Matches[0].Value }
    }
    return "unknown"
}

function Invoke-Stage {
    Write-Step "Staging USB_boot/ desde artefactos" "Cyan"

    $bootloaderPath = $Script:BootloaderOut
    $kernelPath     = $Script:KernelOut
    $bmofsPath      = $Script:BmofsOut
    $bmofsImg       = Join-Path $Script:Root "bmofs.img"

    foreach ($a in @($bootloaderPath, $kernelPath)) {
        if (!(Test-Path $a)) {
            Exit-With 1 "Artefacto faltante: $a (corre el build primero)"
        }
    }

    # Limpiar USB_boot
    if (Test-Path $Script:UsbBoot) {
        Remove-Item $Script:UsbBoot -Recurse -Force -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Path $Script:UsbBoot -Force | Out-Null
    $efiBootPath = Join-Path $Script:UsbBoot "EFI\BOOT"
    New-Item -ItemType Directory -Path $efiBootPath -Force | Out-Null

    # Copiar bootloader como BOOTX64.EFI (2 ubicaciones: root + EFI/BOOT)
    Write-Sub "BOOTX64.EFI (bootloader)"
    Copy-Item $bootloaderPath (Join-Path $Script:UsbBoot $Script:EfiTarget) -Force
    Copy-Item $bootloaderPath (Join-Path $efiBootPath $Script:EfiTarget) -Force

    # Copiar kernel.elf (2 ubicaciones: root + EFI/BOOT)
    Write-Sub "kernel.elf"
    Copy-Item $kernelPath (Join-Path $Script:UsbBoot "kernel.elf") -Force
    Copy-Item $kernelPath (Join-Path $efiBootPath "kernel.elf") -Force

    # Tambien copiarlos a root (convenience)
    Write-Sub "Sincronizando root con staging"
    Copy-Item $bootloaderPath (Join-Path $Script:Root $Script:EfiTarget) -Force
    Copy-Item $kernelPath     (Join-Path $Script:Root "kernel.elf") -Force

    # bmofs.img (si existe)
    if (Test-Path $bmofsImg) {
        Write-Sub "bmofs.img"
        Copy-Item $bmofsImg (Join-Path $Script:UsbBoot "bmofs.img") -Force
    } elseif (Test-Path $bmofsPath) {
        Write-Warn "bmofs.exe existe pero bmofs.img no -- generando con bmofs (si hay firmware)"
    }

    # Build marker con SHA256
    $bootloaderHash = (Get-FileHash $bootloaderPath -Algorithm SHA256).Hash
    $kernelHash     = (Get-FileHash $kernelPath     -Algorithm SHA256).Hash
    $markerPath     = Join-Path $Script:UsbBoot "FASTOS_BUILD_MARKER.txt"
    $marker = @(
        "FastOS USB build marker"
        "Date=$((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))"
        "Script=v$Script:ScriptVersion"
        "Kernel.Version=$(Get-KernelVersion)"
        "Bootloader.Version=$(Get-BootloaderVersion)"
        "BOOTX64.EFI.Size=$((Get-Item $bootloaderPath).Length)"
        "BOOTX64.EFI.SHA256=$bootloaderHash"
        "kernel.elf.Size=$((Get-Item $kernelPath).Length)"
        "kernel.elf.SHA256=$kernelHash"
    )
    Set-Content -Path $markerPath -Encoding ASCII -Value $marker
    Write-Sub "FASTOS_BUILD_MARKER.txt escrito (SHA256 capturado)"

    Write-OK "Staging completo en $Script:UsbBoot"
}

# ============================================================================
# USB FLASH
# ============================================================================

function Get-TargetDisk {
    param([int]$DiskNumber)

    Write-Step "Seleccionando disco USB" "Cyan"

    if ($DiskNumber -lt 0) {
        Write-Info "Listando discos disponibles:"
        $disks = Get-Disk | Where-Object { $_.IsBoot -eq $false -or $_.IsBoot -eq $true } | Select-Object Number, FriendlyName, BusType, @{N="SizeGB";E={[math]::Round($_.Size/1GB,1)}}
        $disks | Format-Table -AutoSize | Out-String | ForEach-Object { Write-Host "          $_" }

        $sysDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
        Write-Warn "El disco del sistema es [$sysDisk] (C:). NO lo uses."
        $resp = Read-Host "  Numero de disco USB (ej: 2)"
        $DiskNumber = [int]$resp
    }

    $disk = Get-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
    if (!$disk) {
        Exit-With 1 "Disco $DiskNumber no encontrado"
    }

    # Safety: no permitir flashear el disco del sistema
    $sysDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
    if ($DiskNumber -eq $sysDisk) {
        Exit-With 1 "El disco $DiskNumber es el DISCO DEL SISTEMA. Cancelado por seguridad."
    }

    Write-Sub "Disco [$DiskNumber] $($disk.FriendlyName) -- $([math]::Round($disk.Size/1GB,1)) GB ($($disk.BusType))"
    return $disk
}

function Format-UsbGptFat32 {
    param($Disk)

    Write-Step "[FLASH 1/3] Formateando GPT + FAT32" "Cyan"

    $dl = $null
    try {
        Write-Sub "Limpiando disco..."
        $Disk | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false -ErrorAction Stop
        Write-Sub "Aplicando GPT..."
        $Disk | Set-Disk -PartitionStyle GPT -ErrorAction Stop
        Write-Sub "Creando particion EFI (ESP)..."
        $partition = $Disk | New-Partition -GptType '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}' -UseMaximumSize -AssignDriveLetter -ErrorAction Stop
        Write-Sub "Formateando FAT32..."
        $partition | Format-Volume -FileSystem FAT32 -NewFileSystemLabel "FastOS" -Confirm:$false -ErrorAction Stop
        Start-Sleep -Seconds 2
        $dl = (Get-Partition -DiskNumber $Disk.Number | Where-Object DriveLetter | Select-Object -First 1).DriveLetter
    } catch {
        Write-Warn "PowerShell fallo: $_"
        Write-Info "Reintentando con Diskpart..."
        $dl = Format-UsbGptFat32Diskpart $Disk.Number
    }

    if (!$dl) { Exit-With 1 "No se pudo obtener letra de unidad" }
    Write-OK "Unidad asignada: ${dl}:\"
    return "${dl}:\"
}

function Format-UsbGptFat32Diskpart {
    param([int]$DiskNumber)
    $dpScript = Join-Path $Script:Root "diskpart_temp.txt"
    @"
select disk $DiskNumber
clean
convert gpt
create partition efi
format fs=fat32 quick label="FastOS"
assign
"@ | Set-Content -Path $dpScript -Encoding ASCII

    $p = Start-Process diskpart.exe -ArgumentList "/s `"$dpScript`"" -NoNewWindow -PassThru -Wait
    Remove-Item $dpScript -Force -ErrorAction SilentlyContinue
    if ($p.ExitCode -ne 0) { return $null }

    Start-Sleep -Seconds 3
    $part = Get-Partition -DiskNumber $DiskNumber | Where-Object DriveLetter | Select-Object -First 1
    if ($part) { return $part.DriveLetter }
    $vol = Get-Volume | Where-Object { $_.FileSystemLabel -eq "FastOS" } | Select-Object -First 1
    if ($vol -and $vol.DriveLetter) { return $vol.DriveLetter }
    return $null
}

function Copy-ArtifactsToUsb {
    param([string]$DriveRoot)

    Write-Step "[FLASH 2/3] Copiando USB_boot -> $DriveRoot" "Cyan"

    if (!(Test-Path $Script:UsbBoot)) {
        Exit-With 1 "USB_boot/ no existe -- corre staging primero"
    }

    $efiBoot = Join-Path $DriveRoot "EFI\BOOT"
    New-Item -ItemType Directory -Path $efiBoot -Force | Out-Null

    # Copia recursiva de todo el staging
    Write-Sub "Copiando contenido de USB_boot/*"
    Copy-Item -Path "$Script:UsbBoot\*" -Destination $DriveRoot -Recurse -Force

    Write-OK "Copia completa"
}

function Test-UsbFlash {
    param([string]$DriveRoot)

    Write-Step "[FLASH 3/3] Verificando integridad" "Cyan"

    $required = @(
        @{ Name = "BOOTX64.EFI (EFI/BOOT)"; Path = (Join-Path $DriveRoot "EFI\BOOT\BOOTX64.EFI"); Orig = $Script:BootloaderOut },
        @{ Name = "BOOTX64.EFI (root)";     Path = (Join-Path $DriveRoot $Script:EfiTarget);    Orig = $Script:BootloaderOut },
        @{ Name = "kernel.elf (EFI/BOOT)";  Path = (Join-Path $DriveRoot "EFI\BOOT\kernel.elf");  Orig = $Script:KernelOut },
        @{ Name = "kernel.elf (root)";      Path = (Join-Path $DriveRoot "kernel.elf");        Orig = $Script:KernelOut },
        @{ Name = "FASTOS_BUILD_MARKER.txt"; Path = (Join-Path $DriveRoot "FASTOS_BUILD_MARKER.txt"); Orig = (Join-Path $Script:UsbBoot "FASTOS_BUILD_MARKER.txt") }
    )

    $ok = $true
    foreach ($c in $required) {
        if (!(Test-Path $c.Path)) {
            Write-Err "$($c.Name): no copiado"
            $ok = $false
            continue
        }
        if (!(Test-Path $c.Orig)) {
            Write-Warn "$($c.Name): origen no existe, saltando verificacion"
            continue
        }
        $cHash = (Get-FileHash $c.Path -Algorithm SHA256).Hash
        $oHash = (Get-FileHash $c.Orig -Algorithm SHA256).Hash
        if ($cHash -eq $oHash) {
            Write-Sub "$($c.Name): $([math]::Round((Get-Item $c.Path).Length/1024)) KB -- SHA256 OK"
        } else {
            Write-Err "$($c.Name): SHA256 no coincide"
            Write-Sub "  origen: $oHash"
            Write-Sub "  copia:  $cHash"
            $ok = $false
        }
    }

    if ($ok) { Write-OK "Verificacion: TODO OK" }
    else     { Write-Err "Verificacion: HUBO ERRORES" }
    return $ok
}

function Invoke-Flash {
    param([int]$DiskNumber, [switch]$Force)

    $disk = Get-TargetDisk -DiskNumber $DiskNumber
    $bootloaderSize = [math]::Round((Get-Item $Script:BootloaderOut).Length / 1024, 1)
    $kernelSize     = [math]::Round((Get-Item $Script:KernelOut).Length     / 1024, 1)
    $diskSizeGB     = [math]::Round($disk.Size / 1GB, 1)

    Write-Host ""
    Write-Host "  =================================================================" -ForegroundColor Yellow
    Write-Host "  RESUMEN FLASH" -ForegroundColor Yellow
    Write-Host "    BOOTX64.EFI  ${bootloaderSize} KB  (kernel.elf bootloader)" -ForegroundColor White
    Write-Host "    kernel.elf   ${kernelSize} KB  (FastOS v$(Get-KernelVersion))" -ForegroundColor White
    Write-Host "    Disco        [$($disk.Number)] $($disk.FriendlyName) -- $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
    Write-Host "  =================================================================" -ForegroundColor Yellow
    Write-Host "  ESTO FORMATEARA EL DISCO [$($disk.Number)] POR COMPLETO" -ForegroundColor Red
    Write-Host ""

    if (-not (Read-Confirm "Escribe FLASH para continuar (o Enter para cancelar)" "FLASH" -Force:$Force)) {
        Write-Warn "Cancelado por el usuario"
        return
    }

    $driveRoot = Format-UsbGptFat32 $disk
    Copy-ArtifactsToUsb $driveRoot

    $verified = Test-UsbFlash $driveRoot
    Show-FinalSummary $driveRoot $verified
}

function Show-FinalSummary {
    param([string]$DriveRoot, [bool]$Verified)

    $buildDate = Get-Date -Format 'yyyy-MM-dd HH:mm'

    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Green
    if ($Verified) {
        Write-Host "  FASTOS LISTO EN USB ($DriveRoot)" -ForegroundColor Green
    } else {
        Write-Host "  FLASH COMPLETADO CON ERRORES -- revisa arriba" -ForegroundColor Yellow
    }
    Write-Host "  Build:  $buildDate" -ForegroundColor Green
    Write-Host "  Kernel: v$(Get-KernelVersion)" -ForegroundColor Green
    Write-Host "  Script: v$Script:ScriptVersion" -ForegroundColor Green
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Contenido del USB:" -ForegroundColor White
    Write-Host "    ${DriveRoot}EFI\BOOT\BOOTX64.EFI  (bootloader UEFI)" -ForegroundColor White
    Write-Host "    ${DriveRoot}EFI\BOOT\kernel.elf    (kernel FastOS)" -ForegroundColor White
    Write-Host "    ${DriveRoot}kernel.elf              (copia en root)" -ForegroundColor White
    Write-Host "    ${DriveRoot}FASTOS_BUILD_MARKER.txt (build info + SHA256)" -ForegroundColor White
    Write-Host ""
    Write-Host "  Pasos para bootear:" -ForegroundColor Yellow
    Write-Host "    1. Reinicia el PC" -ForegroundColor White
    Write-Host "    2. BIOS: CSM = DISABLED, Secure Boot = DISABLED" -ForegroundColor White
    Write-Host "    3. Boot desde USB (UEFI only)" -ForegroundColor White
    Write-Host ""
    Write-Host "  Que esperas en pantalla:" -ForegroundColor Cyan
    Write-Host "    - Bootloader UEFI carga el kernel inmediatamente" -ForegroundColor White
    Write-Host "    - 5 fases de boot (CPU, Mem, Dev, Disp, Desk)" -ForegroundColor White
    Write-Host "    - Welcome screen v$(Get-KernelVersion) dark elegante" -ForegroundColor White
    Write-Host "    - Escribe Run -> BMO API v2.0 desktop real" -ForegroundColor White
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host ""

    if (-not $Force) {
        Read-Host "  Presiona Enter para cerrar" | Out-Null
    }
}

# ============================================================================
# MAIN FLOW
# ============================================================================

Write-Banner

# -- 1) Clean si se pidio ----------------------------------------------------
if ($Clean) {
    Invoke-Clean "all"
    if (!$BuildOnly) { exit 0 }
}

# -- 2) Verificar toolchain --------------------------------------------------
Test-Toolchain | Out-Null

# -- 3) Decidir alcance del build -------------------------------------------
$buildAll      = !$KernelOnly -and !$BootloaderOnly -and !$BmofsOnly
$doBootloader  = $buildAll -or $BootloaderOnly
$doKernel      = $buildAll -or $KernelOnly
$doBmofs       = $buildAll -or $BmofsOnly
$doClean       = $Rebuild -or $ForceRebuild

if ($doClean) { Invoke-Clean "build" }

# -- 4) Build ----------------------------------------------------------------
$buildOk = $true

if ($FlashOnly) {
    Write-Info "FlashOnly: saltando build, usando artefactos existentes"
} else {
    if ($doBootloader) { if (!(Build-Bootloader -Force:$ForceRebuild)) { $buildOk = $false } }
    if ($doKernel)     { if (!(Build-Kernel     -Force:$ForceRebuild)) { $buildOk = $false } }
    if ($doBmofs)      { if (!(Build-Bmofs      -Force:$ForceRebuild)) { $buildOk = $false } }

    if (!$buildOk) {
        Exit-With 1 "Build fallo. Corrige los errores y vuelve a intentar."
    }
}

# -- 5) Verificar que los artefactos requeridos existen ---------------------
if (!(Test-Path $Script:BootloaderOut)) { Exit-With 1 "bootloader.efi no encontrado" }
if (!(Test-Path $Script:KernelOut))     { Exit-With 1 "kernel.elf no encontrado" }

# -- 6) Staging USB_boot/ ----------------------------------------------------
Invoke-Stage

# -- 7) Flash ----------------------------------------------------------------
if ($BuildOnly) {
    Write-Host ""
    Write-Host "  =================================================================" -ForegroundColor Cyan
    Write-Host "  BUILD COMPLETO (sin flash)" -ForegroundColor Cyan
    Write-Host "    bootloader: $Script:BootloaderOut" -ForegroundColor White
    Write-Host "    kernel:     $Script:KernelOut" -ForegroundColor White
    Write-Host "    staging:    $Script:UsbBoot" -ForegroundColor White
    Write-Host "  =================================================================" -ForegroundColor Cyan
    if (-not $Force) { Read-Host "  Presiona Enter para cerrar" | Out-Null }
    exit 0
}

Invoke-Flash -DiskNumber $DiskNumber -Force:$Force
