# _find_fwsec3.ps1 - Deep analysis of PMU pointer chain
# CONFIRMED: BIT entries at 0x93BC, stride=6, count=18
# Token 'p': ver=2, len=4, ptr=0x0401
# 
# The BIT entry ptr values (0x0238, 0x0244, 0x0269, etc.) are clearly
# relative to the ROM image start (0x9200), since they're small offsets
# within the 64KB ROM image.
#
# In nouveau, nvbios_prom reads via NV_PROM (BAR0+0x300000).
# For actual GPU PROM access, the mapping is:
#   bios->data[addr] = nv_rd08(priv, 0x300000 + addr)
# The SPI flash is memory-mapped starting at PROM offset 0.
#
# BUT: nouveau's shadow_image() also adjusts for PCI ROM images.
# In nvbios_shadow_pci(), it reads via pci_map_rom() which gives the
# PCI expansion ROM - this starts at the 55 AA header, NOT SPI offset 0.
#
# Key question: which shadow method does the actual nouveau driver use?
# For discrete GPUs: usually nvbios_prom (reads full SPI flash)
# 
# Looking at nouveau more carefully:
# nvbios_prom reads NV_PROM which maps the full SPI flash
# BUT the BIT search happens via nvbios_findstr which searches the
# ENTIRE bios->data. On our dump, BIT is at offset 0x93B2.
# The BIT entry ptr=0x0401 is used as bios->data[0x0401].
# 
# If bios->data starts at SPI offset 0, then bios->data[0x0401] = vbios[0x0401]
# If bios->data starts at ROM offset, then bios->data[0x0401] = vbios[0x9200+0x0401]
#
# The BIT itself is at bios offset 0x93B2 (SPI-absolute) or 0x01B2 (ROM-relative).
# If BIT entry ptr=0x0401 is ROM-relative, it would be within the ROM (valid).
# If BIT entry ptr=0x0401 is SPI-absolute, that's offset 0x0401 in SPI flash
# (within the NVGI header area, before the ROM) - unlikely.
#
# CONCLUSION: ptr values are ROM-relative. And the data at romBase+ptr is
# what nouveau would see. So the PMU pointer 0x73E1B is ALSO ROM-relative.
# But 0x73E1B is 474KB, way beyond the 64KB legacy ROM.
#
# This means the pointer likely refers to the FULL SPI flash image,
# not just the legacy ROM. In the full SPI image mapped via PROM,
# offset 0x73E1B would be... let me check what's there.

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$romBase = 0x9200

# In nouveau's nvbios_prom, bios->data[0] = SPI[0]
# The BIT is found at bios->data[0x93B0] = SPI[0x93B0]
# BIT entry 'p' has ptr=0x0401
# But wait - if bios->data[0] = SPI[0], then the ptr value
# is used as bios->data[0x0401] = SPI[0x0401]
# 
# Let me reconsider: maybe the ptr field in BIT entries IS SPI-absolute
# after all, and the correlation with ROM base was coincidental.
#
# Let's check: entry '2' has ptr=0x0238
# At SPI 0x0238: what's there?
# At ROM+0x0238 = 0x9438: what's there?

Write-Output "=== Checking BIT entry ptr interpretation ==="
Write-Output ""

# Entry '2': ptr=0x0238
Write-Output "Entry '2' ptr=0x0238:"
Write-Output "  SPI[0x0238]: $([BitConverter]::ToString($vbios, 0x0238, 16))"
Write-Output "  ROM[0x0238] = SPI[0x9438]: $([BitConverter]::ToString($vbios, 0x9438, 16))"
Write-Output ""

# Entry 'B': ptr=0x0244
Write-Output "Entry 'B' ptr=0x0244:"
Write-Output "  SPI[0x0244]: $([BitConverter]::ToString($vbios, 0x0244, 16))"
Write-Output "  ROM[0x0244] = SPI[0x9444]: $([BitConverter]::ToString($vbios, 0x9444, 16))"
Write-Output ""

# Entry 'V': ptr=0x03F1, len=6
Write-Output "Entry 'V' ptr=0x03F1, len=6:"
Write-Output "  SPI[0x03F1]: $([BitConverter]::ToString($vbios, 0x03F1, 16))"
Write-Output "  ROM[0x03F1] = SPI[0x95F1]: $([BitConverter]::ToString($vbios, 0x95F1, 16))"

# BIT 'V' (Voltage) should contain recognizable voltage table data
# BIT '2' should contain version/date info
# The data should look "structured" at the correct offset

