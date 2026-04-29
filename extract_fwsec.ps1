# extract_fwsec.ps1 — Extract FWSEC-FRTS Falcon HS ucode from NVIDIA VBIOS (NVGI dump)
# Target: GA106 (RTX 3060 12G) — PCI ID 10DE:2504
#
# Lookup path:
#   NVGI dump → PCI ROM (55 AA) → BIT header → token 0x70 (Falcon Data)
#   → PMU lookup table → app_id 0x85 (FWSEC_PROD) → FalconUCodeDescV3
#   → extract signatures + IMEM + DMEM → pack into fwsec_ga106.bin
#
# Output: fwsec_ga106.bin with header:
#   [16-byte header][descriptor][signatures][imem_code][dmem_data]
#
# Header format (16 bytes):
#   magic:    u32 = 0x46575345  ('FWSE')
#   desc_off: u32 = offset of FalconUCodeDescV3 in output
#   imem_off: u32 = offset of IMEM code
#   dmem_off: u32 = offset of DMEM data

param(
    [string]$VbiosPath = "USB_boot\firmware\vbios_rtx3060.rom",
    [string]$OutputPath = "USB_boot\firmware\fwsec_ga106.bin"
)

$ErrorActionPreference = "Stop"

function Read-U8($data, $off) { return $data[$off] }
function Read-U16($data, $off) { return [BitConverter]::ToUInt16($data, $off) }
function Read-U32($data, $off) { return [BitConverter]::ToUInt32($data, $off) }

function Write-Hex($val, $digits=8) {
    return ("0x{0}" -f ([Convert]::ToString($val, 16).PadLeft($digits, '0')))
}

# ── Load VBIOS ──
Write-Output "=== FWSEC Extractor for GA106 ==="
$vbios = [System.IO.File]::ReadAllBytes((Resolve-Path $VbiosPath))
Write-Output ("VBIOS: " + $VbiosPath + " (" + $vbios.Length + " bytes)")

$magic = [System.Text.Encoding]::ASCII.GetString($vbios, 0, 4)
Write-Output ("Magic: " + $magic)
if ($magic -ne "NVGI") {
    Write-Output "WARNING: Expected NVGI magic, got '$magic'"
}

# ── Step 1: Find PCI ROM images ──
Write-Output ""
Write-Output "=== Step 1: Find PCI ROM images ==="
$romBase = -1
$romLegacyLen = 0
$romEfiBase = -1
$romEfiLen = 0

for ($i = 0; $i -lt $vbios.Length - 2; $i++) {
    if ($vbios[$i] -eq 0x55 -and $vbios[$i+1] -eq 0xAA) {
        # Verify PCIR
        $pcirRelOff = Read-U16 $vbios ($i + 0x18)
        $pcirAbs = $i + $pcirRelOff
        if ($pcirAbs + 24 -lt $vbios.Length) {
            $pcirSig = [System.Text.Encoding]::ASCII.GetString($vbios, $pcirAbs, 4)
            if ($pcirSig -eq "PCIR") {
                $vendor = Read-U16 $vbios ($pcirAbs + 4)
                $device = Read-U16 $vbios ($pcirAbs + 6)
                $codeType = $vbios[$pcirAbs + 20]
                $imgLen = (Read-U16 $vbios ($pcirAbs + 16)) * 512

                Write-Output ("  ROM at " + (Write-Hex $i) + ": vendor=" + (Write-Hex $vendor 4) + " device=" + (Write-Hex $device 4) + " type=$codeType len=$imgLen")

                if ($vendor -eq 0x10DE -and $codeType -eq 0) {
                    $romBase = $i
                    $romLegacyLen = $imgLen
                    Write-Output "    -> Legacy x86 ROM (this is our target)"
                }
                if ($vendor -eq 0x10DE -and $codeType -eq 3) {
                    $romEfiBase = $i
                    $romEfiLen = $imgLen
                    Write-Output "    -> EFI ROM"
                }
                # Skip to next image
                if ($imgLen -gt 0) { $i += $imgLen - 1 }
            }
        }
    }
}

if ($romBase -lt 0) {
    Write-Output "ERROR: No PCI legacy ROM found"
    exit 1
}

