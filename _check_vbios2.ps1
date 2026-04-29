# Deep analysis of vbios_rtx3060.rom for FWSEC extraction
$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

Write-Output "=== VBIOS Deep Analysis ==="
Write-Output "Total size: $($vbios.Length) bytes"
Write-Output ""

# 1. This is NOT a standard PCI ROM - it starts with 'NVGI' (0x4E 0x56 0x47 0x49)
$magic = [System.Text.Encoding]::ASCII.GetString($vbios, 0, 4)
Write-Output "Magic: '$magic' (0x$([System.BitConverter]::ToString($vbios, 0, 4)))"
Write-Output "  NOTE: This is NVIDIA's native VBIOS format, NOT PCI expansion ROM (55 AA)"
Write-Output ""

# 2. Look at the nvfw_bin_hdr we found at 0x1c0e24
$off = 0x1c0e24
Write-Output "=== nvfw_bin_hdr at 0x$([Convert]::ToString($off, 16)) ==="
$magic = [BitConverter]::ToUInt32($vbios, $off)
$hdr_ver = [BitConverter]::ToUInt32($vbios, $off+4)
$hdr_size = [BitConverter]::ToUInt32($vbios, $off+8)
$hdr_off = [BitConverter]::ToUInt32($vbios, $off+12)
$data_off = [BitConverter]::ToUInt32($vbios, $off+16)
$data_sz = [BitConverter]::ToUInt32($vbios, $off+20)
Write-Output "  magic=0x$([Convert]::ToString($magic, 16)) ver=0x$([Convert]::ToString($hdr_ver, 16))"
Write-Output "  hdr_size=$hdr_size hdr_off=0x$([Convert]::ToString($hdr_off, 16))"
Write-Output "  data_off=0x$([Convert]::ToString($data_off, 16)) data_sz=0x$([Convert]::ToString($data_sz, 16))"
Write-Output ""

# 3. BIT table analysis - this is how nouveau finds the FWSEC ucode
$bitOff = 0x93b2
Write-Output "=== BIT Table at 0x$([Convert]::ToString($bitOff, 16)) ==="
# BIT header: 'B' 'I' 'T' 0x00 version entries_count header_size
$bitMagic = [System.Text.Encoding]::ASCII.GetString($vbios, $bitOff, 3)
$bitVer = $vbios[$bitOff + 4]
$bitHdrSz = $vbios[$bitOff + 5]
Write-Output "  Magic: '$bitMagic' Ver: $bitVer HdrSize: $bitHdrSz"

# BIT entries start after header
$entryOff = $bitOff + $bitHdrSz
$maxEntries = 50
Write-Output ""
Write-Output "=== BIT Entries ==="
for ($e = 0; $e -lt $maxEntries; $e++) {
    if ($entryOff + 6 -ge $vbios.Length) { break }
    $type = $vbios[$entryOff]
    $ver = $vbios[$entryOff + 1]
    $len = [BitConverter]::ToUInt16($vbios, $entryOff + 2)
    $ptr = [BitConverter]::ToUInt16($vbios, $entryOff + 4)
    
    if ($type -eq 0 -and $ver -eq 0 -and $len -eq 0) { break }
    
    $typeChar = if ($type -ge 0x20 -and $type -le 0x7E) { [char]$type } else { '?' }
    Write-Output "  BIT '$typeChar' (0x$([Convert]::ToString($type, 16))): ver=$ver len=$len ptr=0x$([Convert]::ToString($ptr, 16))"
    
    # Look for 'F' entries (Falcon data)
    if ($type -eq [byte][char]'F') {
        Write-Output "    *** FALCON DATA ENTRY ***"
        if ($ptr -ne 0 -and $ptr + $len -le $vbios.Length) {
            $fdata = [System.BitConverter]::ToString($vbios, $ptr, [Math]::Min(32, $len))
            Write-Output "    Data at 0x$([Convert]::ToString($ptr, 16)): $fdata"
        }
    }
    
    # 'B' = Bootloader/BIOS data
    if ($type -eq [byte][char]'B') {
        Write-Output "    *** BIOS DATA ENTRY ***"
    }
    
    $entryOff += 6
}

# 4. Search ALL nvfw_bin_hdr (0x10DE) occurrences
Write-Output ""
Write-Output "=== All nvfw_bin_hdr (0x10DE) occurrences ==="
$count = 0
for ($i = 0; $i -lt $vbios.Length - 24; $i += 4) {
    if ($vbios[$i] -eq 0xDE -and $vbios[$i+1] -eq 0x10 -and $vbios[$i+2] -eq 0x00 -and $vbios[$i+3] -eq 0x00) {
        $hdr_off2 = [BitConverter]::ToUInt32($vbios, $i+12)
        $data_off2 = [BitConverter]::ToUInt32($vbios, $i+16)
        $data_sz2 = [BitConverter]::ToUInt32($vbios, $i+20)
        Write-Output "  [${count}] at 0x$([Convert]::ToString($i, 16)): hdr_off=0x$([Convert]::ToString($hdr_off2, 16)) data_off=0x$([Convert]::ToString($data_off2, 16)) data_sz=0x$([Convert]::ToString($data_sz2, 16))"
        $count++
    }
}

# 5. fwsec_offset.bin is all zeros - this is the problem!
Write-Output ""
Write-Output "=== fwsec_offset.bin ==="
Write-Output "PROBLEM: fwsec_offset.bin is all zeros (8 bytes of 0x00)"
Write-Output "This means no valid FWSEC offset was extracted from the VBIOS"

# 6. Search for Falcon ucode headers (the actual FWSEC binary embedded in VBIOS)
# In nouveau, FWSEC is found via nvbios_falcon_lookup() which uses BIT 'B' table entry
Write-Output ""
Write-Output "=== Searching for potential Falcon ucode segments ==="
# Falcon IMEM code typically starts with specific patterns
# Look for the "nv_fw" signature pattern that nouveau uses
for ($i = 0; $i -lt $vbios.Length - 8; $i += 4) {
    $v = [BitConverter]::ToUInt32($vbios, $i)
    # Falcon code header magic (from nv50_firmware.h) - look for HSFW_HEADER magic
    if ($v -eq 0x00010001 -or $v -eq 0x00010002 -or $v -eq 0x00020001) {
        $next4 = [BitConverter]::ToUInt32($vbios, $i+4)
        if ($next4 -ne 0 -and $next4 -lt $vbios.Length) {
            Write-Output "  Potential Falcon hdr at 0x$([Convert]::ToString($i, 16)): type=0x$([Convert]::ToString($v, 16)) next=0x$([Convert]::ToString($next4, 16))"
        }
    }
}
