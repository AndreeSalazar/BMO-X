# _find_fwsec.ps1 — Parse VBIOS BIT 'p' token to find FWSEC (type 0x85)
# Following nouveau's exact algorithm: BIT -> token 'p' -> PMU table -> entry type 0x85
# 
# The VBIOS is an NVGI SPI flash dump. PCI ROM at offset 0x9200.
# BIT table at absolute 0x93B2 (0xB8FF prefix at 0x93B0).
# BIT header: ver=0x0100, hdr_size=12, token_size=6, count=18.
# Nouveau BIT parsing: entries start at bit_offset + 12, stride from byte +9.

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
Write-Output "VBIOS size: $($vbios.Length) bytes"

# ROM base (first PCI ROM in NVGI dump)
$romBase = 0x9200

# Find BIT: scan for FF B8 42 49 54 (0xFF 0xB8 'B' 'I' 'T')
$bitOff = -1
for ($i = 0; $i -lt $vbios.Length - 5; $i++) {
    if ($vbios[$i] -eq 0xFF -and $vbios[$i+1] -eq 0xB8 -and $vbios[$i+2] -eq 0x42 -and $vbios[$i+3] -eq 0x49 -and $vbios[$i+4] -eq 0x54) {
        $bitOff = $i + 2  # nouveau sets bit_offset to the 'B' of "BIT"
        Write-Output "BIT found at absolute 0x$([Convert]::ToString($bitOff, 16)) (prefix at 0x$([Convert]::ToString($i, 16)))"
        break
    }
}

if ($bitOff -lt 0) {
    Write-Output "ERROR: BIT not found"
    exit 1
}

# BIT header (from 'B' of "BIT"):
#   +0: 'B','I','T',0x00  (4 bytes magic)
#   +4: BCD version (2 bytes) 
#   +5: ?
#   +7: header_size (1 byte) — NOT USED by nouveau for entry offset
#   +8: token_size/stride (1 byte)  — nouveau reads from bitOff+9
#   +9: entry stride (1 byte)       — nouveau: nvbios_rd08(bios, bios->bit_offset + 9)
#   +10: entry count (1 byte)       — nouveau: nvbios_rd08(bios, bios->bit_offset + 10)
#   +12: entries start              — nouveau: entry = bios->bit_offset + 12

$entryStride = $vbios[$bitOff + 9]
$entryCount = $vbios[$bitOff + 10]
$entriesStart = $bitOff + 12

Write-Output "BIT: stride=$entryStride, count=$entryCount, entries at 0x$([Convert]::ToString($entriesStart, 16))"

# Parse all BIT entries
Write-Output ""
Write-Output "=== BIT Entries ==="
$pTokenOffset = -1
$pTokenVersion = 0
$pTokenLength = 0

for ($e = 0; $e -lt $entryCount; $e++) {
    $eOff = $entriesStart + $e * $entryStride
    $id = $vbios[$eOff]
    $ver = $vbios[$eOff + 1]
    $len = [BitConverter]::ToUInt16($vbios, $eOff + 2)
    $ptr = [BitConverter]::ToUInt16($vbios, $eOff + 4)
    
    $ch = [char]$id
    $absPtr = $romBase + $ptr
    Write-Output ("  Entry[$e]: id=0x{0:X2} ('{1}') ver={2} len={3} ptr=0x{4:X4} -> abs=0x{5:X}" -f $id, $ch, $ver, $len, $ptr, $absPtr)
    
    # Token 'p' = 0x70
    if ($id -eq 0x70) {
        $pTokenOffset = $absPtr
        $pTokenVersion = $ver
        $pTokenLength = $len
        Write-Output "    *** Found 'p' token (Falcon/PMU data) ***"
    }
}

if ($pTokenOffset -lt 0) {
    Write-Output "ERROR: BIT 'p' token not found"
    exit 1
}

