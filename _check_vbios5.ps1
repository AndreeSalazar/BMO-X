# Focused BIT token analysis with correct offset math
$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

# Use FIRST ROM copy at 0x9200 (not 0xE9200)
$romBase = 0x9200
$bitOff = 0x93b2  # BIT\0 sig offset (absolute)

Write-Output "ROM base: 0x$([Convert]::ToString($romBase, 16))"
Write-Output "BIT sig at: 0x$([Convert]::ToString($bitOff, 16))"
Write-Output ""

# Raw dump around BIT
Write-Output "=== Raw bytes around BIT ==="
for ($row = -2; $row -le 20; $row++) {
    $off = $bitOff + $row * 16
    if ($off -ge 0 -and $off + 16 -le $vbios.Length) {
        $hex = [System.BitConverter]::ToString($vbios, $off, 16)
        Write-Output ("  " + ("+{0:D3}" -f ($row * 16)) + " (0x$([Convert]::ToString($off, 16))): " + $hex)
    }
}

Write-Output ""
Write-Output "=== BIT Header Parse ==="
# Bytes at BIT: 42-49-54-00-00-01-0C-06-12-45
# "BIT\0" = 42 49 54 00
# Then: 00 01 = bcd_ver 0x0100
# 0C = hdr_size = 12 (from start of "BIT\0")
# 06 = token_size = 6
# 12 = token_count = 18 (decimal)
# 45 = checksum

$hdrSize = $vbios[$bitOff + 6]  # 12
$tokenSize = $vbios[$bitOff + 7]  # 6
$tokenCount = $vbios[$bitOff + 8]  # 18

Write-Output "hdr_size=$hdrSize token_size=$tokenSize token_count=$tokenCount"
Write-Output ""

# First token at bitOff + hdrSize = 0x93b2 + 12 = 0x93BE
$firstTok = $bitOff + $hdrSize
Write-Output ("First token at: 0x$([Convert]::ToString($firstTok, 16))")
Write-Output ""

# Parse each 6-byte token: id(1) data_version(1) data_size(2) data_ptr(2)
Write-Output "=== Tokens (6 bytes each) ==="
for ($t = 0; $t -lt $tokenCount; $t++) {
    $tOff = $firstTok + $t * $tokenSize
    $raw6 = [System.BitConverter]::ToString($vbios, $tOff, 6)
    
    $tId = $vbios[$tOff]
    $tDv = $vbios[$tOff + 1]
    $tSz = [BitConverter]::ToUInt16($vbios, $tOff + 2)
    $tPtr = [BitConverter]::ToUInt16($vbios, $tOff + 4)
    
    $tChar = if ($tId -ge 0x20 -and $tId -le 0x7E) { [char]$tId } else { ("{0:X2}" -f $tId) }
    
    # Resolve pointer: BIT data pointers are offsets from ROM image base
    $absPtr = $romBase + $tPtr
    
    Write-Output ("  [$t] '$tChar' (0x$([Convert]::ToString($tId, 16))): dv=$tDv sz=$tSz ptr=0x$([Convert]::ToString($tPtr, 16)) -> abs=0x$([Convert]::ToString($absPtr, 16))  raw=$raw6")
    
    # Show data at the pointer for interesting tokens
    if ($tPtr -ne 0 -and $tSz -gt 0 -and $absPtr + [Math]::Min($tSz, 32) -lt $vbios.Length) {
        $dataRaw = [System.BitConverter]::ToString($vbios, $absPtr, [Math]::Min($tSz, 32))
        Write-Output ("        data: $dataRaw")
    }
}

# Try alternative: maybe hdr_size includes the 0xB8FF prefix?
# So first token at (bitOff - 2) + 12 = bitOff + 10
Write-Output ""
Write-Output "=== Alt: Tokens starting at bitOff+10 (accounting for 0xB8FF) ==="
$firstTokAlt = $bitOff + 10  # 0x93BC
for ($t = 0; $t -lt 18; $t++) {
    $tOff = $firstTokAlt + $t * 6
    $raw6 = [System.BitConverter]::ToString($vbios, $tOff, 6)
    
    $tId = $vbios[$tOff]
    $tDv = $vbios[$tOff + 1]
    $tSz = [BitConverter]::ToUInt16($vbios, $tOff + 2)
    $tPtr = [BitConverter]::ToUInt16($vbios, $tOff + 4)
    
    $tChar = if ($tId -ge 0x20 -and $tId -le 0x7E) { [char]$tId } else { ("{0:X2}" -f $tId) }
    
    $absPtr = $romBase + $tPtr
    
    Write-Output ("  [$t] '$tChar' (0x$([Convert]::ToString($tId, 16))): dv=$tDv sz=$tSz ptr=0x$([Convert]::ToString($tPtr, 16)) -> abs=0x$([Convert]::ToString($absPtr, 16))  raw=$raw6")
    
    if ($tPtr -ne 0 -and $tSz -gt 0 -and $absPtr + [Math]::Min($tSz, 32) -lt $vbios.Length) {
        $dataRaw = [System.BitConverter]::ToString($vbios, $absPtr, [Math]::Min($tSz, 32))
        Write-Output ("        data: $dataRaw")
    }
}
