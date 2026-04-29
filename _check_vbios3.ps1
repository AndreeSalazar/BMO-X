# Proper NVIDIA VBIOS (NVGI format) analysis
# Reference: nouveau/nvkm/subdev/bios/ + nvbios project
$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

Write-Output "=== NVGI Header Analysis ==="
# NVGI format: first 2 bytes should be 55 AA for real ROM, or NVGI for raw dump
# Let's check if there's a real PCI ROM image inside

# NVIDIA VBIOS can have multiple images. On modern GPUs the VBIOS is stored in SPI flash
# and dumped with nvidia-smi or nvflash. The format depends on how it was dumped.

# Check if this is actually a full SPI flash dump (includes flash header)
Write-Output "First 128 bytes:"
$hex = [System.BitConverter]::ToString($vbios, 0, 128)
Write-Output $hex
Write-Output ""

# Look for 55 AA (PCI ROM header) anywhere in first 64KB
Write-Output "=== Looking for PCI ROM headers (55 AA) ==="
for ($i = 0; $i -lt [Math]::Min(0x100000, $vbios.Length - 2); $i++) {
    if ($vbios[$i] -eq 0x55 -and $vbios[$i+1] -eq 0xAA) {
        $imgSz = $vbios[$i+2] * 512
        Write-Output "  55 AA at offset 0x$([Convert]::ToString($i, 16)), image_size=$imgSz bytes"
        # Look for PCIR at the expected offset
        if ($i + 0x18 + 4 -lt $vbios.Length) {
            $pcirOff = [BitConverter]::ToUInt16($vbios, $i + 0x18)
            if ($pcirOff -gt 0 -and ($i + $pcirOff + 20) -lt $vbios.Length) {
                $pcirSig = [System.Text.Encoding]::ASCII.GetString($vbios, $i + $pcirOff, 4)
                if ($pcirSig -eq "PCIR") {
                    $vendor = [BitConverter]::ToUInt16($vbios, $i + $pcirOff + 4)
                    $device = [BitConverter]::ToUInt16($vbios, $i + $pcirOff + 6)
                    $codeType = $vbios[$i + $pcirOff + 20]
                    $indicator = $vbios[$i + $pcirOff + 21]
                    Write-Output "    PCIR: vendor=0x$([Convert]::ToString($vendor, 16)) device=0x$([Convert]::ToString($device, 16)) codeType=$codeType last=$(($indicator -band 0x80) -ne 0)"
                }
            }
        }
    }
}

# Check if offset 0x200 or 0x1000 has a ROM image (common in SPI dumps)
Write-Output ""
Write-Output "=== Checking known offsets for embedded PCI ROM ==="
foreach ($checkOff in @(0x200, 0x1000, 0x10000, 0xC0000, 0xE0000)) {
    if ($checkOff + 2 -lt $vbios.Length) {
        $b0 = $vbios[$checkOff]
        $b1 = $vbios[$checkOff+1]
        Write-Output "  0x$([Convert]::ToString($checkOff, 16)): 0x$([Convert]::ToString($b0, 16)) 0x$([Convert]::ToString($b1, 16))"
    }
}

# The real BIT table parsing - BIT has a specific structure:
# Signature: 0xB8FF ('BIT\0' is used in some docs, but the real sig is 0xFF 0xB8)
# Or it can be "BIT" followed by null
Write-Output ""
Write-Output "=== Searching for BIT signatures more carefully ==="
for ($i = 0; $i -lt [Math]::Min(0x100000, $vbios.Length - 16); $i++) {
    # BIT\0 pattern
    if ($vbios[$i] -eq 0x42 -and $vbios[$i+1] -eq 0x49 -and $vbios[$i+2] -eq 0x54 -and $vbios[$i+3] -eq 0x00) {
        Write-Output "  BIT at 0x$([Convert]::ToString($i, 16))"
        # BIT header: "BIT\0" + version(1) + header_size(1) + entry_count(1) + entry_size(1)
        # But newer format: "BIT\0" + version(1) + header_len(1)
        $bitVer = $vbios[$i+4]
        $bitHdrLen = $vbios[$i+5]
        $bitTokenEntrySize = $vbios[$i+6]
        $bitTokenCount = $vbios[$i+7]
        Write-Output "    ver=$bitVer hdr_len=$bitHdrLen token_entry_size=$bitTokenEntrySize token_count=$bitTokenCount"
        
        # Each BIT token entry: id(1) + data_version(1) + data_size(2) + data_ptr(2)
        $tokOff = $i + $bitHdrLen
        for ($t = 0; $t -lt $bitTokenCount; $t++) {
            if ($tokOff + $bitTokenEntrySize -gt $vbios.Length) { break }
            $tokId = $vbios[$tokOff]
            $tokDv = $vbios[$tokOff+1]
            $tokSz = [BitConverter]::ToUInt16($vbios, $tokOff+2)
            $tokPtr = [BitConverter]::ToUInt16($vbios, $tokOff+4)
            $tokChar = if ($tokId -ge 0x20 -and $tokId -le 0x7E) { [char]$tokId } else { "0x$([Convert]::ToString($tokId, 16))" }
            Write-Output "    Token '$tokChar' (0x$([Convert]::ToString($tokId, 16))): dv=$tokDv sz=$tokSz ptr=0x$([Convert]::ToString($tokPtr, 16))"
            
            # 'F' = Falcon data
            if ($tokId -eq [byte][char]'F' -and $tokPtr -ne 0 -and $tokPtr -lt $vbios.Length) {
                Write-Output "      *** FALCON DATA ***"
                $fdata = [System.BitConverter]::ToString($vbios, $tokPtr, [Math]::Min(48, $vbios.Length - $tokPtr))
                Write-Output "      Raw: $fdata"
                # BIT_FALCON_DATA structure: 
                #   falcon_ucode_table_ptr(4) for v1/v2
                if ($tokSz -ge 4) {
                    $ucTablePtr = [BitConverter]::ToUInt32($vbios, $tokPtr)
                    Write-Output "      ucode_table_ptr = 0x$([Convert]::ToString($ucTablePtr, 16))"
                    if ($ucTablePtr -gt 0 -and $ucTablePtr + 16 -lt $vbios.Length) {
                        $ucData = [System.BitConverter]::ToString($vbios, [int]$ucTablePtr, [Math]::Min(64, $vbios.Length - [int]$ucTablePtr))
                        Write-Output "      ucode_table: $ucData"
                    }
                }
            }
            
            $tokOff += $bitTokenEntrySize
        }
        break
    }
}
