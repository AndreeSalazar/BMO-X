# ============================================================================
# FastOS — Flash to USB (raw LBA 0, auto-elevates to Admin)
# ============================================================================
# Usage:  .\flash_direct.ps1
#         .\flash_direct.ps1 -DiskNumber 2
#
# If no DiskNumber given, auto-detects USB drives and asks you to pick.
# Always verifies after writing. Auto-elevates to Administrator.
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

# ── Find image ───────────────────────────────────────────────────────────────
$ImagePath = "$Root\fastos.img"
if (!(Test-Path $ImagePath)) {
    Write-Host "ERROR: fastos.img no encontrado en $Root" -ForegroundColor Red
    Write-Host "  Ejecuta build.ps1 primero." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 1
}
$imgData = [System.IO.File]::ReadAllBytes($ImagePath)
$imgSize = $imgData.Length

# Verify MBR signature
if ($imgData.Length -ge 512) {
    $sig = [BitConverter]::ToUInt16($imgData, 510)
    if ($sig -ne 0xAA55) {
        Write-Host "ERROR: La imagen no tiene firma MBR valida (0xAA55)" -ForegroundColor Red
        Read-Host "  Presiona Enter para salir"
        exit 1
    }
}

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
Write-Host "  FastOS USB Flash (Raw LBA 0)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Imagen : fastos.img ($([int]($imgSize/1024)) KB)" -ForegroundColor White
Write-Host "  Disco  : [$DiskNumber] $($disk.FriendlyName)" -ForegroundColor White
Write-Host "  Tamano : $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
Write-Host "  Modo   : RAW a LBA 0 (MBR directo)" -ForegroundColor White
Write-Host ""
Write-Host "  ESTO VA A SOBREESCRIBIR EL DISCO $DiskNumber" -ForegroundColor Red
Write-Host ""
$confirm = Read-Host "  Escribe FLASH para continuar"
if ($confirm -ne "FLASH") {
    Write-Host "  Cancelado." -ForegroundColor Yellow
    Read-Host "  Presiona Enter para salir"
    exit 0
}

# ── Dismount all volumes on this disk ────────────────────────────────────────
Write-Host ""
Write-Host "[1/3] Preparando disco..." -ForegroundColor Cyan

try {
    # Set disk offline then online to release locks
    Set-Disk -Number $DiskNumber -IsOffline $true -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
    Set-Disk -Number $DiskNumber -IsOffline $false -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
} catch {
    Write-Host "      (disk prep warning: $($_.Exception.Message))" -ForegroundColor DarkGray
}

Write-Host "[1/3] Listo" -ForegroundColor Green

# ── Write image at LBA 0 ────────────────────────────────────────────────────
Write-Host "[2/3] Escribiendo fastos.img -> Disco $DiskNumber (LBA 0)..." -ForegroundColor Cyan

$diskPath = "\\.\PhysicalDrive$DiskNumber"
$handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)

try {
    $handle.Seek(0, [System.IO.SeekOrigin]::Begin) | Out-Null

    $chunkSize = 64 * 1024
    $written = 0
    while ($written -lt $imgData.Length) {
        $remaining = $imgData.Length - $written
        $toWrite = [math]::Min($chunkSize, $remaining)
        $handle.Write($imgData, $written, $toWrite)
        $written += $toWrite
        $pct = [math]::Round(($written / $imgData.Length) * 100)
        Write-Host "`r      Writing: $pct% ($([int]($written/1024)) KB / $([int]($imgSize/1024)) KB)" -NoNewline -ForegroundColor Gray
    }
    $handle.Flush()
    Write-Host ""
    Write-Host "      Written: $written bytes" -ForegroundColor DarkGray
}
finally {
    $handle.Close()
}

Write-Host "[2/3] Escritura OK" -ForegroundColor Green

# ── Verify (always) ──────────────────────────────────────────────────────────
Write-Host "[3/3] Verificando escritura (read-back)..." -ForegroundColor Cyan

$handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
try {
    $handle.Seek(0, [System.IO.SeekOrigin]::Begin) | Out-Null
    $readBack = New-Object byte[] $imgData.Length
    $totalRead = 0
    while ($totalRead -lt $imgData.Length) {
        $bytesRead = $handle.Read($readBack, $totalRead, [math]::Min(65536, $imgData.Length - $totalRead))
        if ($bytesRead -eq 0) { break }
        $totalRead += $bytesRead
    }

    $ok = $true
    $mismatchByte = -1
    for ($i = 0; $i -lt $imgData.Length; $i++) {
        if ($imgData[$i] -ne $readBack[$i]) {
            $mismatchByte = $i
            $ok = $false
            break
        }
    }

    if ($ok) {
        Write-Host "      VERIFICADO OK: $totalRead bytes coinciden perfectamente" -ForegroundColor Green
        # Extra: confirm MBR signature on disk
        $diskSig = [BitConverter]::ToUInt16($readBack, 510)
        Write-Host "      MBR en disco: 0x$($diskSig.ToString('X4')) $(if($diskSig -eq 0xAA55){'OK'}else{'ERROR'})" -ForegroundColor Green
        Write-Host "      Stage2 byte 0: 0x$($readBack[512].ToString('X2')) $(if($readBack[512] -eq 0xE9){'(JMP) OK'}else{'ERROR'})" -ForegroundColor Green
    } else {
        Write-Host "      ERROR: Byte $mismatchByte no coincide!" -ForegroundColor Red
        Write-Host "      Esperado: 0x$($imgData[$mismatchByte].ToString('X2'))  Leido: 0x$($readBack[$mismatchByte].ToString('X2'))" -ForegroundColor Red
        Write-Host "      El USB puede estar defectuoso o protegido contra escritura." -ForegroundColor Yellow
    }
}
finally {
    $handle.Close()
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
Write-Host "    3. Boot -> Legacy/CSM -> USB primero" -ForegroundColor White
Write-Host "    4. Guardar y reiniciar" -ForegroundColor White
Write-Host ""
Read-Host "  Presiona Enter para cerrar"
