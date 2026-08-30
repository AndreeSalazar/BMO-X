# EL UNICO SITIO DEL BUILD QUE ESCRIBE FUERA DEL ARBOL DEL PROYECTO.
#
# ** Por que esto es un fichero y no un parrafo de `build.ps1` (2026-08-28)
#
# Porque L6e no elige el corte solo por el tamano: elige tambien por LO QUE
# CUESTA EQUIVOCARSE. Y aqui equivocarse no es un build rojo -- es escribir en
# el disco que no era. Todo lo demas que hace este build se deshace borrando
# `staging`; esto no.
#
# Lo que hay dentro son las dos mitades del despliegue, separadas a proposito:
#
#     -Flash   toca la ESP de ARRANQUE          (el volumen del firmware)
#     -Data    toca BMO-DATA, los PROGRAMAS     (el volumen FAT32)
#
# Cada una lleva sus tres cierres antes de copiar un byte: no puede ser el
# disco del sistema, tiene que ser el tipo de volumen correcto, y hay que
# teclear la frase entera con la letra dentro.
#
# [!] Se carga con punto, asi que corre EN EL AMBITO DE `build.ps1` y ve sus
# variables --`$root`, `$dataBase`, `$Yes`-- y sus funciones --`Step`, `Fail`,
# `Hash256`, `Espejo`--. Es el mismo texto en el mismo orden: lo unico que
# cambio de sitio es el fichero.

