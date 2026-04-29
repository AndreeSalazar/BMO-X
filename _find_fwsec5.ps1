# _find_fwsec5.ps1 - Find FWSEC using NVGI header and proper base address
# The NVGI header starts the SPI dump. Format:
# +0: "NVGI" magic
# +4: ... various fields
# The NVGI format stores multiple firmware images at different offsets.
# We need to understand the NVGI structure to find the correct base.

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$romBase = 0x9200

Write-Output "=== NVGI Header Analysis ==="
$raw = [BitConverter]::ToString($vbios, 0, 128)
Write-Output "First 128 bytes: $raw"

Write-Output ""
Write-Output "NVGI magic: $([System.Text.Encoding]::ASCII.GetString($vbios, 0, 4))"

# NVGI header fields (from NVIDIA SPI flash documentation):
# +0: 4 bytes "NVGI" magic
# +4: u32 - total image size or flags
# +8: u32 - some version/ID
# +12: u32 - checksum or hash
# Let me just dump key u32 values
for ($i = 0; $i -lt 64; $i += 4) {
    $val = [BitConverter]::ToUInt32($vbios, $i)
    Write-Output "  +$i = 0x$([Convert]::ToString($val, 16))"
}

# Let me reconsider the PMU table finding.
# I found the table at SPI 0x9181B by brute force.
# But maybe this table address can be reached through the BIT chain
# if we interpret offsets correctly.
#
# The PMU table ROM offset = 0x9181B - 0x9200 = 0x861B
# But actually... wait. 0x9181B - 0x9200 = 0x8861B? No.
# 0x9181B - 0x9200 = 0x8F61B? Let me compute properly.
# 0x9181B = 596,507
# 0x9200 = 37,376
# 0x9181B - 0x9200 = 559,131 = 0x8861B
# That's ~546KB, way beyond the 64KB legacy ROM.
#
# Actually the PMU table is in the EXPANDED SPI flash area,
# not within the legacy PCI ROM image itself.
# The PROM mapping exposes the FULL SPI flash starting at offset 0.
# So bios->data[0x9181B] would be at SPI 0x9181B... but that means
# bios->data starts at SPI[0] and the BIT is at bios[0x93B0].
# BIT entry ptrs would be at SPI offsets... but 0x0401 < 0x9200
# which means it's before the ROM image, in the NVGI header area.
# That gave us garbage data. So something is wrong with this model.
#
# ALTERNATIVE: nouveau's PROM shadow on Ampere might NOT read from
# SPI offset 0. It might read from the ROM image start.
# Different GPU architectures have different PROM layouts.

# Let me look at what nouveau does for GA10x specifically.
# Actually, for modern GPUs (Turing+), nouveau often uses ACPI _ROM
# or PCI ROM BAR, which gives just the PCI expansion ROM.
# The PROM access (NV_PROM) might not even be available on Ampere.

# So if nouveau uses PCI ROM BAR:
# bios->data[0] = first byte of PCI ROM = SPI[0x9200] = 0x55
# bios->data[0x01B0] = SPI[0x93B0] = BIT prefix (correct!)
# BIT entry 'p' ptr = 0x0401 -> bios->data[0x0401] = SPI[0x9200 + 0x0401] = SPI[0x9601]
# Data = 1B-3E-07-00 = ptr 0x73E1B
# PMU table at bios->data[0x73E1B] = SPI[0x9200 + 0x73E1B] = SPI[0x7D01B]
# But this is Falcon code, not a table header.
#
# UNLESS the PCI ROM BAR exposes MORE than just the 64KB legacy image.
# On modern GPUs, the PCI expansion ROM can be much larger.
# The EFI ROM at 0x19000 (SPI) = ROM offset 0xF800.
# That's beyond the 64KB legacy image.
# ROM offset 0x73E1B = 474KB from ROM start - could this be within
# the full expansion ROM?

