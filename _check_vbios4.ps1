# The REAL PCI ROM is at offset 0x9200 inside the NVGI dump
# The 0x9200 bytes before it are the NVIDIA SPI flash header
$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

$romBase = 0x9200
Write-Output "=== PCI ROM Image 0 at 0x$([Convert]::ToString($romBase, 16)) ==="
Write-Output "Device: GA106 (RTX 3060 12G) - PCI ID 10DE:2504"
Write-Output ""

# The PCIR header tells us the structure
$pcirOff = [BitConverter]::ToUInt16($vbios, $romBase + 0x18)
Write-Output "PCIR offset (from ROM base): 0x$([Convert]::ToString($pcirOff, 16))"
$pcirAbsOff = $romBase + $pcirOff
$pcirSig = [System.Text.Encoding]::ASCII.GetString($vbios, $pcirAbsOff, 4)
Write-Output "PCIR sig: $pcirSig"
$imgLen = [BitConverter]::ToUInt16($vbios, $pcirAbsOff + 16) * 512
Write-Output "Image length: $imgLen bytes"
Write-Output ""

# Now look for BIT table WITHIN the ROM image (not the SPI header)
Write-Output "=== Looking for BIT table inside PCI ROM ==="
for ($i = $romBase; $i -lt [Math]::Min($romBase + 0x20000, $vbios.Length - 16); $i++) {
    if ($vbios[$i] -eq 0x42 -and $vbios[$i+1] -eq 0x49 -and $vbios[$i+2] -eq 0x54 -and $vbios[$i+3] -eq 0x00) {
        $relOff = $i - $romBase
        Write-Output "  BIT at absolute=0x$([Convert]::ToString($i, 16)) (ROM+0x$([Convert]::ToString($relOff, 16)))"
        
        # Dump raw bytes at BIT location
        $rawBit = [System.BitConverter]::ToString($vbios, $i, 32)
        Write-Output "  Raw: $rawBit"
        
        # BIT header from nouveau (nvbios/bit.c):
        # signature[2] = "BIT\0" (that's actually bytes 'B','I','T',0x00)
        # Then varies by version. Let's just try the header carefully:
        # offset+0: 'B'
        # offset+1: 'I'  
        # offset+2: 'T'
        # offset+3: 0x00
        # offset+4: version (uint8)
        # offset+5: header_length (uint8) - total header bytes before entries
        # offset+6: entry_size (uint8)  
        # offset+7: entry_count (uint8)
        
        # But wait - nouveau bit.c uses version check:
        # if version == 1: hdr_len = hdr[5], token_size = hdr[6], tokens = hdr[7]
        # if version == 2: same
        
        # Looking at the raw bytes more carefully...
        # Let me try offset+3 as null, +4 as version
        $bitVersion = $vbios[$i+4]
        Write-Output "  Version byte: $bitVersion"
        
        # Try different header layouts
        # Layout 1: BIT\0 + 1 byte ver + 1 byte hdr_size + rest
        for ($hdrLen = 6; $hdrLen -le 16; $hdrLen += 2) {
            $entrySz = $vbios[$i + $hdrLen - 2]  
            $entryCnt = $vbios[$i + $hdrLen - 1]
            if ($entrySz -ge 6 -and $entrySz -le 12 -and $entryCnt -ge 5 -and $entryCnt -le 40) {
                Write-Output ("  Trying hdrLen=" + $hdrLen + " entry_size=" + $entrySz + " entry_count=" + $entryCnt)
                
                $tokOff = $i + $hdrLen
                $validTokens = 0
                for ($t = 0; $t -lt $entryCnt; $t++) {
                    if ($tokOff + $entrySz -gt $vbios.Length) { break }
                    $tokId = $vbios[$tokOff]
                    $tokDv = $vbios[$tokOff+1]
                    $tokSz = [BitConverter]::ToUInt16($vbios, $tokOff+2)
                    $tokPtr = [BitConverter]::ToUInt16($vbios, $tokOff+4)
                    $tokChar = if ($tokId -ge 0x20 -and $tokId -le 0x7E) { [char]$tokId } else { "0x$([Convert]::ToString($tokId, 16))" }
                    
                    if ($tokId -ne 0 -or $tokDv -ne 0) {
                        $validTokens++
                        Write-Output "      [$t] Token '$tokChar' (0x$([Convert]::ToString($tokId, 16))): dv=$tokDv sz=$tokSz ptr=0x$([Convert]::ToString($tokPtr, 16))"
                        
                        # For Falcon data ('F' = 0x46), show the ucode table
                        if ($tokId -eq 0x46 -and $tokPtr -ne 0 -and $tokPtr -lt $vbios.Length) {
                            Write-Output "        *** FALCON UCODE TABLE ***"
                            $fraw = [System.BitConverter]::ToString($vbios, $tokPtr, [Math]::Min(64, $vbios.Length - $tokPtr))
                            Write-Output "        Raw: $fraw"
                        }
                    }
                    
                    $tokOff += $entrySz
                }
                Write-Output "  Valid tokens: $validTokens"
                Write-Output ""
            }
        }
        break
    }
}

# Also check the second PCI ROM image at 0x19000 (codeType=3 = EFI)
Write-Output ""
Write-Output "=== PCI ROM Image at 0x19000 ==="
$rom2 = 0x19000
$pcir2Off = $rom2 + [BitConverter]::ToUInt16($vbios, $rom2 + 0x18)
$ct2 = $vbios[$pcir2Off + 20]
Write-Output "  codeType = $ct2 (3=EFI, 0=x86)"

# Now specifically look for FWSEC inside the first ROM image
# In nouveau/nvkm/subdev/gsp/ga102.c, FWSEC is loaded via:
#   nvkm_falcon_fw_boot(&gsp->fwsec, &gsp->subdev, true, ...)
# which gets the firmware from nvkm_gsp_fwsec_load()
# which calls nvkm_falcon_fw_ctor_hs() which reads from VBIOS
#
# The key is: in nouveau, FWSEC code is read from VBIOS using BIT 'F' falcon table,
# specifically the entry for application_id = NVKM_FALCON_FWSEC
Write-Output ""
Write-Output "=== Searching for nvkm_falcon_ucode_table entries ==="
# The falcon ucode table in VBIOS has entries with:
# struct nvbios_falcon_func_hdr {
#   u16 app_id;
#   u16 target; // falcon target (SEC2=0x07, GSP=0x30, etc)
#   u32 hdr_off;
#   u32 data_off;
#   u32 data_sz;
# }

# Let's look at the area around the BIT table for the falcon ucode table pointer
# BIT 'F' token data -> first u32 = pointer to falcon ucode table