Write-Output ""
Write-Output "=== Checking if BIT ptr is relative to BIT or absolute ==="
# In nouveau bit.c, the BIT signature is found, then entries are parsed.
# The entry's offset field is used directly with nvbios_rd* functions.
# nvbios_rd* reads from bios->data[offset].
#
# For PROM shadow: bios->data IS the full SPI content.
# So bios->data[0x0401] = SPI[0x0401] = our vbios[0x0401]
#
# BUT WAIT: nouveau shadow_image() in shadow.c first tries to find
# the ROM header. If it's an SPI dump, it looks for 55 AA at various
# offsets. When it finds 55 AA at offset 0x9200, it might set
# bios->image_offset = 0x9200 and then all reads become:
# bios->data[offset] = SPI[bios->image_offset + offset]
#
# Let me check shadow.c and nvbios_rd08

# Actually, looking at nouveau code more carefully:
# nvbios_rd08(bios, addr) reads bios->data[addr]
# bios->data is set by the shadow function and always starts at SPI offset 0
# (for PROM), or at the mapped ROM start (for PCI ROM).
#
# The BIT table IS found via scanning, so bit_offset is a bios->data offset.
# BIT is at bios->data[0x93B0]. The entry ptr fields are ALSO bios->data offsets.
# So ptr=0x0401 means bios->data[0x0401] = SPI[0x0401].
#
# But that contradicts the user's earlier finding that data at SPI[0x9601]
# (romBase + ptr) looked correct (1B-3E-07-00), while SPI[0x0401]
# contained garbage (15-2F-08-70).
#
# Unless... the ptr field is NOT directly a bios offset, but is ROM-image-relative.
# Let me check the BIT specification more carefully.
#
# OK: the BIT header structure IS inside the PCI ROM image. The BIT at
# SPI 0x93B0 is at ROM-relative 0x01B0 (0x93B0 - 0x9200).
# The entry ptrs are small (0x0238, 0x0244, etc.) and make sense as
# ROM-relative offsets.
# 
# In nouveau, when shadowing via PROM, the entire SPI flash is read into
# bios->data. Then nvbios_findstr finds "BIT" at absolute offset 0x93B0.
# Then BIT entry offsets should be absolute within bios->data, i.e.,
# they should be SPI-absolute. But they're small values like 0x0238.
#
# THE ANSWER: The BIT ptr fields INCLUDE the ROM image offset already!
# No wait - 0x0238 is too small for that (ROM starts at 0x9200).
#
# OK, I think the issue is that nouveau doesn't use PROM shadow for this
# specific VBIOS. Or rather, the ptr values are indeed ROM-image-relative
# and nouveau's code adds the image base.
#
# Let me look at this differently. Let me check if there's a devinit
# table or known structure at either offset to confirm which is correct.

Write-Output ""
Write-Output "=== BIT 'T' token = PMC Table ==="
# Entry 'T': ptr=0x03E6, ver=1, len=2
Write-Output "  ROM-relative: SPI[0x95E6]: $([BitConverter]::ToString($vbios, 0x95E6, 8))"
Write-Output "  SPI-absolute: SPI[0x03E6]: $([BitConverter]::ToString($vbios, 0x03E6, 8))"

Write-Output ""
Write-Output "=== BIT 'I' token = Init Tables ==="
# Entry 'I': ptr=0x0299, ver=1, len=36
Write-Output "  ROM-relative: SPI[0x9499] = $([BitConverter]::ToString($vbios, 0x9499, 32))"
Write-Output "  SPI-absolute: SPI[0x0299] = $([BitConverter]::ToString($vbios, 0x0299, 32))"

# BIT 'I' data should contain a list of u16 pointers to init script tables
# These pointers should themselves be valid ROM offsets (small-ish values < 0x10000)
# Check which interpretation gives valid-looking u16 pointers:

Write-Output ""
Write-Output "BIT 'I' as ROM-relative - first 8 u16 words:"
for ($w = 0; $w -lt 8; $w++) {
    $val = [BitConverter]::ToUInt16($vbios, 0x9499 + $w * 2)
    Write-Output "  [$w] = 0x$([Convert]::ToString($val, 16))"
}

Write-Output ""
Write-Output "BIT 'I' as SPI-absolute - first 8 u16 words:"
for ($w = 0; $w -lt 8; $w++) {
    $val = [BitConverter]::ToUInt16($vbios, 0x0299 + $w * 2)
    Write-Output "  [$w] = 0x$([Convert]::ToString($val, 16))"
}