# Check: what is the total PCI ROM size according to PCIR?
$pcirOff = 0x9218  # PCIR in first ROM image (SPI absolute)
Write-Output ""
Write-Output "=== PCI ROM structure ==="
$pcirMagic = [System.Text.Encoding]::ASCII.GetString($vbios, $pcirOff, 4)
Write-Output "PCIR at SPI 0x9218: $pcirMagic"
$romLen512 = [BitConverter]::ToUInt16($vbios, $pcirOff + 16)
$romLen = $romLen512 * 512
$codeType = $vbios[$pcirOff + 20]
$lastImage = $vbios[$pcirOff + 21]
Write-Output "  image_length = $romLen512 x 512 = $romLen bytes"
Write-Output "  code_type = $codeType"
Write-Output "  last_image = 0x$([Convert]::ToString($lastImage, 16))"

# The legacy ROM says 65024 bytes. But are there more images?
# Walk all PCI ROM images starting from romBase
Write-Output ""
Write-Output "=== Walking PCI ROM image chain ==="
$imgOff = $romBase
$imgIdx = 0
while ($imgOff -lt $vbios.Length - 32) {
    if ($vbios[$imgOff] -ne 0x55 -or $vbios[$imgOff+1] -ne 0xAA) {
        Write-Output "  No more ROM images at SPI 0x$([Convert]::ToString($imgOff, 16))"
        break
    }
    
    # Find PCIR
    $pcrOff = $imgOff + [BitConverter]::ToUInt16($vbios, $imgOff + 24)
    if ($pcrOff + 24 -gt $vbios.Length) { break }
    
    $vendor = [BitConverter]::ToUInt16($vbios, $pcrOff + 4)
    $device = [BitConverter]::ToUInt16($vbios, $pcrOff + 6)
    $imgLen512 = [BitConverter]::ToUInt16($vbios, $pcrOff + 16)
    $imgLen = $imgLen512 * 512
    $cType = $vbios[$pcrOff + 20]
    $last = $vbios[$pcrOff + 21]
    
    $relOff = $imgOff - $romBase
    Write-Output "  Image[$imgIdx] at SPI 0x$([Convert]::ToString($imgOff, 16)) ROM+0x$([Convert]::ToString($relOff, 16)): vendor=0x$([Convert]::ToString($vendor, 16)) device=0x$([Convert]::ToString($device, 16)) type=$cType size=$imgLen last=0x$([Convert]::ToString($last, 16))"
    
    if ($imgLen -eq 0) { break }
    
    $imgIdx++
    $imgOff += $imgLen
    
    if (($last -band 0x80) -ne 0) {
        Write-Output "  Last image flag set. Total PCI ROM = 0x$([Convert]::ToString($imgOff - $romBase, 16)) bytes"
        break
    }
}

# Now let me check: is the total PCI ROM size >= 0x73E1B?
$totalRomSize = $imgOff - $romBase
Write-Output ""
Write-Output "Total PCI ROM chain size: 0x$([Convert]::ToString($totalRomSize, 16)) = $totalRomSize bytes"
Write-Output "PMU pointer: 0x73E1B = $([Convert]::ToInt32('73E1B', 16)) bytes"

