# _find_fwsec6.ps1 - Determine correct base for PMU data_ptr values
# PMU table at SPI 0x9181B: entries have data_ptr values that need a base offset
# Entry type=0x85 (FWSEC) data_ptr=0x2CA10
#
# Strategy: find valid falcon_ucode_desc_v3 headers by trying different bases

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

$pmuTableSPI = 0x9181B
$entries = @(
    @{type=0x01; ptr=0x15454; name="PMU"},
    @{type=0x07; ptr=0x3b8bc; name="FWSEC-DBG?"},
    @{type=0x08; ptr=0x55898; name="GSP-FMC?"},
    @{type=0x45; ptr=0x1db64; name="SEC2?"},
    @{type=0x85; ptr=0x2ca10; name="FWSEC_PROD"},
    @{type=0x49; ptr=0x4b558; name="ACR?"},
    @{type=0x89; ptr=0x506f8; name="FWSEC_PROD2?"}
)

# Try different base offsets
$bases = @(
    @{name="SPI base 0"; base=0},
    @{name="ROM base 0x9200"; base=0x9200},
    @{name="After ROM 0x2D800"; base=0x2D800},
    @{name="NVGI+hdr 0x24"; base=0x24},
    @{name="PMU table area"; base=$pmuTableSPI - 0x2CA10}
)

# Actually, the data_ptr might be relative to the PMU table itself
# Or relative to some firmware region base

# Let me first find the pattern: For FWSEC, data_ptr = 0x2CA10
# The FWSEC ucode descriptor v3 should have:
# - Hdr with valid=1, ver=3, reasonable size (0x2C-0x100)
# - IMEM/DMEM sizes in range (0x100-0x80000)

# Brute-force: scan for valid v3 desc with IMEM+DMEM that fits in the SPI dump
Write-Output "=== Scanning SPI flash for v3 falcon descriptors matching FWSEC ==="

$candidates = @()
for ($i = 0x10000; $i -lt $vbios.Length - 48; $i += 4) {
    $hdr = [BitConverter]::ToUInt32($vbios, $i)
    $valid = $hdr -band 1
    $ver = ($hdr -shr 8) -band 0xFF
    $sz = ($hdr -shr 16) -band 0xFFFF
    
    if ($valid -eq 1 -and $ver -eq 3 -and $sz -eq 0x2C) {
        # Standard v3 descriptor size is 0x2C (44 bytes)
        $stored = [BitConverter]::ToUInt32($vbios, $i + 4)
        $pkcOff = [BitConverter]::ToUInt32($vbios, $i + 8)
        $ifaceOff = [BitConverter]::ToUInt32($vbios, $i + 12)
        $imemBase = [BitConverter]::ToUInt32($vbios, $i + 16)
        $imemSize = [BitConverter]::ToUInt32($vbios, $i + 20)
        $imemVirt = [BitConverter]::ToUInt32($vbios, $i + 24)
        $dmemBase = [BitConverter]::ToUInt32($vbios, $i + 28)
        $dmemSize = [BitConverter]::ToUInt32($vbios, $i + 32)
        $engineId = [BitConverter]::ToUInt16($vbios, $i + 36)
        $ucodeId = $vbios[$i + 38]
        $sigCount = $vbios[$i + 39]
        
        if ($imemSize -gt 0x100 -and $imemSize -lt 0x80000 -and $dmemSize -gt 0x100 -and $dmemSize -lt 0x80000 -and $stored -gt 0 -and $stored -lt 0x200000) {
            Write-Output ""
            Write-Output "v3 desc at SPI 0x$([Convert]::ToString($i, 16)):"
            Write-Output "  StoredSize=0x$([Convert]::ToString($stored, 16)) PKCOff=0x$([Convert]::ToString($pkcOff, 16)) IfaceOff=0x$([Convert]::ToString($ifaceOff, 16))"
            Write-Output "  IMEM: base=0x$([Convert]::ToString($imemBase, 16)) size=0x$([Convert]::ToString($imemSize, 16)) virt=0x$([Convert]::ToString($imemVirt, 16))"
            Write-Output "  DMEM: base=0x$([Convert]::ToString($dmemBase, 16)) size=0x$([Convert]::ToString($dmemSize, 16))"
            Write-Output "  EngineId=0x$([Convert]::ToString($engineId, 16)) UcodeId=$ucodeId SigCount=$sigCount"
            
            # Check if IMEM data follows the descriptor
            $ucodeStart = $i + $sz
            if ($ucodeStart + $imemSize + $dmemSize -le $vbios.Length) {
                $imemFirst = [BitConverter]::ToString($vbios, $ucodeStart, [Math]::Min(16, $vbios.Length - $ucodeStart))
                Write-Output "  IMEM data: $imemFirst"
                
                # Check for difference from FWSEC data_ptr
                $diffFromFwsec = $i - 0x2CA10
                Write-Output "  Offset from FWSEC ptr 0x2CA10: 0x$([Convert]::ToString($diffFromFwsec, 16)) = $diffFromFwsec"
            }
            $candidates += $i
        }
    }
}