# ── Step 2: Find BIT table ──
Write-Output ""
Write-Output "=== Step 2: Find BIT table ==="
$bitOff = -1

# Search within the ROM area (and some beyond for safety)
$searchEnd = [Math]::Min($romBase + 0x20000, $vbios.Length - 16)
for ($i = $romBase; $i -lt $searchEnd; $i++) {
    if ($vbios[$i] -eq 0x42 -and $vbios[$i+1] -eq 0x49 -and $vbios[$i+2] -eq 0x54 -and $vbios[$i+3] -eq 0x00) {
        $bitOff = $i
        break
    }
}

if ($bitOff -lt 0) {
    Write-Output "ERROR: BIT table not found"
    exit 1
}

Write-Output ("BIT at absolute " + (Write-Hex $bitOff) + " (ROM+" + (Write-Hex ($bitOff - $romBase)) + ")")

# Check for 0xB8FF prefix 2 bytes before
$hasPrefix = ($bitOff -ge 2 -and $vbios[$bitOff-2] -eq 0xFF -and $vbios[$bitOff-1] -eq 0xB8)
if ($hasPrefix) {
    Write-Output "  Found 0xB8FF prefix at $(Write-Hex ($bitOff-2))"
    $bitHdrStart = $bitOff - 2
} else {
    Write-Output "  No 0xB8FF prefix found, using BIT sig as start"
    $bitHdrStart = $bitOff
}

# Parse BIT header
# bytes at BIT sig: 42 49 54 00 | version(2) | hdr_size(1) | token_size(1) | token_count(1) | checksum(1)
# But the actual layout after "BIT\0" depends on version
$rawHdr = [System.BitConverter]::ToString($vbios, $bitOff, 16)
Write-Output ("  Raw (from BIT): " + $rawHdr)

# Try the Nouveau layout: after "BIT\0", next bytes are version info
# "BIT\0" is 4 bytes, then:
#   byte 4-5: BCD version (u16)  
#   byte 6: header_size (from start of "BIT\0")
#   byte 7: token_entry_size
#   byte 8: token_count
#   byte 9: checksum

$bcdVer = Read-U16 $vbios ($bitOff + 4)
$hdrSize = $vbios[$bitOff + 6]
$tokenSize = $vbios[$bitOff + 7]
$tokenCount = $vbios[$bitOff + 8]
$checksum = $vbios[$bitOff + 9]

Write-Output ("  BCD version: " + (Write-Hex $bcdVer 4))
Write-Output ("  hdr_size: $hdrSize  token_size: $tokenSize  token_count: $tokenCount  checksum: " + (Write-Hex $checksum 2))

# Validate - if token_size is 6 and token_count is reasonable, we have the right layout
if ($tokenSize -ne 6 -or $tokenCount -lt 3 -or $tokenCount -gt 50) {
    # Try alternate layout: hdr_size might be the offset from BIT to first token
    Write-Output "  WARNING: token_size=$tokenSize seems wrong, trying alternate parsing..."
    
    # Some VBIOS have: BIT\0 + version(1) + hdr_total_size(1) where hdr includes BIT\0
    # Then tokens start at bitOff + hdr_total_size, with size 6
    $altHdrSz = $vbios[$bitOff + 5]
    $altTokSz = 6
    Write-Output ("  Alt: hdr_total=" + $altHdrSz + " assuming token_size=6")
    
    # Find token count by scanning
    $altFirst = $bitOff + $altHdrSz
    $altCount = 0
    for ($t = 0; $t -lt 50; $t++) {
        $tOff = $altFirst + $t * $altTokSz
        if ($tOff + $altTokSz -gt $vbios.Length) { break }
        $tId = $vbios[$tOff]
        $tDv = $vbios[$tOff + 1]
        # End marker is usually 0x00 id with 0x00 version and 0 ptr
        if ($tId -eq 0 -and $tDv -eq 0 -and (Read-U16 $vbios ($tOff+4)) -eq 0) { break }
        $altCount++
    }
    
    $tokenCount = $altCount
    $tokenSize = $altTokSz
    $firstTokenOff = $altFirst
    Write-Output ("  Detected $tokenCount tokens starting at " + (Write-Hex $firstTokenOff))
} else {
    # Normal layout: first token at bitOff + hdr_size
    $firstTokenOff = $bitOff + $hdrSize
}