# If the ROM-relative interpretation gives reasonable init table pointers
# (values like 0x0500-0x3000), then ptr is ROM-relative.
# The PMU pointer 0x73E1B would then be "ROM-relative" but extends beyond
# the single ROM image, possibly into the SPI flash expansion area.

Write-Output ""
Write-Output "=== Testing PMU pointer in expanded SPI context ==="
# If ptrs are ROM-relative, and the PMU pointer 0x73E1B is also ROM-relative,
# then the actual SPI offset = romBase + 0x73E1B = 0x9200 + 0x73E1B = 0x7D01B
# But we already know that contains Falcon opcodes.
#
# Alternative: for NVGI SPI dumps, there might be a secondary mapping.
# The NVGI header might indicate where actual data regions are.
# Let me look at the NVGI header structure.

Write-Output "=== NVGI Header ==="
$nvgi = [BitConverter]::ToString($vbios, 0, 64)
Write-Output "SPI[0x0000]: $nvgi"

# Check for a FALCON_DATA_V2 header near the PMU pointer
# Maybe the PMU table uses a different format on GA10x
# In newer VBIOS, the Falcon ucode table might embed the data differently

# Let me also scan for the 0x85 byte near known Falcon-related areas
Write-Output ""
Write-Output "=== Scanning for 0x85 pattern near FWSEC markers ==="
# FWSEC is app_id 0x85. Look for entries that have 0x85 as first byte
# followed by a u32 pointer at +2 that resolves to a valid location

# The PMU table entries on GA10x might be in a different format
# Let me try parsing the data at 0x7D01B as if it might be
# a PMU table with a larger header or different structure

$off = 0x7D01B
Write-Output "Data at 0x7D01B (ROM+0x73E1B):"
$raw = [BitConverter]::ToString($vbios, $off, 64)
Write-Output "  $raw"

# Try skipping some bytes to find a table header
for ($skip = 0; $skip -lt 32; $skip++) {
    $v = $vbios[$off + $skip]
    $h = $vbios[$off + $skip + 1]
    $s = $vbios[$off + $skip + 2]
    $c = $vbios[$off + $skip + 3]
    if ($v -ge 1 -and $v -le 3 -and $h -ge 4 -and $h -le 16 -and $s -ge 4 -and $s -le 32 -and $c -ge 1 -and $c -le 32) {
        Write-Output "  Possible table at +$skip = 0x$([Convert]::ToString($off+$skip, 16)): ver=$v hdr=$h stride=$s count=$c"
    }
}

# Maybe the 0x73E1B value is not a pointer at all?
# Let me re-examine the BIT 'p' payload more carefully
# ptr=0x0401, and data at ROM+0x0401 = SPI[0x9601] = 1B-3E-07-00-00...
# Wait - nvbios_pmuTe reads: data = nvbios_rd32(bios, bit_p.offset + 0x00)
# bit_p.offset = 0x0401
# 
# For PROM shadow, nvbios_rd32(bios, 0x0401) reads from SPI[0x0401]
# NOT from ROM+0x0401
#
# But we showed SPI[0x0401] = 15-2F-08-70 which looks like garbage
# And ROM+0x0401 = SPI[0x9601] = 1B-3E-07-00 which looks like a pointer
#
# RESOLUTION: The BIT entries' offset field might be adjusted by nouveau.
# Actually let me re-read bit.c one more time...
#
# bit_entry.offset = nvbios_rd16(bios, entry + 4)
# This is the RAW u16 from the entry. No adjustment.
# Then nvbios_pmuTe does: nvbios_rd32(bios, bit_p.offset)
# 
# If bios->data for PROM starts at SPI[0], then this reads SPI[0x0401]
# If bios->data for PCI ROM starts at SPI[0x9200], then this reads SPI[0x9601]
#
# The answer depends on how the VBIOS is shadowed!
# For PCI ROM BAR: bios->data = mapped PCI ROM = starts at 55 AA header
# For PROM: bios->data = full SPI flash from offset 0
#
# On a real GPU, if nouveau uses PROM shadow (most common for discrete),
# bios->data starts at SPI[0]. But the BIT table has ptr values that
# only make sense if relative to the ROM image.
#
# UNLESS... the GPU's PROM doesn't start at the NVGI header.
# On some GPUs, PROM access starts at the PCI ROM image, not at SPI offset 0.
# The NVGI container format is only visible via SPI bus (SPISOM interface),
# while PROM access maps to the decoded ROM image directly.
#
# THIS IS THE KEY: NV_PROM reads the DECODED ROM, not raw SPI flash!
# So bios->data[0] = first byte of the decoded PCI ROM (55 AA header),
# NOT the NVGI header.