# Also scan for v2 descriptors
Write-Output ""
Write-Output "=== Scanning for v2 falcon descriptors ==="
for ($i = 0x10000; $i -lt $vbios.Length - 60; $i += 4) {
    $hdr = [BitConverter]::ToUInt32($vbios, $i)
    $valid = $hdr -band 1
    $ver = ($hdr -shr 8) -band 0xFF
    $sz = ($hdr -shr 16) -band 0xFFFF
    
    if ($valid -eq 1 -and $ver -eq 2 -and $sz -ge 0x34 -and $sz -le 0x40) {
        $stored = [BitConverter]::ToUInt32($vbios, $i + 4)
        $imemBase = [BitConverter]::ToUInt32($vbios, $i + 20)
        $imemSize = [BitConverter]::ToUInt32($vbios, $i + 24)
        $imemSecBase = [BitConverter]::ToUInt32($vbios, $i + 32)
        $imemSecSize = [BitConverter]::ToUInt32($vbios, $i + 36)
        $dmemBase = [BitConverter]::ToUInt32($vbios, $i + 44)
        $dmemSize = [BitConverter]::ToUInt32($vbios, $i + 48)
        
        if ($imemSize -gt 0x100 -and $imemSize -lt 0x80000 -and $dmemSize -gt 0x100 -and $dmemSize -lt 0x80000 -and $stored -gt 0 -and $stored -lt 0x200000) {
            Write-Output ""
            Write-Output "v2 desc at SPI 0x$([Convert]::ToString($i, 16)):"
            Write-Output "  StoredSize=0x$([Convert]::ToString($stored, 16))"
            Write-Output "  IMEM: base=0x$([Convert]::ToString($imemBase, 16)) size=0x$([Convert]::ToString($imemSize, 16))"
            Write-Output "  IMEM Secure: base=0x$([Convert]::ToString($imemSecBase, 16)) size=0x$([Convert]::ToString($imemSecSize, 16))"
            Write-Output "  DMEM: base=0x$([Convert]::ToString($dmemBase, 16)) size=0x$([Convert]::ToString($dmemSize, 16))"
            
            $diffFromFwsec = $i - 0x2CA10
            Write-Output "  Offset from FWSEC ptr 0x2CA10: 0x$([Convert]::ToString($diffFromFwsec, 16))"
            
            # Check if engineId hints at FWSEC
            $raw = [BitConverter]::ToString($vbios, $i, [Math]::Min(56, $vbios.Length - $i))
            Write-Output "  Raw: $raw"
        }
    }
}

# Check what the PMU entry data_ptrs look like as v2/v3 descriptors
# with the base = PMU table SPI offset - PMU entry ptr
# PMU table is at SPI 0x9181B. The 'p' token pointer was 0x73E1B.
# If the PMU table is at bios[0x73E1B + some_offset], then:
# base = 0x9181B - 0x73E1B = 0x1DA00
# But let me check: if base=0x1DA00, then FWSEC at base+0x2CA10 = 0x4A410
Write-Output ""
Write-Output "=== Trying PMU data_ptrs with inferred base 0x1DA00 ==="
$inferredBase = 0x1DA00