# ── Step 3: Parse BIT tokens ──
Write-Output ""
Write-Output "=== Step 3: BIT Tokens ==="
$falconDataPtr = -1
$falconDataSz = 0

for ($t = 0; $t -lt $tokenCount; $t++) {
    $tOff = $firstTokenOff + $t * $tokenSize
    if ($tOff + $tokenSize -gt $vbios.Length) { break }
    
    $tId = $vbios[$tOff]
    $tDv = $vbios[$tOff + 1]
    $tSz = Read-U16 $vbios ($tOff + 2)
    $tPtr = Read-U16 $vbios ($tOff + 4)
    
    $tChar = if ($tId -ge 0x20 -and $tId -le 0x7E) { [char]$tId } else { ("0x" + [Convert]::ToString($tId, 16)) }
    Write-Output ("  [$t] Token '$tChar' (0x$([Convert]::ToString($tId, 16))): dv=$tDv sz=$tSz ptr=" + (Write-Hex $tPtr 4))
    
    # Look for Falcon Data token (0x70 or 'p')
    if ($tId -eq 0x70) {
        Write-Output "    *** FALCON DATA (0x70) ***"
        $falconDataPtr = $tPtr
        $falconDataSz = $tSz
    }
    
    # Also check for 'F' (0x46) as fallback
    if ($tId -eq 0x46) {
        Write-Output "    *** FALCON DATA ('F') ***"
        if ($falconDataPtr -lt 0) {
            $falconDataPtr = $tPtr
            $falconDataSz = $tSz
        }
    }
}

# If we didn't find Falcon data via tokens, try brute-force search for the PMU lookup table
if ($falconDataPtr -lt 0) {
    Write-Output ""
    Write-Output "WARNING: No Falcon Data token found via BIT"
    Write-Output "Trying brute-force search for PMU lookup table..."
    
    # PMU lookup table starts with: version(1)=1, hdr_size(1)=6, entry_size(1)=6, entry_count(1)
    for ($i = $romBase; $i -lt [Math]::Min($romBase + $vbios.Length, $vbios.Length - 64); $i++) {
        $v = $vbios[$i]
        $hs = $vbios[$i+1]
        $es = $vbios[$i+2]
        $ec = $vbios[$i+3]
        if ($v -eq 1 -and $hs -eq 6 -and $es -eq 6 -and $ec -ge 1 -and $ec -le 16) {
            # Check if entries look valid (app_ids in reasonable range)
            $valid = $true
            for ($e = 0; $e -lt $ec; $e++) {
                $eOff = $i + $hs + $e * $es
                if ($eOff + $es -gt $vbios.Length) { $valid = $false; break }
                $appId = $vbios[$eOff]
                $target = $vbios[$eOff + 1]
                # Reasonable app_ids: 0x01-0xFF, target: 0x00-0x40
                if ($target -gt 0x40) { $valid = $false; break }
            }
            if ($valid) {
                # Check if any entry has app_id = 0x85 (FWSEC_PROD)
                for ($e = 0; $e -lt $ec; $e++) {
                    $eOff = $i + $hs + $e * $es
                    if ($vbios[$eOff] -eq 0x85) {
                        Write-Output ("  Found PMU table with FWSEC_PROD at " + (Write-Hex $i))
                        $falconDataPtr = $i - $romBase  # relative to ROM
                        $falconDataSz = $hs + $ec * $es
                        break
                    }
                }
                if ($falconDataPtr -ge 0) { break }
            }
        }
    }
}

# ── Step 4: Resolve Falcon Data pointer ──
Write-Output ""
Write-Output "=== Step 4: Falcon Data Resolution ==="