Write-Output ""
Write-Output "=== Parsing BIT 'p' Token ==="
Write-Output "  Token offset=0x$([Convert]::ToString($pTokenOffset, 16)), version=$pTokenVersion, length=$pTokenLength"

# Dump raw data at 'p' token offset
$rawP = [BitConverter]::ToString($vbios, $pTokenOffset, [Math]::Min(16, $vbios.Length - $pTokenOffset))
Write-Output "  Raw data: $rawP"

# nvbios_pmuTe: if version==2 and length>=4, read u32 pointer
if ($pTokenVersion -eq 2 -and $pTokenLength -ge 4) {
    $pmuTablePtr = [BitConverter]::ToUInt32($vbios, $pTokenOffset)
    Write-Output "  PMU table pointer (ROM-relative): 0x$([Convert]::ToString($pmuTablePtr, 16))"
    
    # This pointer is relative to ROM image start
    $pmuTableAbs = $romBase + $pmuTablePtr
    Write-Output "  PMU table absolute: 0x$([Convert]::ToString($pmuTableAbs, 16))"
    
    if ($pmuTableAbs -ge $vbios.Length) {
        Write-Output "  ERROR: PMU table pointer 0x$([Convert]::ToString($pmuTableAbs, 16)) is beyond VBIOS $($vbios.Length) bytes"
        Write-Output ""
        Write-Output "  TRYING: pointer might be absolute within SPI dump, not relative to ROM base"
        $pmuTableAbs = $pmuTablePtr
        Write-Output "  PMU table as absolute SPI offset: 0x$([Convert]::ToString($pmuTableAbs, 16))"
    }
    
    if ($pmuTableAbs -lt $vbios.Length - 4) {
        $pmuVer = $vbios[$pmuTableAbs]
        $pmuHdr = $vbios[$pmuTableAbs + 1]
        $pmuEntryLen = $vbios[$pmuTableAbs + 2]
        $pmuEntryCount = $vbios[$pmuTableAbs + 3]
        
        Write-Output "  PMU table: ver=$pmuVer, hdr_size=$pmuHdr, entry_size=$pmuEntryLen, entry_count=$pmuEntryCount"
        $rawPmu = [BitConverter]::ToString($vbios, $pmuTableAbs, [Math]::Min(32, $vbios.Length - $pmuTableAbs))
        Write-Output "  Raw PMU header: $rawPmu"
        
        # Sanity check
        if ($pmuVer -ge 1 -and $pmuVer -le 3 -and $pmuHdr -ge 4 -and $pmuHdr -le 16 -and $pmuEntryLen -ge 4 -and $pmuEntryLen -le 32 -and $pmuEntryCount -ge 1 -and $pmuEntryCount -le 32) {
            Write-Output "  PMU table header looks VALID!"
            
            Write-Output ""
            Write-Output "=== PMU Table Entries ==="
            for ($i = 0; $i -lt $pmuEntryCount; $i++) {
                $entOff = $pmuTableAbs + $pmuHdr + $i * $pmuEntryLen
                if ($entOff + $pmuEntryLen -gt $vbios.Length) { break }
                
                $type = $vbios[$entOff]
                # nvbios_pmuEp: type at +0, data (u32) at +2
                $dataPtr = [BitConverter]::ToUInt32($vbios, $entOff + 2)
                
                $raw = [BitConverter]::ToString($vbios, $entOff, [Math]::Min($pmuEntryLen, $vbios.Length - $entOff))
                Write-Output ("  PMU[$i]: type=0x{0:X2} data_ptr=0x{1:X8} raw=$raw" -f $type, $dataPtr)
                
                if ($type -eq 0x85) {
                    Write-Output "    *** FWSEC (type 0x85) FOUND! ***"
                    Write-Output "    Ucode descriptor at ROM offset 0x$([Convert]::ToString($dataPtr, 16))"
                    $descAbs = $romBase + $dataPtr
                    Write-Output "    Ucode descriptor absolute: 0x$([Convert]::ToString($descAbs, 16))"
                    
                    if ($descAbs -ge $vbios.Length) {
                        Write-Output "    Trying as SPI-absolute..."
                        $descAbs = $dataPtr
                    }
                    
                    if ($descAbs -lt $vbios.Length - 64) {
                        Write-Output ""
                        Write-Output "=== FWSEC Ucode Descriptor ==="
                        $raw64 = [BitConverter]::ToString($vbios, $descAbs, 64)
                        Write-Output "  Raw 64 bytes: $raw64"
                        
                        $hdr = [BitConverter]::ToUInt32($vbios, $descAbs)
                        $hdrSize = ($hdr -shr 16) -band 0xFFFF
                        $hdrVer = ($hdr -shr 8) -band 0xFF
                        $hdrValid = $hdr -band 0x1
                        Write-Output "  Hdr=0x$([Convert]::ToString($hdr, 16)): size=$hdrSize ver=$hdrVer valid=$hdrValid"
                        
                        if ($hdrVer -eq 3) {
                            Write-Output "  *** Ampere v3 descriptor ***"
                            $storedSize = [BitConverter]::ToUInt32($vbios, $descAbs + 4)
                            $pkcOff = [BitConverter]::ToUInt32($vbios, $descAbs + 8)
                            $ifaceOff = [BitConverter]::ToUInt32($vbios, $descAbs + 12)
                            $imemBase = [BitConverter]::ToUInt32($vbios, $descAbs + 16)
                            $imemSize = [BitConverter]::ToUInt32($vbios, $descAbs + 20)
                            $imemVirt = [BitConverter]::ToUInt32($vbios, $descAbs + 24)
                            $dmemBase = [BitConverter]::ToUInt32($vbios, $descAbs + 28)
                            $dmemSize = [BitConverter]::ToUInt32($vbios, $descAbs + 32)
                            $engineId = [BitConverter]::ToUInt16($vbios, $descAbs + 36)
                            $ucodeId = $vbios[$descAbs + 38]
                            $sigCount = $vbios[$descAbs + 39]
                            $sigVers = [BitConverter]::ToUInt16($vbios, $descAbs + 40)
                            
                            Write-Output "  StoredSize = 0x$([Convert]::ToString($storedSize, 16)), $storedSize bytes"
                            Write-Output "  PKCDataOffset = 0x$([Convert]::ToString($pkcOff, 16))"
                            Write-Output "  InterfaceOffset = 0x$([Convert]::ToString($ifaceOff, 16))"
                            Write-Output "  IMEM: base=0x$([Convert]::ToString($imemBase, 16)) size=0x$([Convert]::ToString($imemSize, 16))"
                            Write-Output "  IMEM virt=0x$([Convert]::ToString($imemVirt, 16))"
                            Write-Output "  DMEM: base=0x$([Convert]::ToString($dmemBase, 16)) size=0x$([Convert]::ToString($dmemSize, 16))"
                            Write-Output "  EngineIdMask = 0x$([Convert]::ToString($engineId, 16))"
                            Write-Output "  UcodeId = $ucodeId"
                            Write-Output "  SignatureCount = $sigCount"
                            Write-Output "  SignatureVersions = 0x$([Convert]::ToString($sigVers, 16))"
                            
                            # The ucode blob starts after the descriptor header
                            $ucodeStart = $descAbs + $hdrSize
                            $totalUcodeSize = $imemSize + $dmemSize
                            Write-Output ""
                            Write-Output "  Ucode blob starts at absolute 0x$([Convert]::ToString($ucodeStart, 16))"
                            Write-Output "  Total ucode size = 0x$([Convert]::ToString($totalUcodeSize, 16)) IMEM+DMEM"
                            
                            # Signatures start at descriptor + 0x2C
                            $sigStart = $descAbs + 0x2C
                            $sigSize = 96 * 4  # RSA-3072 = 384 bytes per sig
                            Write-Output "  Signatures at 0x$([Convert]::ToString($sigStart, 16)), $sigCount x $sigSize bytes each"
                            
                            if ($ucodeStart + $totalUcodeSize -le $vbios.Length) {
                                Write-Output "  IMEM first 32 bytes:"
                                $imemRaw = [BitConverter]::ToString($vbios, $ucodeStart, [Math]::Min(32, $vbios.Length - $ucodeStart))
                                Write-Output "    $imemRaw"
                                
                                $dmemStart = $ucodeStart + $imemSize
                                Write-Output "  DMEM first 32 bytes (at 0x$([Convert]::ToString($dmemStart, 16))):"
                                $dmemRaw = [BitConverter]::ToString($vbios, $dmemStart, [Math]::Min(32, $vbios.Length - $dmemStart))
                                Write-Output "    $dmemRaw"
                                
                                Write-Output ""
                                Write-Output "SUCCESS: FWSEC ucode found and parsed!"
                                Write-Output "  Descriptor at: 0x$([Convert]::ToString($descAbs, 16))"
                                Write-Output "  IMEM at: 0x$([Convert]::ToString($ucodeStart, 16)), size=0x$([Convert]::ToString($imemSize, 16))"
                                Write-Output "  DMEM at: 0x$([Convert]::ToString($dmemStart, 16)), size=0x$([Convert]::ToString($dmemSize, 16))"
                            } else {
                                Write-Output "  WARNING: ucode extends beyond VBIOS!"
                            }
                        } elseif ($hdrVer -eq 2) {
                            Write-Output "  *** Turing v2 descriptor ***"
                            $storedSize = [BitConverter]::ToUInt32($vbios, $descAbs + 4)
                            $uncompSize = [BitConverter]::ToUInt32($vbios, $descAbs + 8)
                            $virtEntry = [BitConverter]::ToUInt32($vbios, $descAbs + 12)
                            $ifaceOff = [BitConverter]::ToUInt32($vbios, $descAbs + 16)
                            $imemBase = [BitConverter]::ToUInt32($vbios, $descAbs + 20)
                            $imemSize = [BitConverter]::ToUInt32($vbios, $descAbs + 24)
                            $imemVirt = [BitConverter]::ToUInt32($vbios, $descAbs + 28)
                            $imemSecBase = [BitConverter]::ToUInt32($vbios, $descAbs + 32)
                            $imemSecSize = [BitConverter]::ToUInt32($vbios, $descAbs + 36)
                            $dmemOffset = [BitConverter]::ToUInt32($vbios, $descAbs + 40)
                            $dmemBase = [BitConverter]::ToUInt32($vbios, $descAbs + 44)
                            $dmemSize = [BitConverter]::ToUInt32($vbios, $descAbs + 48)
                            
                            Write-Output "  StoredSize=$storedSize UncompSize=$uncompSize"
                            Write-Output "  VirtualEntry=0x$([Convert]::ToString($virtEntry, 16))"
                            Write-Output "  InterfaceOffset=0x$([Convert]::ToString($ifaceOff, 16))"
                            Write-Output "  IMEM: base=0x$([Convert]::ToString($imemBase, 16)) size=$imemSize virt=0x$([Convert]::ToString($imemVirt, 16))"
                            Write-Output "  IMEM Secure: base=0x$([Convert]::ToString($imemSecBase, 16)) size=$imemSecSize"
                            Write-Output "  DMEM: offset=$dmemOffset base=0x$([Convert]::ToString($dmemBase, 16)) size=$dmemSize"
                        } else {
                            Write-Output "  Unknown descriptor version: $hdrVer"
                        }
                    }
                }
            }
        } else {
            Write-Output "  PMU table header looks INVALID (bad values)"
            Write-Output "  Trying: the u32 at 'p' payload might NOT be ROM-relative"
            Write-Output ""
            
            # nouveau reads the 'p' token pointer data directly from the VBIOS
            # The 'p' offset field in BIT entry is already an absolute offset within the BIOS image
            # The data at that offset is a pointer that's also relative to start of BIOS
            # For NVGI dumps, "start of BIOS" might mean start of the SPI image (offset 0), not ROM base
            
            # Let's try the pointer as-is (relative to offset 0)
            Write-Output "=== Trying PMU pointer as SPI-absolute ==="
            $pmuTableAbs2 = $pmuTablePtr
            if ($pmuTableAbs2 -lt $vbios.Length - 4) {
                $pmuVer2 = $vbios[$pmuTableAbs2]
                $pmuHdr2 = $vbios[$pmuTableAbs2 + 1]
                $pmuEntryLen2 = $vbios[$pmuTableAbs2 + 2]
                $pmuEntryCount2 = $vbios[$pmuTableAbs2 + 3]
                Write-Output "  At SPI offset 0x$([Convert]::ToString($pmuTableAbs2, 16)): ver=$pmuVer2 hdr=$pmuHdr2 entry_size=$pmuEntryLen2 count=$pmuEntryCount2"
                $rawPmu2 = [BitConverter]::ToString($vbios, $pmuTableAbs2, [Math]::Min(32, $vbios.Length - $pmuTableAbs2))
                Write-Output "  Raw: $rawPmu2"
            }
            
            # Also try: maybe the BIT 'p' offset field is already absolute in the SPI dump
            # (not relative to ROM base). Let's check what's at the raw BIT ptr value
            $rawBitPtr = [BitConverter]::ToUInt16($vbios, $entriesStart + 18 * 0 + 4)  # wrong, let's find 'p' entry again
            
            # Re-find the 'p' entry to get its raw ptr
            for ($e2 = 0; $e2 -lt $entryCount; $e2++) {
                $eOff2 = $entriesStart + $e2 * $entryStride
                if ($vbios[$eOff2] -eq 0x70) {
                    $rawPtr = [BitConverter]::ToUInt16($vbios, $eOff2 + 4)
                    Write-Output ""
                    Write-Output "  BIT 'p' raw ptr = 0x$([Convert]::ToString($rawPtr, 16))"
                    Write-Output "  As absolute (SPI + 0): reading at 0x$([Convert]::ToString($rawPtr, 16))"
                    if ($rawPtr -lt $vbios.Length - 4) {
                        $d = [BitConverter]::ToString($vbios, $rawPtr, [Math]::Min(16, $vbios.Length - $rawPtr))
                        Write-Output "  Data at raw ptr: $d"
                    }
                    Write-Output "  As ROM-relative ($romBase + 0x$([Convert]::ToString($rawPtr, 16))):"
                    $absP = $romBase + $rawPtr
                    if ($absP -lt $vbios.Length - 4) {
                        $d2 = [BitConverter]::ToString($vbios, $absP, [Math]::Min(16, $vbios.Length - $absP))
                        Write-Output "  Data at ROM+ptr: $d2"
                        
                        # Read the u32 at this location (this is what nouveau reads)
                        $pmuPtr2 = [BitConverter]::ToUInt32($vbios, $absP)
                        Write-Output "  u32 value = 0x$([Convert]::ToString($pmuPtr2, 16))"
                        
                        # Now THIS pointer — try both ROM-relative and SPI-absolute
                        Write-Output ""
                        Write-Output "  Following pointer 0x$([Convert]::ToString($pmuPtr2, 16)):"
                        $tryAbs = $romBase + $pmuPtr2
                        if ($tryAbs -lt $vbios.Length - 8) {
                            $dd = [BitConverter]::ToString($vbios, $tryAbs, [Math]::Min(16, $vbios.Length - $tryAbs))
                            Write-Output "    ROM-relative (0x$([Convert]::ToString($tryAbs, 16))): $dd"
                            Write-Output "    As PMU header: ver=$($vbios[$tryAbs]) hdr=$($vbios[$tryAbs+1]) stride=$($vbios[$tryAbs+2]) count=$($vbios[$tryAbs+3])"
                        }
                        if ($pmuPtr2 -lt $vbios.Length - 8) {
                            $dd2 = [BitConverter]::ToString($vbios, $pmuPtr2, [Math]::Min(16, $vbios.Length - $pmuPtr2))
                            Write-Output "    SPI-absolute (0x$([Convert]::ToString($pmuPtr2, 16))): $dd2"
                            Write-Output "    As PMU header: ver=$($vbios[$pmuPtr2]) hdr=$($vbios[$pmuPtr2+1]) stride=$($vbios[$pmuPtr2+2]) count=$($vbios[$pmuPtr2+3])"
                        }
                    }
                    break
                }
            }
        }
    }
} else {
    Write-Output "  BIT 'p' token version=$pTokenVersion length=$pTokenLength — not version 2 with length>=4"
    Write-Output "  Nouveau only handles version 2. This VBIOS may use a different format."
}