foreach ($e in $entries) {
    $absOff = $inferredBase + $e.ptr
    if ($absOff + 48 -lt $vbios.Length) {
        $hdr = [BitConverter]::ToUInt32($vbios, $absOff)
        $valid = $hdr -band 1
        $ver = ($hdr -shr 8) -band 0xFF
        $sz = ($hdr -shr 16) -band 0xFFFF
        Write-Output "  type=0x$([Convert]::ToString($e.type, 16)) at SPI 0x$([Convert]::ToString($absOff, 16)): hdr=0x$([Convert]::ToString($hdr, 16)) ver=$ver sz=0x$([Convert]::ToString($sz, 16)) valid=$valid"
    }
}

# Hmm let me also try: the PMU table's data_ptrs might be relative to
# the start of a firmware region that begins right after the PCI ROM chain.
# After ROM chain: SPI 0x2D800 (from previous analysis)
Write-Output ""
Write-Output "=== Trying base = 0x2D800 (after PCI ROM chain) ==="
$afterRomBase = 0x2D800

foreach ($e in $entries) {
    $absOff = $afterRomBase + $e.ptr
    if ($absOff + 48 -lt $vbios.Length -and $absOff -ge 0) {
        $hdr = [BitConverter]::ToUInt32($vbios, $absOff)
        $valid = $hdr -band 1
        $ver = ($hdr -shr 8) -band 0xFF
        $sz = ($hdr -shr 16) -band 0xFFFF
        Write-Output "  type=0x$([Convert]::ToString($e.type, 16)) at SPI 0x$([Convert]::ToString($absOff, 16)): hdr=0x$([Convert]::ToString($hdr, 16)) ver=$ver sz=0x$([Convert]::ToString($sz, 16)) valid=$valid"
    }
}

# Try base = 0 (SPI absolute)
Write-Output ""
Write-Output "=== PMU data_ptrs as SPI absolute ==="
foreach ($e in $entries) {
    $absOff = $e.ptr
    if ($absOff + 48 -lt $vbios.Length -and $absOff -ge 0) {
        $hdr = [BitConverter]::ToUInt32($vbios, $absOff)
        $valid = $hdr -band 1
        $ver = ($hdr -shr 8) -band 0xFF
        $sz = ($hdr -shr 16) -band 0xFFFF
        Write-Output "  type=0x$([Convert]::ToString($e.type, 16)) at SPI 0x$([Convert]::ToString($absOff, 16)): hdr=0x$([Convert]::ToString($hdr, 16)) ver=$ver sz=0x$([Convert]::ToString($sz, 16)) valid=$valid"
        
        if ($valid -eq 1 -and ($ver -eq 2 -or $ver -eq 3) -and $sz -ge 0x20 -and $sz -le 0x100) {
            Write-Output "    *** VALID DESCRIPTOR! ***"
            if ($ver -eq 3 -and $absOff + 44 -lt $vbios.Length) {
                $stored = [BitConverter]::ToUInt32($vbios, $absOff + 4)
                $imemSize = [BitConverter]::ToUInt32($vbios, $absOff + 20)
                $dmemSize = [BitConverter]::ToUInt32($vbios, $absOff + 32)
                $engineId = [BitConverter]::ToUInt16($vbios, $absOff + 36)
                $ucodeId = $vbios[$absOff + 38]
                $sigCount = $vbios[$absOff + 39]
                Write-Output "    Stored=$stored IMEM=0x$([Convert]::ToString($imemSize, 16)) DMEM=0x$([Convert]::ToString($dmemSize, 16)) Engine=0x$([Convert]::ToString($engineId, 16)) UcodeId=$ucodeId SigCount=$sigCount"
            }
        }
    }
}