Write-Output ""
Write-Output "=== FINAL: PROM maps decoded ROM, not raw SPI ==="
Write-Output "bios->data[0] = PCI ROM start = SPI[0x9200]"
Write-Output "BIT at bios->data[0x01B0] = SPI[0x93B0]"
Write-Output "BIT 'p' offset=0x0401: bios->data[0x0401] = SPI[0x9601] = 1B-3E-07-00"
Write-Output "PMU table pointer = 0x73E1B: bios->data[0x73E1B] = SPI[0x7D01B]"
Write-Output ""
Write-Output "But the PCI ROM is only 65024 bytes! 0x73E1B is way beyond it."
Write-Output "PROM access must map MORE than just the legacy ROM image."
Write-Output "The full PROM window likely includes ALL firmware data in the SPI flash."
Write-Output ""
Write-Output "For NVGI format, the 'decoded' PROM view likely presents a flat"
Write-Output "address space that includes both ROM images and firmware blobs."
Write-Output "So bios->data[0x73E1B] really does point into the firmware area."
Write-Output ""

# Let me check: what is the total PROM size?
# NVGI SPI flash = 2MB. If PROM maps from ROM start (0x9200),
# then max accessible = 2MB - 0x9200 = ~2027KB
# 0x73E1B = 475KB from ROM start, well within 2MB SPI flash
#
# So SPI[0x9200 + 0x73E1B] = SPI[0x7D01B] contains Falcon code opcodes.
# This IS the FWSEC ucode! It's just that it's not a table header -
# it's the raw Falcon instructions!
#
# But nouveau expects a PMU LOOKUP TABLE here, not raw code.
# The table should have: ver(1), hdr(1), stride(1), count(1), then entries
# Each entry: type(1), pad(1), ptr(4)
#
# Unless 0x73E1B points to a NEWER table format, or we need to interpret
# the BIT 'p' data differently for GA10x

# Actually wait - let me re-read the BIT 'p' payload more carefully
# The payload starts at 0x9601. It has length=4, so the ENTIRE payload
# is just the u32 at 0x9601.
# But the PMU table is supposed to be pointed to BY that u32.
# 
# What if the data at 0x7D01B actually IS a valid PMU table in a
# different encoding? Falcon opcodes vs table data:
# 32-0A-7E-BD-04-00-89-72...
#
# 0x32 = 50 -> not a valid table version (1-3)
# This is definitely NOT a PMU table.
# 
# Let me try something else: maybe for GA10x NVGI images, the BIOS
# has TWO copies of the ROM, and the second copy at 0xE9200 has
# different PMU data (because the first copy might be a recovery copy)

$romBase2 = 0xE9200
$pmuAbs2 = $romBase2 + 0x73E1B
Write-Output "=== Trying second ROM copy ==="
Write-Output "Second ROM at 0xE9200"
Write-Output "PMU table at second_ROM + 0x73E1B = 0x$([Convert]::ToString($pmuAbs2, 16))"
if ($pmuAbs2 -lt $vbios.Length - 32) {
    $d = [BitConverter]::ToString($vbios, $pmuAbs2, 32)
    Write-Output "  Data: $d"
    Write-Output "  As table: ver=$($vbios[$pmuAbs2]) hdr=$($vbios[$pmuAbs2+1]) stride=$($vbios[$pmuAbs2+2]) count=$($vbios[$pmuAbs2+3])"
} else {
    Write-Output "  Beyond VBIOS! Offset 0x$([Convert]::ToString($pmuAbs2, 16)) > $($vbios.Length)"
}

# The second ROM + 0x73E1B = 0xE9200 + 0x73E1B = 0x15D01B
# which is about 1.4MB into the 2MB flash - should exist

# Let me also check: what if the VBIOS uses a FLAT address space
# where offset 0 is the NVGI header, and the PMU pointer 0x73E1B
# is an absolute SPI flash offset?
Write-Output ""
Write-Output "=== PMU pointer as absolute SPI offset ==="
$pmuSpi = 0x73E1B
Write-Output "SPI[0x73E1B]:"
$d = [BitConverter]::ToString($vbios, $pmuSpi, 32)
Write-Output "  $d"
Write-Output "  As table: ver=$($vbios[$pmuSpi]) hdr=$($vbios[$pmuSpi+1]) stride=$($vbios[$pmuSpi+2]) count=$($vbios[$pmuSpi+3])"