# Also check the nvfw_bin_hdr we found at 0x1C0E24
Write-Output ""
Write-Output "=== Checking nvfw_bin_hdr at 0x1C0E24 ==="
$nfwOff = 0x1C0E24
if ($nfwOff + 24 -lt $vbios.Length) {
    $magic = [BitConverter]::ToUInt32($vbios, $nfwOff)
    $binVer = [BitConverter]::ToUInt32($vbios, $nfwOff + 4)
    $binSize = [BitConverter]::ToUInt32($vbios, $nfwOff + 8)
    $hdrOff = [BitConverter]::ToUInt32($vbios, $nfwOff + 12)
    $dataOff = [BitConverter]::ToUInt32($vbios, $nfwOff + 16)
    $dataSz = [BitConverter]::ToUInt32($vbios, $nfwOff + 20)
    Write-Output "  magic=0x$([Convert]::ToString($magic, 16))"
    Write-Output "  bin_ver=$binVer bin_size=$binSize"
    Write-Output "  hdr_off=0x$([Convert]::ToString($hdrOff, 16))"
    Write-Output "  data_off=0x$([Convert]::ToString($dataOff, 16))"
    Write-Output "  data_sz=0x$([Convert]::ToString($dataSz, 16)), $dataSz bytes"
    
    $raw = [BitConverter]::ToString($vbios, $nfwOff, 64)
    Write-Output "  Raw 64 bytes = $raw"
    
    if ($dataOff -gt 0 -and $dataSz -gt 0) {
        $dataAbs = $nfwOff + $dataOff
        Write-Output "  Data at absolute 0x$([Convert]::ToString($dataAbs, 16)):"
        if ($dataAbs + 32 -lt $vbios.Length) {
            $dataRaw = [BitConverter]::ToString($vbios, $dataAbs, [Math]::Min(32, $vbios.Length - $dataAbs))
            Write-Output "    $dataRaw"
        }
    }
}

