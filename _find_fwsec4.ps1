# _find_fwsec4.ps1 - Parse the FWSEC ucode descriptor
# PMU table at SPI 0x9181B: ver=1, hdr=6, stride=6, count=16
# Entry[9]: type=0x85 (FWSEC_PROD), data_ptr=0x2CA10
#
# The data_ptr values in the PMU table are ROM-relative (from 0x9200 base).
# So the actual SPI offset = romBase + data_ptr.
#
# Actually wait - let me verify. The PMU table itself is at SPI 0x9181B.
# romBase = 0x9200. 0x9181B is NOT aligned to romBase (0x9181B < 0x9200? No 0x9181B > 0x9200).
# 0x9181B - 0x9200 = 0x861B. So the PMU table is at ROM offset 0x861B.
# But the BIT 'p' pointer was 0x73E1B... that doesn't match.
# 
# Let me figure out how nouveau reaches this table.
# The exhaustive scan found the table by brute force.
# The BIT chain should reach the same place.
#
# Maybe the BIT 'p' payload at ROM+0x0401 = SPI[0x9601] = 1B-3E-07-00
# doesn't point to 0x73E1B. Let me recheck.
# 0x073E1B = little-endian of 1B-3E-07-00. Yes.
# But the PMU table is at ROM offset 0x861B = SPI 0x9181B.
# 0x861B != 0x73E1B.
#
# So maybe the pointer 0x73E1B is NOT the PMU table start.
# The BIT 'p' data might have multiple fields beyond just the first u32.
# BIT 'p' len=4 means 4 bytes of payload at bit_p.offset.
# nvbios_pmuTe reads just the first u32 as the table pointer.
#
# Let me recheck: we found the PMU table at SPI 0x9181B = ROM 0x861B.
# If nouveau reads nvbios_rd32(bios, bit_p.offset=0x0401), 
# and bios->data starts at ROM base, then bios[0x0401] has 0x73E1B.
# And bios[0x73E1B] should be the PMU table.
# But bios[0x73E1B] = SPI[0x9200+0x73E1B] = SPI[0x7D01B] which we know is Falcon code.
#
# BUT the PMU table we found at SPI 0x9181B is at ROM offset 0x861B.
# If I look at the data at SPI 0x9601 more carefully:
# 1B-3E-07-00 = 0x00073E1B
# The PMU table at ROM 0x861B doesn't match 0x73E1B at all.
#
# WAIT - maybe the BIT 'p' data at SPI 0x9601 uses a DIFFERENT
# interpretation. Let me check the ACTUAL offset used by nouveau.

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$romBase = 0x9200

# First, let me verify the PMU table by checking data_ptr values
$pmuTableSPI = 0x9181B
$pmuHdr = 6
$pmuStride = 6
$pmuCount = 16

Write-Output "=== PMU Table at SPI 0x9181B ==="
Write-Output "ROM offset: 0x$([Convert]::ToString($pmuTableSPI - $romBase, 16))"

# Entry[9]: type=0x85, data_ptr=0x2CA10
# The data_ptr should point to a falcon_ucode_desc structure
# Is data_ptr ROM-relative or SPI-absolute?

$fwsecDataPtr = 0x2CA10
Write-Output ""
Write-Output "FWSEC data_ptr = 0x$([Convert]::ToString($fwsecDataPtr, 16))"

# Try 1: ROM-relative
$descSPI_rom = $romBase + $fwsecDataPtr
Write-Output "As ROM-relative: SPI 0x$([Convert]::ToString($descSPI_rom, 16))"
if ($descSPI_rom + 64 -lt $vbios.Length) {
    $raw = [BitConverter]::ToString($vbios, $descSPI_rom, 64)
    Write-Output "  $raw"
    $hdr = [BitConverter]::ToUInt32($vbios, $descSPI_rom)
    $sz = ($hdr -shr 16) -band 0xFFFF
    $ver = ($hdr -shr 8) -band 0xFF
    $valid = $hdr -band 1
    Write-Output "  Hdr: ver=$ver size=0x$([Convert]::ToString($sz, 16)) valid=$valid"
}

# Try 2: SPI-absolute
Write-Output ""
Write-Output "As SPI-absolute: SPI 0x$([Convert]::ToString($fwsecDataPtr, 16))"
if ($fwsecDataPtr + 64 -lt $vbios.Length) {
    $raw = [BitConverter]::ToString($vbios, $fwsecDataPtr, 64)
    Write-Output "  $raw"
    $hdr = [BitConverter]::ToUInt32($vbios, $fwsecDataPtr)
    $sz = ($hdr -shr 16) -band 0xFFFF
    $ver = ($hdr -shr 8) -band 0xFF
    $valid = $hdr -band 1
    Write-Output "  Hdr: ver=$ver size=0x$([Convert]::ToString($sz, 16)) valid=$valid"
}