if ($falconDataPtr -lt 0) {
    Write-Output "ERROR: Could not find Falcon Data"
    Write-Output ""
    Write-Output "=== Fallback: Dump all BIT token data for manual analysis ==="
    for ($t = 0; $t -lt $tokenCount; $t++) {
        $tOff = $firstTokenOff + $t * $tokenSize
        if ($tOff + $tokenSize -gt $vbios.Length) { break }
        $tId = $vbios[$tOff]
        $tPtr = Read-U16 $vbios ($tOff + 4)
        $tSz = Read-U16 $vbios ($tOff + 2)
        
        if ($tPtr -ne 0 -and $tSz -gt 0) {
            # Resolve pointer relative to ROM base
            $absPtr = $romBase + $tPtr
            if ($absPtr + [Math]::Min($tSz, 32) -lt $vbios.Length) {
                $rawData = [System.BitConverter]::ToString($vbios, $absPtr, [Math]::Min($tSz, 48))
                $tChar = if ($tId -ge 0x20 -and $tId -le 0x7E) { [char]$tId } else { ("0x" + [Convert]::ToString($tId, 16)) }
                Write-Output ("  Token '$tChar' data at abs=" + (Write-Hex $absPtr) + ": " + $rawData)
            }
        }
    }
    
    # Also try: maybe the PMU table pointer is INSIDE one of the token data areas
    Write-Output ""
    Write-Output "=== Searching for FWSEC app_id=0x85 in entire ROM ==="
    for ($i = $romBase; $i -lt [Math]::Min($romBase + 0x80000, $vbios.Length - 8); $i++) {
        if ($vbios[$i] -eq 0x85 -and $vbios[$i+1] -le 0x40) {
            # Could be app_id=0x85, target<=0x40 
            # Check if 6 bytes before looks like a table header
            if ($i -ge 6) {
                $tblStart = $i - 6
                $v2 = $vbios[$tblStart]
                $hs2 = $vbios[$tblStart+1]
                $es2 = $vbios[$tblStart+2]
                $ec2 = $vbios[$tblStart+3]
                if (($v2 -eq 1 -or $v2 -eq 2) -and $hs2 -ge 4 -and $hs2 -le 12 -and $es2 -ge 4 -and $es2 -le 12 -and $ec2 -ge 1 -and $ec2 -le 16) {
                    Write-Output ("  Candidate PMU table at " + (Write-Hex $tblStart) + ": ver=$v2 hdr=$hs2 entry=$es2 count=$ec2")
                    $rawTbl = [System.BitConverter]::ToString($vbios, $tblStart, [Math]::Min(48, $vbios.Length - $tblStart))
                    Write-Output ("    Raw: " + $rawTbl)
                }
            }
        }
    }
    
    exit 1
}

# Resolve pointer: BIT pointers are relative to PCI ROM image start
$absPtr = $romBase + $falconDataPtr
Write-Output ("Falcon Data at abs=" + (Write-Hex $absPtr) + " (ROM+" + (Write-Hex $falconDataPtr) + ")")