# Brute-force scan for falcon ucode descriptors in the upper part of VBIOS
# Looking for v3 Hdr pattern: bits[0]=1, bits[8:15]=3, bits[16:31]=reasonable size (0x20-0x40)
Write-Output ""
Write-Output "=== Brute-force scan for v3 Falcon ucode descriptors ==="
$found = 0
for ($i = 0x10000; $i -lt $vbios.Length - 44; $i += 4) {
    $word = [BitConverter]::ToUInt32($vbios, $i)
    $valid = $word -band 1
    $ver = ($word -shr 8) -band 0xFF
    $sz = ($word -shr 16) -band 0xFFFF
    
    if ($valid -eq 1 -and $ver -eq 3 -and $sz -ge 0x2C -and $sz -le 0x100) {
        # Check if following fields look reasonable
        $storedSz = [BitConverter]::ToUInt32($vbios, $i + 4)
        $imemBase = [BitConverter]::ToUInt32($vbios, $i + 16)
        $imemSize = [BitConverter]::ToUInt32($vbios, $i + 20)
        $dmemBase = [BitConverter]::ToUInt32($vbios, $i + 28)
        $dmemSize = [BitConverter]::ToUInt32($vbios, $i + 32)
        
        # Falcon IMEM is typically 0-256KB, DMEM similarly bounded
        if ($imemSize -gt 0 -and $imemSize -lt 0x80000 -and $dmemSize -gt 0 -and $dmemSize -lt 0x80000 -and $storedSz -gt 0 -and $storedSz -lt 0x200000) {
            $engineId = [BitConverter]::ToUInt16($vbios, $i + 36)
            $ucodeId = $vbios[$i + 38]
            $sigCount = $vbios[$i + 39]
            $sigVers = [BitConverter]::ToUInt16($vbios, $i + 40)
            
            Write-Output ""
            Write-Output ("  v3 descriptor at 0x{0:X}: sz=0x{1:X} stored=0x{2:X}" -f $i, $sz, $storedSz)
            Write-Output ("    IMEM: base=0x{0:X} size=0x{1:X}" -f $imemBase, $imemSize)
            Write-Output ("    DMEM: base=0x{0:X} size=0x{1:X}" -f $dmemBase, $dmemSize)
            Write-Output ("    EngineId=0x{0:X} UcodeId={1} SigCount={2} SigVers=0x{3:X}" -f $engineId, $ucodeId, $sigCount, $sigVers)
            
            $raw = [BitConverter]::ToString($vbios, $i, [Math]::Min(44, $vbios.Length - $i))
            Write-Output "    Raw: $raw"
            $found++
            if ($found -ge 10) { break }
        }
    }
}

