$bytes = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\fwsec_offset.bin')
Write-Output "fwsec_offset.bin size: $($bytes.Length) bytes"
$hex = [System.BitConverter]::ToString($bytes)
Write-Output "Hex: $hex"

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
Write-Output ""
Write-Output "vbios_rtx3060.rom size: $($vbios.Length) bytes ($([Math]::Round($vbios.Length / 1024)) KB)"
$first64 = [System.BitConverter]::ToString($vbios, 0, 64)
Write-Output "First 64: $first64"

# Check PCI ROM signature
$sig = [System.BitConverter]::ToString($vbios, 0, 2)
Write-Output "PCI ROM sig: $sig (expect 55-AA)"

# Find PCIR
for ($i = 0; $i -lt [Math]::Min(512, $vbios.Length); $i++) {
    if ($vbios[$i] -eq 0x50 -and ($i+3) -lt $vbios.Length -and $vbios[$i+1] -eq 0x43 -and $vbios[$i+2] -eq 0x49 -and $vbios[$i+3] -eq 0x52) {
        Write-Output "PCIR at offset: $i (0x$([Convert]::ToString($i, 16)))"
        # Read vendor/device ID from PCIR header
        $vendor = [BitConverter]::ToUInt16($vbios, $i+4)
        $device = [BitConverter]::ToUInt16($vbios, $i+6)
        Write-Output "  PCIR Vendor: 0x$([Convert]::ToString($vendor, 16)) Device: 0x$([Convert]::ToString($device, 16))"
        break
    }
}

# Find NPDE
for ($i = 0; $i -lt [Math]::Min(4096, $vbios.Length); $i++) {
    if ($vbios[$i] -eq 0x4E -and ($i+3) -lt $vbios.Length -and $vbios[$i+1] -eq 0x50 -and $vbios[$i+2] -eq 0x44 -and $vbios[$i+3] -eq 0x45) {
        Write-Output "NPDE at offset: $i (0x$([Convert]::ToString($i, 16)))"
        break
    }
}

# Search for FWSEC signature patterns in VBIOS
# NVIDIA VBIOS contains multiple "images" - the FWSEC code is typically in a FALCON microcode image
# Look for Falcon code signature (0x7F 'E' 'L' 'F' = ELF) or nvfw_bin_hdr magic 0x10DE
$found_elf = $false
$found_10de = $false
for ($i = 0; $i -lt $vbios.Length - 4; $i++) {
    if (-not $found_elf -and $vbios[$i] -eq 0x7F -and $vbios[$i+1] -eq 0x45 -and $vbios[$i+2] -eq 0x4C -and $vbios[$i+3] -eq 0x46) {
        Write-Output "ELF header at offset: $i (0x$([Convert]::ToString($i, 16)))"
        $found_elf = $true
    }
    if (-not $found_10de -and $vbios[$i] -eq 0xDE -and $vbios[$i+1] -eq 0x10 -and $vbios[$i+2] -eq 0x00 -and $vbios[$i+3] -eq 0x00) {
        Write-Output "nvfw_bin_hdr (0x10DE) at offset: $i (0x$([Convert]::ToString($i, 16)))"
        # Print surrounding context
        $ctx = [System.BitConverter]::ToString($vbios, [Math]::Max(0, $i), [Math]::Min(32, $vbios.Length - $i))
        Write-Output "  Context: $ctx"
        $found_10de = $true
    }
}

# Look for BIT (BIOS Information Table) header "BIT\0"
for ($i = 0; $i -lt [Math]::Min(65536, $vbios.Length); $i++) {
    if ($vbios[$i] -eq 0x42 -and ($i+3) -lt $vbios.Length -and $vbios[$i+1] -eq 0x49 -and $vbios[$i+2] -eq 0x54 -and $vbios[$i+3] -eq 0x00) {
        Write-Output "BIT header at offset: $i (0x$([Convert]::ToString($i, 16)))"
        break
    }
}

# Look for FALCON ucode header patterns (SEC2/GSP microcode in VBIOS)
# Pattern: look for the magic bytes that indicate a HS (Heavy Secure) ucode section
Write-Output ""
Write-Output "=== Searching for Falcon/HS ucode signatures ==="

# In NVIDIA VBIOS, the FWSEC ucode is found via BIT_FALCON_DATA
# It typically has a header structure: nvkm_falcon_fw (from nouveau)
# The key is the PCI expansion ROM structure - each image has a type field

# Check all PCI ROM images
$offset = 0
$imgIdx = 0
while ($offset -lt $vbios.Length - 2) {
    if ($vbios[$offset] -eq 0x55 -and $vbios[$offset+1] -eq 0xAA) {
        $imgSize = $vbios[$offset + 2] * 512
        Write-Output "ROM Image $imgIdx at 0x$([Convert]::ToString($offset, 16)), size=$imgSize bytes"
        # Find PCIR in this image
        for ($j = $offset; $j -lt [Math]::Min($offset + 256, $vbios.Length - 4); $j++) {
            if ($vbios[$j] -eq 0x50 -and $vbios[$j+1] -eq 0x43 -and $vbios[$j+2] -eq 0x49 -and $vbios[$j+3] -eq 0x52) {
                $type = $vbios[$j + 20]
                $last = $vbios[$j + 21]
                Write-Output "  PCIR: type=0x$([Convert]::ToString($type, 16)) last=0x$([Convert]::ToString($last, 16))"
                break
            }
        }
        if ($imgSize -eq 0) { break }
        $offset += $imgSize
        $imgIdx++
    } else {
        break
    }
}