# Also check the PMU table's pointer chain
# How does 0x73E1B relate to 0x9181B?
# PMU table ROM offset = 0x861B
# BIT 'p' pointer value = 0x73E1B
# Difference: 0x73E1B - 0x861B = 0x6D800
# 0x6D800 = 448KB... hmm
#
# OR: 0x9181B - 0x9200 = 0x861B  (PMU table is at ROM offset 0x861B)
# The u32 at bit_p.offset should point here
# bit_p.offset = 0x0401
# Data at ROM[0x0401] = 0x73E1B (from the 1B-3E-07-00)
# 0x73E1B != 0x861B
#
# What if the nvbios functions add/subtract an image offset?
# What if the "first PMU table" at SPI 0x9181B was found differently?
# 0x9181B = romBase + 0x861B. And the u32 at offset 0x0401 = 0x73E1B.
# 
# Could there be TWO different tables? The BIT 'p' data might point
# to a V2 or V3 format table that we're not recognizing?

Write-Output ""
Write-Output "=== Checking what 0x73E1B really points to ==="
# ROM[0x73E1B] = SPI[0x7D01B]
# Maybe this is a POINTER to the ucode, not a table header?
# In some VBIOS formats, the PMU data might be stored differently

$off = $romBase + 0x73E1B
Write-Output "SPI[0x$([Convert]::ToString($off, 16))] first 128 bytes:"
$raw = [BitConverter]::ToString($vbios, $off, 128)
Write-Output "  $raw"

# Let me check entry type=0x01 data_ptr=0x15454 to verify interpretation
Write-Output ""
Write-Output "=== Verifying with PMU entry type=0x01 ==="
$pmuPtr1 = 0x15454
$spi1 = $romBase + $pmuPtr1
Write-Output "type=0x01 data_ptr=0x$([Convert]::ToString($pmuPtr1, 16))"
Write-Output "As ROM-relative: SPI 0x$([Convert]::ToString($spi1, 16))"
if ($spi1 + 48 -lt $vbios.Length) {
    $raw = [BitConverter]::ToString($vbios, $spi1, 48)
    Write-Output "  $raw"
    $hdr = [BitConverter]::ToUInt32($vbios, $spi1)
    $sz = ($hdr -shr 16) -band 0xFFFF
    $ver = ($hdr -shr 8) -band 0xFF
    $valid = $hdr -band 1
    Write-Output "  Hdr: ver=$ver size=0x$([Convert]::ToString($sz, 16)) valid=$valid"
}

Write-Output "As SPI-absolute: SPI 0x$([Convert]::ToString($pmuPtr1, 16))"
if ($pmuPtr1 + 48 -lt $vbios.Length) {
    $raw = [BitConverter]::ToString($vbios, $pmuPtr1, 48)
    Write-Output "  $raw"
    $hdr = [BitConverter]::ToUInt32($vbios, $pmuPtr1)
    $sz = ($hdr -shr 16) -band 0xFFFF
    $ver = ($hdr -shr 8) -band 0xFF
    $valid = $hdr -band 1
    Write-Output "  Hdr: ver=$ver size=0x$([Convert]::ToString($sz, 16)) valid=$valid"
}

# Check all other PMU entries' data_ptrs as both ROM-relative and SPI-absolute
Write-Output ""
Write-Output "=== All PMU entries: checking data_ptr interpretation ==="
$entries = @(
    @{type=0x01; ptr=0x15454},
    @{type=0x07; ptr=0x3b8bc},
    @{type=0x08; ptr=0x55898},
    @{type=0x45; ptr=0x1db64},
    @{type=0x85; ptr=0x2ca10},
    @{type=0x49; ptr=0x4b558},
    @{type=0x89; ptr=0x506f8}
)

foreach ($e in $entries) {
    $t = $e.type
    $p = $e.ptr
    Write-Output ""
    Write-Output "--- type=0x$([Convert]::ToString($t, 16)) ptr=0x$([Convert]::ToString($p, 16)) ---"
    
    # ROM-relative
    $spiR = $romBase + $p
    if ($spiR + 8 -lt $vbios.Length) {
        $hdr = [BitConverter]::ToUInt32($vbios, $spiR)
        $sz = ($hdr -shr 16) -band 0xFFFF
        $ver = ($hdr -shr 8) -band 0xFF
        $valid = $hdr -band 1
        Write-Output "  ROM-rel SPI 0x$([Convert]::ToString($spiR, 16)): hdr=0x$([Convert]::ToString($hdr, 16)) ver=$ver sz=0x$([Convert]::ToString($sz, 16)) valid=$valid"
    } else {
        Write-Output "  ROM-rel: BEYOND VBIOS"
    }
    
    # SPI-absolute
    if ($p + 8 -lt $vbios.Length) {
        $hdr = [BitConverter]::ToUInt32($vbios, $p)
        $sz = ($hdr -shr 16) -band 0xFFFF
        $ver = ($hdr -shr 8) -band 0xFF
        $valid = $hdr -band 1
        Write-Output "  SPI-abs  0x$([Convert]::ToString($p, 16)): hdr=0x$([Convert]::ToString($hdr, 16)) ver=$ver sz=0x$([Convert]::ToString($sz, 16)) valid=$valid"
    }
}
