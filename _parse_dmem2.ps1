$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$dmemStart = 0x58ABC
$mapOff = $dmemStart + 0x560

Write-Output "DMEMMAPPER raw 64 bytes:"
$raw = [BitConverter]::ToString($vbios, $mapOff, 64)
Write-Output $raw

Write-Output ""
for ($i = 0; $i -lt 16; $i++) {
    $off = $mapOff + $i * 4
    $val = [BitConverter]::ToUInt32($vbios, $off)
    $hex = [Convert]::ToString($val, 16).PadLeft(8, '0')
    $offHex = [Convert]::ToString($i * 4, 16).PadLeft(2, '0')
    Write-Output "+0x${offHex}: 0x${hex}"
}

Write-Output ""
$magic = [System.Text.Encoding]::ASCII.GetString($vbios, $mapOff, 4)
Write-Output "ASCII magic: $magic"

# Interpretation: DMAP header then fields
Write-Output ""
Write-Output "=== If DMAP struct (magic + version + fields) ==="
$ver = [BitConverter]::ToUInt16($vbios, $mapOff + 4)
$hdr = [BitConverter]::ToUInt16($vbios, $mapOff + 6)
Write-Output "  version=$ver hdr_size=$hdr"

# Fields after header
$cmdInBuf = [BitConverter]::ToUInt32($vbios, $mapOff + $hdr)
$cmdOutBuf = [BitConverter]::ToUInt32($vbios, $mapOff + $hdr + 4)
Write-Output "  field[0] (at +$hdr) = 0x$([Convert]::ToString($cmdInBuf, 16))"
Write-Output "  field[1] (at +$($hdr+4)) = 0x$([Convert]::ToString($cmdOutBuf, 16))"

# Alternative: skip magic, parse as { magic(4), ver(u8), hdr(u8), ... }
Write-Output ""
Write-Output "=== Alternative: magic(4) + u8 fields ==="
$v2 = $vbios[$mapOff + 4]
$h2 = $vbios[$mapOff + 5]
Write-Output "  byte[4]=$v2 byte[5]=$h2"

# Try reading cmd_in_buffer at various known offsets
Write-Output ""
Write-Output "=== Scanning for cmd_in_buffer_offset value 0x7C0 in DMEMMAPPER region ==="
for ($i = 0; $i -lt 64; $i += 4) {
    $val = [BitConverter]::ToUInt32($vbios, $mapOff + $i)
    if ($val -eq 0x7C0) {
        Write-Output "  Found 0x7C0 at DMEMMAPPER+0x$([Convert]::ToString($i, 16))"
    }
}

# Also check init_cmd = where should we write 0x15?
Write-Output ""
Write-Output "=== Scanning for init_cmd location (looking for 0x00000000 after cmd offsets) ==="
Write-Output "  DMEMMAPPER+0x08 = 0x$([Convert]::ToString([BitConverter]::ToUInt32($vbios, $mapOff+8), 16))"
Write-Output "  DMEMMAPPER+0x0C = 0x$([Convert]::ToString([BitConverter]::ToUInt32($vbios, $mapOff+12), 16))"
Write-Output "  DMEMMAPPER+0x10 = 0x$([Convert]::ToString([BitConverter]::ToUInt32($vbios, $mapOff+16), 16))"
Write-Output "  DMEMMAPPER+0x14 = 0x$([Convert]::ToString([BitConverter]::ToUInt32($vbios, $mapOff+20), 16))"
