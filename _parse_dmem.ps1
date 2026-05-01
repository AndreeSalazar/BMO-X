$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$descOff = 0x4A410

$storedSize = [BitConverter]::ToUInt32($vbios, $descOff + 4)
$imemLoadSize = [BitConverter]::ToUInt32($vbios, $descOff + 20)
$dmemLoadSize = [BitConverter]::ToUInt32($vbios, $descOff + 32)
$hdrSize = ([BitConverter]::ToUInt32($vbios, $descOff) -shr 16) -band 0xFFFF

Write-Output "StoredSize = 0x$([Convert]::ToString($storedSize, 16))"
Write-Output "IMEMLoadSize = 0x$([Convert]::ToString($imemLoadSize, 16))"
Write-Output "DMEMLoadSize = 0x$([Convert]::ToString($dmemLoadSize, 16))"
Write-Output "hdrSize = 0x$([Convert]::ToString($hdrSize, 16))"
Write-Output "Fallback DMEM = storedSize - imemLoadSize = 0x$([Convert]::ToString($storedSize - $imemLoadSize, 16))"

$dmemStart = $descOff + $hdrSize + $imemLoadSize
Write-Output ""
Write-Output "DMEM blob at SPI 0x$([Convert]::ToString($dmemStart, 16))"

# Dump AppIf header area (DMEM + 0x1C)
$ifaceOff = 0x1C
Write-Output ""
Write-Output "=== AppIf header at DMEM+0x$([Convert]::ToString($ifaceOff, 16)) ==="
$raw = [BitConverter]::ToString($vbios, $dmemStart + $ifaceOff, 32)
Write-Output "Raw 32 bytes: $raw"

$b0 = $vbios[$dmemStart + $ifaceOff]
$b1 = $vbios[$dmemStart + $ifaceOff + 1]
$b2 = $vbios[$dmemStart + $ifaceOff + 2]
$b3 = $vbios[$dmemStart + $ifaceOff + 3]
Write-Output "As u8: version=$b0 hdr_size=$b1 entry_size=$b2 count=$b3"

# Parse entries (u8 format: hdr at +hdr_size from header start)
$entryBase = $dmemStart + $ifaceOff + $b1
Write-Output ""
Write-Output "=== Entries (count=$b3, entry_size=$b2) ==="
for ($i = 0; $i -lt $b3; $i++) {
    $eOff = $entryBase + $i * $b2
    $id = [BitConverter]::ToUInt16($vbios, $eOff)
    $pad = [BitConverter]::ToUInt16($vbios, $eOff + 2)
    $dmemBase = [BitConverter]::ToUInt32($vbios, $eOff + 4)
    $raw = [BitConverter]::ToString($vbios, $eOff, $b2)
    Write-Output "  entry[$i] id=0x$([Convert]::ToString($id, 16)) pad=0x$([Convert]::ToString($pad, 16)) dmem_base=0x$([Convert]::ToString($dmemBase, 16)) raw=$raw"
    
    if ($id -eq 4) {
        Write-Output "    *** DMEMMAPPER FOUND ***"
        $mapOff = $dmemStart + $dmemBase
        Write-Output "    DMEMMAPPER struct at DMEM+0x$([Convert]::ToString($dmemBase, 16))"
        $mapRaw = [BitConverter]::ToString($vbios, $mapOff, 24)
        Write-Output "    Raw 24 bytes: $mapRaw"
        
        $cmdLineSize = [BitConverter]::ToUInt32($vbios, $mapOff)
        $cmdInBufOff = [BitConverter]::ToUInt32($vbios, $mapOff + 4)
        $cmdOutBufOff = [BitConverter]::ToUInt32($vbios, $mapOff + 8)
        $cmdLineOff = [BitConverter]::ToUInt32($vbios, $mapOff + 12)
        $initCmd = [BitConverter]::ToUInt32($vbios, $mapOff + 16)
        
        Write-Output "    cmd_line_size=0x$([Convert]::ToString($cmdLineSize, 16))"
        Write-Output "    cmd_in_buffer_offset=0x$([Convert]::ToString($cmdInBufOff, 16))"
        Write-Output "    cmd_out_buffer_offset=0x$([Convert]::ToString($cmdOutBufOff, 16))"
        Write-Output "    cmd_line_offset=0x$([Convert]::ToString($cmdLineOff, 16))"
        Write-Output "    init_cmd=0x$([Convert]::ToString($initCmd, 16))"
    }
}