if ($found -eq 0) {
    Write-Output "  No v3 descriptors found via brute force"
}

# Also scan for v2 descriptors
Write-Output ""
Write-Output "=== Brute-force scan for v2 Falcon ucode descriptors ==="
$found2 = 0
for ($i = 0x10000; $i -lt $vbios.Length - 56; $i += 4) {
    $word = [BitConverter]::ToUInt32($vbios, $i)
    $valid = $word -band 1
    $ver = ($word -shr 8) -band 0xFF
    $sz = ($word -shr 16) -band 0xFFFF
    
    if ($valid -eq 1 -and $ver -eq 2 -and $sz -ge 0x30 -and $sz -le 0x100) {
        $storedSz = [BitConverter]::ToUInt32($vbios, $i + 4)
        $imemBase = [BitConverter]::ToUInt32($vbios, $i + 20)
        $imemSize = [BitConverter]::ToUInt32($vbios, $i + 24)
        $dmemBase = [BitConverter]::ToUInt32($vbios, $i + 44)
        $dmemSize = [BitConverter]::ToUInt32($vbios, $i + 48)
        
        if ($imemSize -gt 0 -and $imemSize -lt 0x80000 -and $dmemSize -gt 0 -and $dmemSize -lt 0x80000 -and $storedSz -gt 0 -and $storedSz -lt 0x200000) {
            Write-Output ""
            Write-Output ("  v2 descriptor at 0x{0:X}: sz=0x{1:X} stored=0x{2:X}" -f $i, $sz, $storedSz)
            Write-Output ("    IMEM: base=0x{0:X} size=0x{1:X}" -f $imemBase, $imemSize)
            Write-Output ("    DMEM: base=0x{0:X} size=0x{1:X}" -f $dmemBase, $dmemSize)
            
            $raw = [BitConverter]::ToString($vbios, $i, [Math]::Min(56, $vbios.Length - $i))
            Write-Output "    Raw: $raw"
            $found2++
            if ($found2 -ge 10) { break }
        }
    }
}
if ($found2 -eq 0) {
    Write-Output "  No v2 descriptors found via brute force"
}