# -- Flash --------------------------------------------------------
if ($Flash -or $Verify) {
    $targetLetter = $Drive.TrimEnd([char]':',[char]'\').ToUpper()
    if ($targetLetter -notmatch '^[A-Z]$') { Fail ('Invalid drive letter: ' + $Drive) }
    $systemLetter = $env:SystemDrive.TrimEnd([char]':',[char]'\').ToUpper()
    if ($targetLetter -eq $systemLetter) { Fail 'Refusing to deploy BMO onto the Windows system volume' }
    $targetRoot = ($targetLetter + ':\')
    if (-not (Test-Path $targetRoot)) { Fail ('Drive ' + $targetLetter + ' not found') }

    $volume = Get-Volume -DriveLetter $targetLetter -ErrorAction SilentlyContinue
    if ($volume) {
        $sizeGiB = [math]::Round($volume.Size / 1GB, 1)
        Write-Host ('  Target: {0}: label="{1}" filesystem={2} size={3} GiB' -f `
            $targetLetter, $volume.FileSystemLabel, $volume.FileSystem, $sizeGiB) -ForegroundColor Yellow
        if ($volume.FileSystem -notin @('FAT', 'FAT32')) {
            Fail ('UEFI boot requires a FAT/FAT32 ESP; ' + $targetLetter + ': is ' + $volume.FileSystem)
        }
    }

    $efiDest = Join-Path $targetRoot (Join-Path 'EFI' 'BOOT')
    if ($Flash) {
        if (-not $Yes) {
            $expected = 'FLASH ' + $targetLetter + ' BMO'
            $confirmation = Read-Host ('  Type "' + $expected + '" to update Ring 0')
            if ($confirmation -ne $expected) { Write-Host '  Aborted.'; exit 0 }
        }

        Step ('Deploying Ring 0 to ' + $targetLetter + ':\EFI\BOOT')
        New-Item -ItemType Directory -Path $efiDest -Force | Out-Null
        $nextDest = Join-Path $efiDest ('.bmo-next-' + $PID)
        if (Test-Path $nextDest) { Remove-Item -LiteralPath $nextDest -Recurse -Force }
        New-Item -ItemType Directory -Path $nextDest -Force | Out-Null
        Copy-Item (Join-Path $stage '*') -Destination $nextDest -Recurse -Force

        foreach ($relativePath in $deployFiles) {
            $expectedHash = Hash256 (Join-Path $stage $relativePath)
            $nextPath = Join-Path $nextDest $relativePath
            if (-not (Test-Path -LiteralPath $nextPath) -or (Hash256 $nextPath) -ne $expectedHash) {
                Remove-Item -LiteralPath $nextDest -Recurse -Force -ErrorAction SilentlyContinue
                Fail ('Staged SSD copy failed verification: ' + $relativePath)
            }
        }

        # LIMPIEZA del destino: los subarboles ring0\ y ring3\ que dejaban las
        # versiones anteriores ya no se generan (eran el duplicado de lo que
        # BOOTX64.EFI lleva embebido). Si siguen en el disco, se borran: un
        # deploy tiene que dejar el destino como el build, no acumular restos
        # de builds pasados que nadie lee pero que confunden al mirarlos.
        foreach ($stale in @('ring0', 'ring3')) {
            $stalePath = Join-Path $efiDest $stale
            if (Test-Path $stalePath) {
                Write-Host ('    limpiando resto de deploys viejos: ' + $stale + '\') -ForegroundColor DarkYellow
                Remove-Item -LiteralPath $stalePath -Recurse -Force
            }
        }
        Copy-Item -LiteralPath (Join-Path $nextDest 'BOOTX64.EFI') -Destination (Join-Path $efiDest 'BOOTX64.EFI') -Force
        Copy-Item -LiteralPath (Join-Path $nextDest 'BMO-MANIFEST.TXT') -Destination (Join-Path $efiDest 'BMO-MANIFEST.TXT') -Force
        Remove-Item -LiteralPath $nextDest -Recurse -Force
    }

    Step ('Verifying ' + $targetLetter + ':\EFI\BOOT against the current build')
    foreach ($relativePath in $deployFiles) {
        $sourcePath = Join-Path $stage $relativePath
        $destinationPath = Join-Path $efiDest $relativePath
        if (-not (Test-Path -LiteralPath $destinationPath)) { Fail ('Missing on SSD: ' + $relativePath) }
        if ((Get-Item -LiteralPath $destinationPath).Length -ne (Get-Item -LiteralPath $sourcePath).Length) {
            Fail ('Size mismatch on SSD: ' + $relativePath)
        }
        if ((Hash256 $destinationPath) -ne (Hash256 $sourcePath)) { Fail ('SHA-256 mismatch on SSD: ' + $relativePath) }
        Write-Host ('    SHA-256 OK  ' + $relativePath) -ForegroundColor Green
    }

    Write-Host ''
    Write-Host '  === BMO RING 0 SSD VERIFIED ===' -ForegroundColor Green
    if ($Flash) { Write-Host '  Reboot from the SSD to test the new kernel.' -ForegroundColor White }
    Write-Host ''
}

# -- Data: los programas de Ring 3 al volumen de datos -------------
#
# Separado de -Flash a proposito. Son dos discos distintos con dos riesgos
# distintos: -Flash toca la ESP de arranque, esto toca BMO-DATA. Que compartan
# bandera invitaria a escribir en uno cuando se queria el otro.
#
# * Este es el UNICO sitio del build que escribe fuera del arbol del proyecto.
# Por eso lleva tres cierres antes de copiar un byte: no puede ser el disco del
# sistema, tiene que ser FAT/FAT32, y hay que teclear la frase entera. El NVMe
# de esta maquina lleva un Windows que no es de BMO.
if ($Data) {
    $dataLetter = $Data.TrimEnd([char]':',[char]'\').ToUpper()
    if ($dataLetter.Length -ne 1) { Fail ('-Data espera UNA letra de unidad, no: ' + $Data) }
    $dataRoot = $dataLetter + ':\'

    # Cierre 1: nunca el disco del sistema. Es el que tiene el Windows del
    # dueno de la maquina, y una copia ahi no es un error recuperable.
    $sistema = ($env:SystemDrive).TrimEnd([char]':').ToUpper()
    if ($dataLetter -eq $sistema) {
        Fail ('NO: ' + $dataLetter + ': es el disco del sistema (' + $env:SystemDrive + '). BMO no escribe ahi.')
    }
    if (-not (Test-Path $dataRoot)) { Fail ('no existe la unidad ' + $dataRoot) }

    # Cierre 2: tiene que ser el tipo de volumen correcto, y se ENSENA cual es
    # antes de preguntar. Una confirmacion a ciegas no es una confirmacion.
    $dataVol = Get-Volume -DriveLetter $dataLetter -ErrorAction SilentlyContinue
    if ($dataVol) {
        $dSize = [math]::Round($dataVol.Size / 1GB, 1)
        Write-Host ''
        Write-Host ('  Destino de datos: {0}: label="{1}" filesystem={2} size={3} GiB' -f `
            $dataLetter, $dataVol.FileSystemLabel, $dataVol.FileSystem, $dSize) -ForegroundColor Yellow
        if ($dataVol.FileSystem -notin @('FAT', 'FAT32')) {
            Fail ('el volumen de datos de BMO es FAT32; ' + $dataLetter + ': es ' + $dataVol.FileSystem)
        }
    }

    # Cierre 3: la frase, igual que -Flash. Con la letra dentro, para que
    # copiar-pegar la de otra sesion no valga.
    if (-not $Yes) {
        $esperado = 'DATA ' + $dataLetter + ' BMO'
        $conf = Read-Host ('  Escribe "' + $esperado + '" para copiar los programas de Ring 3')
        if ($conf -ne $esperado) { Write-Host '  Abortado.'; exit 0 }
    }

    # * Ya no es una sola carpeta: van a sys\ cobol\ c\ ada\ datos\. El bucle de
    # abajo copia recursivo y crea los directorios que falten, asi que no hay
    # nada que cambiar aqui salvo el mensaje -- pero **la vieja `apps\` del disco
    # NO se borra**: este deploy no borra nada que no haya puesto el. Hay que
    # quitarla a mano una vez, o quedan dos copias de cada programa.
    Step ('Copiando programas de Ring 3 a ' + $dataLetter + ':\  (sys, cobol, c, ada, datos)')
    $dataSrc = Join-Path $root 'staging\BMO-DATA'
    if (-not (Test-Path $dataSrc)) { Fail 'no hay staging\BMO-DATA (ejecuta el build primero)' }

    # Se copia SOLO lo que este build produjo, archivo a archivo. Nada de
    # borrar el destino: en BMO-DATA puede haber cosas que no salen de aqui, y
    # un deploy no tiene derecho a decidir sobre ellas.
    $copiados = 0
    foreach ($f in Get-ChildItem -Path $dataSrc -Recurse -File) {
        $rel = $f.FullName.Substring($dataSrc.Length).TrimStart([char]'\')
        $dst = Join-Path $dataRoot $rel
        New-Item -ItemType Directory -Path (Split-Path -Parent $dst) -Force | Out-Null
        Copy-Item -LiteralPath $f.FullName -Destination $dst -Force
        # Verificado por hash, como Ring 0. Un .bex a medio copiar no falla al
        # arrancar: falla en la admision BEX, y ese mensaje manda a buscar el
        # bug al compilador en vez de al cable.
        if ((Hash256 $dst) -ne (Hash256 $f.FullName)) { Fail ('copia corrupta: ' + $rel) }
        Write-Host ('    SHA-256 OK  ' + $rel) -ForegroundColor Green
        $copiados++
    }
    if ($copiados -eq 0) { Fail 'staging\BMO-DATA esta vacio' }

    Write-Host ''
    Write-Host ('  === BMO-DATA VERIFICADO (' + $copiados + ' archivos) ===') -ForegroundColor Green
    Write-Host ''
}