if ($absPtr + 4 -lt $vbios.Length) {
    # First u32 in Falcon Data is pointer to PMU/ucode lookup table
    $ucTableRelPtr = Read-U32 $vbios $absPtr
    Write-Output ("  ucode_table_ptr = " + (Write-Hex $ucTableRelPtr))
    
    $ucTableAbs = $romBase + $ucTableRelPtr
    if ($ucTableRelPtr -gt $romLegacyLen) {
        # Pointer goes beyond legacy image, adjust
        $ucTableAbs = $romBase + $ucTableRelPtr
        Write-Output ("  NOTE: ptr > legacy ROM len, abs=" + (Write-Hex $ucTableAbs))
    }
    
    if ($ucTableAbs + 16 -lt $vbios.Length) {
        $rawTbl = [System.BitConverter]::ToString($vbios, $ucTableAbs, [Math]::Min(64, $vbios.Length - $ucTableAbs))
        Write-Output ("  ucode_table raw: " + $rawTbl)
        
        # Parse PMU lookup table header
        $pmuVer = $vbios[$ucTableAbs]
        $pmuHdrSz = $vbios[$ucTableAbs + 1]
        $pmuEntrySz = $vbios[$ucTableAbs + 2]
        $pmuEntryCount = $vbios[$ucTableAbs + 3]
        Write-Output ("  PMU table: ver=$pmuVer hdr=$pmuHdrSz entry_sz=$pmuEntrySz count=$pmuEntryCount")
        
        # Walk entries looking for app_id = 0x85
        for ($e = 0; $e -lt $pmuEntryCount; $e++) {
            $eOff = $ucTableAbs + $pmuHdrSz + $e * $pmuEntrySz
            if ($eOff + $pmuEntrySz -gt $vbios.Length) { break }
            $appId = $vbios[$eOff]
            $target = $vbios[$eOff + 1]
            $descPtr = Read-U32 $vbios ($eOff + 2)
            Write-Output ("    Entry[$e]: app_id=" + (Write-Hex $appId 2) + " target=" + (Write-Hex $target 2) + " desc_ptr=" + (Write-Hex $descPtr))
            
            if ($appId -eq 0x85) {
                Write-Output "    *** FWSEC_PROD FOUND ***"
                # Resolve descriptor
                $descAbs = $romBase + $descPtr
                Write-Output ("    Descriptor at abs=" + (Write-Hex $descAbs))
                
                if ($descAbs + 64 -lt $vbios.Length) {
                    $rawDesc = [System.BitConverter]::ToString($vbios, $descAbs, 64)
                    Write-Output ("    Desc raw: " + $rawDesc)
                    
                    # FalconUCodeDescV3 layout (approximate from nouveau):
                    # u32 stored_size     - total size of stored ucode
                    # u32 uncompressed_size
                    # u32 virtual_entry
                    # u32 interface_offset
                    # u32 imem_phys_base
                    # u32 imem_load_size
                    # u32 imem_virt_base  
                    # u32 imem_sec_base
                    # u32 imem_sec_size
                    # u32 dmem_offset
                    # u32 dmem_phys_base
                    # u32 dmem_load_size
                    
                    $storedSize = Read-U32 $vbios $descAbs
                    $uncompSize = Read-U32 $vbios ($descAbs + 4)
                    $virtEntry = Read-U32 $vbios ($descAbs + 8)
                    $ifaceOff = Read-U32 $vbios ($descAbs + 12)
                    $imemPhysBase = Read-U32 $vbios ($descAbs + 16)
                    $imemLoadSize = Read-U32 $vbios ($descAbs + 20)
                    $imemVirtBase = Read-U32 $vbios ($descAbs + 24)
                    $imemSecBase = Read-U32 $vbios ($descAbs + 28)
                    $imemSecSize = Read-U32 $vbios ($descAbs + 32)
                    $dmemOffset = Read-U32 $vbios ($descAbs + 36)
                    $dmemPhysBase = Read-U32 $vbios ($descAbs + 40)
                    $dmemLoadSize = Read-U32 $vbios ($descAbs + 44)
                    
                    Write-Output ("    stored_size     = " + (Write-Hex $storedSize))
                    Write-Output ("    uncompressed_sz = " + (Write-Hex $uncompSize))
                    Write-Output ("    virtual_entry   = " + (Write-Hex $virtEntry))
                    Write-Output ("    interface_off   = " + (Write-Hex $ifaceOff))
                    Write-Output ("    imem_phys_base  = " + (Write-Hex $imemPhysBase))
                    Write-Output ("    imem_load_size  = " + (Write-Hex $imemLoadSize))
                    Write-Output ("    imem_virt_base  = " + (Write-Hex $imemVirtBase))
                    Write-Output ("    imem_sec_base   = " + (Write-Hex $imemSecBase))
                    Write-Output ("    imem_sec_size   = " + (Write-Hex $imemSecSize))
                    Write-Output ("    dmem_offset     = " + (Write-Hex $dmemOffset))
                    Write-Output ("    dmem_phys_base  = " + (Write-Hex $dmemPhysBase))
                    Write-Output ("    dmem_load_size  = " + (Write-Hex $dmemLoadSize))
                    
                    # TODO: Extract and pack FWSEC into fwsec_ga106.bin
                    # For now, dump the raw FWSEC ucode from the descriptor
                    Write-Output ""
                    Write-Output "=== Extracting FWSEC ==="
                    
                    # The actual FWSEC code data starts at an offset relative to... 
                    # In nouveau, the code is found at the base of the PMU entry
                    # Actually, stored data often starts right after or at a known offset from descriptor
                }
            }
        }
    }
}

Write-Output ""
Write-Output "Done."