if ($totalRomSize -ge 0x73E1B) {
    Write-Output "PMU pointer IS within ROM chain - checking data..."
    $pmuTableAbs = $romBase + 0x73E1B
    $d = [BitConverter]::ToString($vbios, $pmuTableAbs, 32)
    Write-Output "  $d"
} else {
    Write-Output "PMU pointer EXCEEDS ROM chain size!"
    Write-Output "The FWSEC data extends beyond the PCI ROM images."
    Write-Output ""
    Write-Output "Checking what's after the last PCI ROM image..."
    $afterRom = $imgOff
    if ($afterRom + 64 -lt $vbios.Length) {
        $d = [BitConverter]::ToString($vbios, $afterRom, 64)
        Write-Output "  Data at SPI 0x$([Convert]::ToString($afterRom, 16)): $d"
    }
    
    # For NVGI SPI dumps, firmware blobs (Falcon ucodes) live AFTER
    # the PCI ROM images, still within the SPI flash
    # The PMU table pointer 0x73E1B might reference from the START of 
    # the SPI flash (NVGI offset 0), not from ROM base
    
    Write-Output ""
    Write-Output "=== Trying PMU pointer from NVGI base ==="
    $pmuFromNVGI = 0x73E1B
    if ($pmuFromNVGI + 32 -lt $vbios.Length) {
        $d = [BitConverter]::ToString($vbios, $pmuFromNVGI, 32)
        Write-Output "  SPI[0x73E1B]: $d"
        Write-Output "  As table: ver=$($vbios[$pmuFromNVGI]) hdr=$($vbios[$pmuFromNVGI+1]) stride=$($vbios[$pmuFromNVGI+2]) count=$($vbios[$pmuFromNVGI+3])"
    }
}

# Let me also check if the PMU table we brute-forced at SPI 0x9181B
# matches the BIT 'p' chain in any way
Write-Output ""
Write-Output "=== PMU table relationship ==="
Write-Output "Brute-force PMU at SPI 0x9181B"
Write-Output "BIT 'p' -> 0x73E1B"
Write-Output "Difference: 0x$([Convert]::ToString(0x9181B - 0x73E1B, 16))"
$diff = 0x9181B - 0x73E1B
Write-Output "= $diff = 0x$([Convert]::ToString($diff, 16))"
# 0x9181B - 0x73E1B = 0x1DA00
Write-Output "0x1DA00 = $([Convert]::ToInt32('1DA00', 16))"
# Not an obvious offset

# What about: PMU table SPI 0x9181B minus romBase 0x9200 = 0x861B
# And 0x861B shifted or adjusted...
# Actually: the PMU table at SPI 0x9181B = from NVGI base + 0x9181B
# But from PCI ROM base + 0x861B would give SPI 0x11A1B (since romBase=0x9200)
# That's wrong. Let me compute: romBase + 0x861B = 0x9200 + 0x861B = 0x1181B
# NOT 0x9181B. Hmm.
# 0x9181B = 0x9200 * something? No.

# OK let me just check: is the PMU table at SPI 0x9181B within the 
# data area that the BIT 'p' payload SHOULD reach?
# The 'p' payload at SPI[0x9601] contains:
# 1B-3E-07-00  00-00-00-00  00-00-00-00  00-00-00-00
# That's just one u32 pointer (0x73E1B) followed by zeros.
# Maybe the payload is LARGER than 4 bytes and we need to look at 
# more fields?

Write-Output ""
Write-Output "=== BIT 'p' payload extended dump ==="
$pPayload = 0x9601
$d = [BitConverter]::ToString($vbios, $pPayload, 64)
Write-Output "SPI[0x9601] 64 bytes: $d"

# The PMU table brute-force at SPI 0x9181B contains data that starts
# with 01-06-06-10. Let me search for this specific pattern near
# the ROM area to confirm it's the right table.
Write-Output ""
Write-Output "=== Search for 01-06-06-10 pattern ==="
for ($i = $romBase; $i -lt $vbios.Length - 4; $i++) {
    if ($vbios[$i] -eq 0x01 -and $vbios[$i+1] -eq 0x06 -and $vbios[$i+2] -eq 0x06 -and $vbios[$i+3] -eq 0x10) {
        Write-Output "  Found at SPI 0x$([Convert]::ToString($i, 16)) = ROM+0x$([Convert]::ToString($i - $romBase, 16))"
    }
}

# And check the offset of the PMU table from the ROM base
$pmtRomOff = 0x9181B - $romBase
Write-Output ""
Write-Output "PMU table at ROM offset 0x$([Convert]::ToString($pmtRomOff, 16))"
Write-Output "Is this within the combined ROM images? TotalRomSize=0x$([Convert]::ToString($totalRomSize, 16))"