# 69-00-7E-B9... ver=105, not valid either

# SOMETHING IS WRONG with our understanding. Let me try yet another approach:
# Maybe the ptr field in BIT entries needs to be added to the BIT table base?
# Or there's a "relative base" stored somewhere?

# Actually, let me look at this from the opposite direction.
# SCAN the entire VBIOS for a valid PMU lookup table
Write-Output ""
Write-Output "=== Exhaustive scan for PMU table headers ==="
$foundTables = 0
for ($i = 0; $i -lt $vbios.Length - 32; $i++) {
    $v = $vbios[$i]
    $h = $vbios[$i+1]
    $s = $vbios[$i+2]
    $c = $vbios[$i+3]
    
    # Valid PMU table: ver=1-2, hdr=4-8, stride=6-12, count=1-20
    if ($v -ge 1 -and $v -le 2 -and $h -ge 4 -and $h -le 8 -and $s -ge 6 -and $s -le 12 -and $c -ge 1 -and $c -le 20) {
        # Verify first entry has a valid type byte
        $eOff = $i + $h
        if ($eOff + $s -le $vbios.Length) {
            $type = $vbios[$eOff]
            # Known Falcon ucode types: 0x01=PMU, 0x85=FWSEC_PROD, 0x86=FWSEC_DBG, etc.
            # Type byte should be nonzero and less than 0x90
            if ($type -ne 0 -and $type -lt 0x90) {
                # Check if any entry has type 0x85
                $has85 = $false
                for ($j = 0; $j -lt $c; $j++) {
                    $je = $i + $h + $j * $s
                    if ($je + $s -le $vbios.Length -and $vbios[$je] -eq 0x85) {
                        $has85 = $true
                    }
                }
                if ($has85) {
                    Write-Output ""
                    Write-Output "*** PMU TABLE WITH FWSEC at SPI 0x$([Convert]::ToString($i, 16)) ***"
                    Write-Output "  ver=$v hdr=$h stride=$s count=$c"
                    $d = [BitConverter]::ToString($vbios, $i, [Math]::Min($h + $c * $s, $vbios.Length - $i))
                    Write-Output "  $d"
                    for ($j = 0; $j -lt $c; $j++) {
                        $je = $i + $h + $j * $s
                        $t = $vbios[$je]
                        $dp = [BitConverter]::ToUInt32($vbios, $je + 2)
                        Write-Output "  [$j]: type=0x$([Convert]::ToString($t, 16)) data_ptr=0x$([Convert]::ToString($dp, 16))"
                    }
                    $foundTables++
                    if ($foundTables -ge 5) { break }
                }
            }
        }
    }
}

if ($foundTables -eq 0) {
    Write-Output "  No PMU tables with FWSEC entry found"
    
    # Broader scan: any PMU-like table at all
    Write-Output ""
    Write-Output "=== Broader scan: any valid PMU table ==="
    $foundAny = 0
    for ($i = 0x9000; $i -lt [Math]::Min(0x1A000, $vbios.Length - 32); $i++) {
        $v = $vbios[$i]
        $h = $vbios[$i+1]
        $s = $vbios[$i+2]
        $c = $vbios[$i+3]
        
        if ($v -ge 1 -and $v -le 2 -and $h -ge 4 -and $h -le 8 -and $s -ge 6 -and $s -le 12 -and $c -ge 2 -and $c -le 20) {
            $eOff = $i + $h
            $type = $vbios[$eOff]
            if ($type -ne 0 -and $type -le 0x90) {
                Write-Output "  Candidate at SPI 0x$([Convert]::ToString($i, 16)): ver=$v hdr=$h stride=$s count=$c"
                $d = [BitConverter]::ToString($vbios, $i, [Math]::Min($h + $c * $s + 4, 64))
                Write-Output "    $d"
                for ($j = 0; $j -lt $c; $j++) {
                    $je = $i + $h + $j * $s
                    $t = $vbios[$je]
                    Write-Output "    [$j] type=0x$([Convert]::ToString($t, 16))"
                }
                $foundAny++
                if ($foundAny -ge 5) { break }
            }
        }
    }
}
