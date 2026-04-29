# _find_fwsec2.ps1 - Correct BIT parsing for NVGI SPI dump
# Key insight from previous analysis: BIT header has 0xB8FF prefix at 0x93B0
# "BIT\0" at 0x93B2, hdr_size=12, token_size=6, token_count=18

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$romBase = 0x9200

# Dump BIT area bytes for analysis
$bitPrefixOff = 0x93B0
Write-Output "=== BIT area raw dump ==="
$raw = [BitConverter]::ToString($vbios, $bitPrefixOff, 64)
Write-Output "0x93B0: $raw"
Write-Output ""

# Parse BIT header manually
# 0x93B0: FF B8 42 49 54 00 ...
# The BIT header format (from nouveau bit.c):
#   nouveau stores bit_offset pointing to the FF-B8-B-I-T sequence 
#   Then at bit_offset, the 5-byte magic: FF B8 42 49 54
#   Actually, nouveau bit.c uses bios->bit_offset which points 2 bytes BEFORE "BIT"
#   i.e. to the 0xFF 0xB8 prefix itself
#
# Re-reading nouveau bit.c more carefully:
#   bios->bit_offset = nvbios_findstr(bios->data, bios->size, "\xff\xb8""BIT", 5)
#   So bit_offset points to the 0xFF byte
#   Then: entries = nvbios_rd08(bios, bios->bit_offset + 10)
#         entry = bios->bit_offset + 12
#         stride = nvbios_rd08(bios, bios->bit_offset + 9)

$bitOff = $bitPrefixOff  # Points to 0xFF, the start of the 5-byte sequence
Write-Output "bit_offset = 0x$([Convert]::ToString($bitOff, 16))"

# Verify magic
$m0 = $vbios[$bitOff]
$m1 = $vbios[$bitOff+1]
$m2 = $vbios[$bitOff+2]
$m3 = $vbios[$bitOff+3]
$m4 = $vbios[$bitOff+4]
Write-Output "Magic bytes: $([Convert]::ToString($m0,16))-$([Convert]::ToString($m1,16))-$([Convert]::ToString($m2,16))-$([Convert]::ToString($m3,16))-$([Convert]::ToString($m4,16))"

# BIT header layout after the 5-byte magic:
# +5: BCD version low
# +6: BCD version high  
# +7: header size (bytes from start of BIT to first entry)
# +8: entry size (stride)
# +9: entry count
# Wait - that's only valid if the header format matches
# Let me dump +5 through +15

Write-Output ""
Write-Output "BIT header fields:"
for ($j = 0; $j -lt 20; $j++) {
    $b = $vbios[$bitOff + $j]
    Write-Output "  +$j = 0x$([Convert]::ToString($b, 16)) = $b"
}

# Nouveau code:
#   u8 entries = nvbios_rd08(bios, bios->bit_offset + 10);
#   u32 entry  = bios->bit_offset + 12;
#   stride     = nvbios_rd08(bios, bios->bit_offset + 9);
# But bit_offset can also be stored as pointing to 'B' of "BIT"
# Let me check - in nvbios_findstr, it searches for the full 5 bytes
# and returns the offset of the FIRST byte (0xFF)

$stride = $vbios[$bitOff + 9]
$entryCount = $vbios[$bitOff + 10]
$entriesStart = $bitOff + 12

Write-Output ""
Write-Output "According to nouveau offsets from FF-prefix:"
Write-Output "  stride = byte at +9 = $stride"
Write-Output "  count = byte at +10 = $entryCount"
Write-Output "  entries start at +12 = 0x$([Convert]::ToString($entriesStart, 16))"

# Those values look wrong (stride=69='E', count=50). 
# Let me try with bit_offset pointing to 'B' of "BIT" instead
$bitOff2 = $bitPrefixOff + 2  # Points to 'B'
$stride2 = $vbios[$bitOff2 + 9]
$count2 = $vbios[$bitOff2 + 10]
$start2 = $bitOff2 + 12

Write-Output ""
Write-Output "If bit_offset points to 'B' of BIT:"
Write-Output "  stride = byte at +9 = $stride2"  
Write-Output "  count = byte at +10 = $count2"
Write-Output "  entries start at +12 = 0x$([Convert]::ToString($start2, 16))"

# Hmm, let me try the actual BIT specification
# BIT header (from envytools):
#   +0: 0xB8 0xFF (little-endian marker)
#   +2: 'B' 'I' 'T' 0x00
#   +6: BCD version u16 (e.g. 0x0100 = v1.0)
#   +8: header_size u8 (total header bytes including this)
#   +9: entry_size u8 (bytes per entry)
#   +10: entry_count u8
#   Then entries follow at offset header_size from start of structure

# Wait, the signature from the user analysis says:
# 0xB8FF prefix at 0x93B0 means bytes are: B8 FF at 0x93B0
# Actually, looking at it: the user says "0xB8FF prefix" which in LE is FF B8
# Let me check the raw bytes

Write-Output ""
Write-Output "Raw at 0x93B0: $([BitConverter]::ToString($vbios, 0x93B0, 4))"
Write-Output "Raw at 0x93B2: $([BitConverter]::ToString($vbios, 0x93B2, 4))"

# OK, from the user's analysis:
# The BIT table structure per the envytools docs:
# BIT starts with: B8-FF followed by 'B','I','T',00
# Or: FF-B8 'B','I','T',00  (let me check)

# Actually in nouveau, the search string is "\xff\xb8" "BIT" = bytes FF B8 42 49 54
# So at 0x93B0 we should see: FF B8 42 49 54 00

# Now the BIT header: 
# From envytools BIT specification:
#   Offset 0: u8 = 0xB8 (or 0xFF, depends on exact format)
#   The BIT header after the signature:
#   +6 from 0x93B0: BCD version
#   +8: header_size 
#   +9: entry_size
#   +10: entry_count
# But that assumes the signature is at offset 0 of the table

# Let me just enumerate all reasonable interpretations
Write-Output ""  
Write-Output "=== Testing all offset interpretations ==="

for ($base = $bitPrefixOff; $base -le $bitPrefixOff + 6; $base++) {
    Write-Output ""
    Write-Output "--- Base = 0x$([Convert]::ToString($base, 16)) ---"
    
    # Try reading stride at various offsets
    for ($sOff = 7; $sOff -le 11; $sOff++) {
        $s = $vbios[$base + $sOff]
        $c = $vbios[$base + $sOff + 1]
        if ($s -eq 6 -and $c -ge 1 -and $c -le 30) {
            Write-Output "  MATCH: stride at +$sOff = $s, count at +$($sOff+1) = $c"
            # Try to read first entry
            $eStart = $base + $sOff + 2
            $id = $vbios[$eStart]
            $ver = $vbios[$eStart + 1]
            $len = [BitConverter]::ToUInt16($vbios, $eStart + 2)
            $ptr = [BitConverter]::ToUInt16($vbios, $eStart + 4)
            Write-Output "    First entry: id=0x$([Convert]::ToString($id, 16)) '$([char]$id)' ver=$ver len=$len ptr=0x$([Convert]::ToString($ptr, 16))"
            
            # Check if this makes sense - known BIT IDs: 0x32='2', 0x42='B', 0x43='C', 0x44='D', 0x49='I', 0x4D='M', 0x4E='N', 0x50='P', 0x53='S', 0x54='T', 0x55='U', 0x56='V', 0x70='p', etc
        }
    }
    
    # Also try header_size from byte at +7 or +8
    for ($hOff = 7; $hOff -le 9; $hOff++) {
        $hdrSize = $vbios[$base + $hOff]
        if ($hdrSize -ge 8 -and $hdrSize -le 16) {
            # entries at base + hdrSize
            $eStart = $base + $hdrSize
            $id = $vbios[$eStart]
            Write-Output "  hdr_size at +$hOff = $hdrSize, first entry at 0x$([Convert]::ToString($eStart, 16)), id=0x$([Convert]::ToString($id, 16)) '$([char]$id)'"
        }
    }
}

# Let me also do a direct approach: read the BIT header bytes and figure out the format
Write-Output ""
Write-Output "=== Raw BIT header interpretation ==="
# Bytes at 0x93B0: 
$raw20 = [BitConverter]::ToString($vbios, 0x93B0, 24)
Write-Output "0x93B0: $raw20"

# Expected format per nouveau/envytools:
# FF B8 42 49 54 00 <ver_lo> <ver_hi> <hdr_size> <entry_size> <entry_count> <pad?>
# Then entries at 0x93B0 + hdr_size (if hdr_size counts from 0x93B0)
# OR entries at 0x93B0 + 12 per nouveau code

# Per the user's analysis, the correct parsing gives tokens starting at bitOff+10
# where bitOff=0x93B2. So entries at 0x93B2 + 10 = 0x93BC

$tryStart = 0x93BC
Write-Output ""
Write-Output "=== Trying entries at 0x93BC per user analysis ==="
# User says token_size=6, token_count=18
$stride = 6
$count = 18

for ($e = 0; $e -lt $count; $e++) {
    $eOff = $tryStart + $e * $stride
    $id = $vbios[$eOff]
    $ver = $vbios[$eOff + 1]
    $len = [BitConverter]::ToUInt16($vbios, $eOff + 2)
    $ptr = [BitConverter]::ToUInt16($vbios, $eOff + 4)
    
    $ch = if ($id -ge 0x20 -and $id -le 0x7E) { [char]$id } else { '?' }
    $absPtr = $romBase + $ptr
    Write-Output ("  Entry[$e]: id=0x{0:X2} '{1}' ver={2} len={3} ptr=0x{4:X4} abs=0x{5:X}" -f $id, $ch, $ver, $len, $ptr, $absPtr)
    
    if ($id -eq 0x70) {
        Write-Output "    *** Token 'p' found ***"
        Write-Output "    Reading payload at abs=0x$([Convert]::ToString($absPtr, 16)):"
        $pData = [BitConverter]::ToString($vbios, $absPtr, [Math]::Min(16, $vbios.Length - $absPtr))
        Write-Output "    Raw: $pData"
        
        if ($ver -eq 2 -and $len -ge 4) {
            $pmuPtr = [BitConverter]::ToUInt32($vbios, $absPtr)
            Write-Output "    PMU table pointer = 0x$([Convert]::ToString($pmuPtr, 16))"
            
            # This pointer should be VBIOS-relative from start of image
            # In nouveau, nvbios_rd32 reads from bios->data which is the full shadowed BIOS
            # For SPI dumps, that includes the NVGI header
            # BUT for PCI ROM images, nouveau only shadows the PCI ROM part
            # The question is: what does nouveau see as bios->data?
            
            # Try 1: pointer relative to ROM start
            $pmuAbs1 = $romBase + $pmuPtr
            Write-Output "    Try 1 - ROM-relative: 0x$([Convert]::ToString($pmuAbs1, 16))"
            if ($pmuAbs1 -lt $vbios.Length - 4) {
                $d = [BitConverter]::ToString($vbios, $pmuAbs1, [Math]::Min(16, $vbios.Length - $pmuAbs1))
                Write-Output "      Data: $d"
                Write-Output "      As table hdr: ver=$($vbios[$pmuAbs1]) hdr=$($vbios[$pmuAbs1+1]) stride=$($vbios[$pmuAbs1+2]) count=$($vbios[$pmuAbs1+3])"
            }
            
            # Try 2: pointer is SPI-absolute
            $pmuAbs2 = $pmuPtr
            Write-Output "    Try 2 - SPI-absolute: 0x$([Convert]::ToString($pmuAbs2, 16))"
            if ($pmuAbs2 -lt $vbios.Length - 4) {
                $d = [BitConverter]::ToString($vbios, $pmuAbs2, [Math]::Min(16, $vbios.Length - $pmuAbs2))
                Write-Output "      Data: $d"
                Write-Output "      As table hdr: ver=$($vbios[$pmuAbs2]) hdr=$($vbios[$pmuAbs2+1]) stride=$($vbios[$pmuAbs2+2]) count=$($vbios[$pmuAbs2+3])"
            } else {
                Write-Output "      Beyond VBIOS size"
            }
        } else {
            Write-Output "    Token version=$ver, length=$len - does not match nouveau expected ver=2, len>=4"
        }
    }
}

# Also - the user says the first data at 'p' payload is 1B-3E-07-00 = 0x00073E1B
# Let me trace what nouveau would do with this
Write-Output ""
Write-Output "=== Tracing the Falcon data chain ==="
# The 'p' entry ptr field gives VBIOS offset of the payload
# The payload is a table: first u32 = pointer to PMU ucode table
# But what is the base? In nouveau, ALL pointers from BIT tables
# are relative to the start of the BIOS image as shadowed by nouveau

# Nouveau shadows the BIOS via nvbios_shadow. For PCI ROM BARs, it reads
# starting from the 55 AA header. So for our SPI dump, the BIT offsets
# are relative to the PCI ROM image at 0x9200.

# The 'p' entry has ptr=0x0401. This gives bios offset 0x0401.
# The data at bios+0x0401 = vbios[0x9200+0x0401] = vbios[0x9601]
# That data is: 1B-3E-07-00 = u32 0x00073E1B

# This u32 is ALSO a bios-relative offset, pointing to:
# bios[0x73E1B] = vbios[0x9200 + 0x73E1B] = vbios[0x7D01B]
# But 0x7D01B is only 512027 bytes into the 2MB file - that should exist!

$chain1 = $romBase + 0x0401  # = 0x9601
Write-Output "Chain step 1: BIT 'p' ptr=0x0401 -> abs 0x$([Convert]::ToString($chain1, 16))"
$d1 = [BitConverter]::ToString($vbios, $chain1, 16)
Write-Output "  Data: $d1"
$pmuTblPtr = [BitConverter]::ToUInt32($vbios, $chain1)
Write-Output "  u32 = 0x$([Convert]::ToString($pmuTblPtr, 16))"

$chain2 = $romBase + $pmuTblPtr  # = 0x9200 + 0x73E1B = 0x7D01B
Write-Output ""
Write-Output "Chain step 2: PMU table at bios+0x$([Convert]::ToString($pmuTblPtr, 16)) = abs 0x$([Convert]::ToString($chain2, 16))"
if ($chain2 -lt $vbios.Length - 32) {
    $d2 = [BitConverter]::ToString($vbios, $chain2, 32)
    Write-Output "  Data: $d2"
    Write-Output "  As PMU table: ver=$($vbios[$chain2]) hdr=$($vbios[$chain2+1]) stride=$($vbios[$chain2+2]) count=$($vbios[$chain2+3])"
    
    $v = $vbios[$chain2]
    $h = $vbios[$chain2+1]
    $es = $vbios[$chain2+2]
    $ec = $vbios[$chain2+3]
    
    if ($v -ge 1 -and $v -le 3 -and $h -ge 4 -and $h -le 16 -and $es -ge 4 -and $es -le 32 -and $ec -ge 1 -and $ec -le 32) {
        Write-Output "  VALID PMU table header!"
        for ($i = 0; $i -lt $ec; $i++) {
            $eOff = $chain2 + $h + $i * $es
            $type = $vbios[$eOff]
            $dataPtr = [BitConverter]::ToUInt32($vbios, $eOff + 2)
            $raw = [BitConverter]::ToString($vbios, $eOff, [Math]::Min($es, $vbios.Length - $eOff))
            Write-Output "    PMU[$i]: type=0x$([Convert]::ToString($type, 16)) data=0x$([Convert]::ToString($dataPtr, 16)) raw=$raw"
            
            if ($type -eq 0x85) {
                Write-Output "      *** FWSEC type 0x85 FOUND ***"
            }
        }
    } else {
        Write-Output "  Not a valid PMU table header"
        
        # Maybe the pointer 0x73E1B is beyond the legacy ROM image
        # (legacy ROM is 65024 bytes = 0xFE00, and 0x73E1B >> 0xFE00)
        # This means the pointer might reference data in the second copy or
        # elsewhere in the SPI dump
        
        # Let me also try: the pointer might be from SPI base, not ROM base
        Write-Output ""
        Write-Output "  Trying pointer as SPI-absolute:"
        if ($pmuTblPtr -lt $vbios.Length - 32) {
            $d3 = [BitConverter]::ToString($vbios, $pmuTblPtr, 32)
            Write-Output "  Data at 0x$([Convert]::ToString($pmuTblPtr, 16)): $d3"
            Write-Output "  As PMU: ver=$($vbios[$pmuTblPtr]) hdr=$($vbios[$pmuTblPtr+1]) stride=$($vbios[$pmuTblPtr+2]) count=$($vbios[$pmuTblPtr+3])"
        }
        
        # The legacy ROM is 65024 bytes, but the pointer is 0x73E1B which is ~474KB
        # This is way beyond the legacy ROM. In the SPI dump, the FULL BIOS area 
        # might extend beyond just the PCI ROM images.
        # 
        # Key insight: nouveau reads bios->data which is the FULL SPI image for PROM shadow
        # The BIT offsets AND the pointers within BIT entries are all relative to
        # the start of bios->data (which is the PCI ROM start for PROM shadow,
        # but for SPI dumps it depends)
        #
        # Actually for nvbios_prom (PROM shadow), nouveau reads starting from offset 0
        # of the PROM, which maps to the beginning of SPI flash. The NVGI header IS
        # included. So bios->data[0] corresponds to vbios[0] in our dump.
        
        Write-Output ""
        Write-Output "  CRITICAL INSIGHT: nouveau PROM shadow reads from SPI offset 0"
        Write-Output "  So ALL offsets are relative to SPI start, not ROM start!"
        Write-Output ""
        
        # Re-interpret: BIT at 0x93B2 is at SPI offset 0x93B2
        # 'p' entry ptr = 0x0401 means SPI offset 0x0401
        Write-Output "  BIT 'p' ptr=0x0401 as SPI offset:"
        $spiP = 0x0401
        $d4 = [BitConverter]::ToString($vbios, $spiP, 16)
        Write-Output "  Data at SPI 0x0401: $d4"
        $pmuPtr2 = [BitConverter]::ToUInt32($vbios, $spiP)
        Write-Output "  u32 = 0x$([Convert]::ToString($pmuPtr2, 16))"
        
        if ($pmuPtr2 -lt $vbios.Length - 32) {
            $d5 = [BitConverter]::ToString($vbios, $pmuPtr2, 32)
            Write-Output "  PMU table at SPI 0x$([Convert]::ToString($pmuPtr2, 16)): $d5"
            Write-Output "  As header: ver=$($vbios[$pmuPtr2]) hdr=$($vbios[$pmuPtr2+1]) stride=$($vbios[$pmuPtr2+2]) count=$($vbios[$pmuPtr2+3])"
        }
    }
} else {
    Write-Output "  Beyond VBIOS!"
}

# Finally, let me check: where does the actual SPI flash VBIOS "start" from nouveau's perspective?
# On PROM access, nouveau reads 8-bit values from NV_PROM:
#   nvbios_prom_read(priv, addr) = nv_rd08(priv, 0x300000 + addr)
# This reads the GPU's SPI flash starting from offset 0. The NVGI header IS part of it.
# So bios->data[0] = first byte of SPI flash = NVGI magic "NVGI"
# All BIT pointers are relative to bios->data[0] = SPI offset 0.

Write-Output ""
Write-Output "=== Correct interpretation: all offsets relative to SPI base ==="
# BIT at SPI 0x93B0 (FF B8 prefix)
# BIT entry 'p' at SPI 0x93BC + N*6
# Entry 'p' -> ptr is ROM-image-relative, but bit_entry.offset IS a BIOS offset from 0

# Actually wait - looking at bit.c again:
# bit->offset = nvbios_rd16(bios, entry + 4)
# This is just the raw u16 from the BIT entry.
# Then nvbios_pmuTe does: data = nvbios_rd32(bios, bit_p.offset + 0x00)
# This reads from bios->data[bit_p.offset]
# So bit_p.offset = 0x0401 means bios->data[0x0401]

# For SPI-dump based PROM shadow: bios->data[0x0401] = vbios[0x0401]
Write-Output "Reading at SPI offset 0x0401:"
$d = [BitConverter]::ToString($vbios, 0x0401, 16)
Write-Output "  $d"
$p1 = [BitConverter]::ToUInt32($vbios, 0x0401)
Write-Output "  u32 = 0x$([Convert]::ToString($p1, 16))"

Write-Output ""
Write-Output "Following to SPI 0x$([Convert]::ToString($p1, 16)):"
if ($p1 -lt $vbios.Length - 32) {
    $d = [BitConverter]::ToString($vbios, $p1, 32)
    Write-Output "  $d"
    $v = $vbios[$p1]; $h = $vbios[$p1+1]; $s = $vbios[$p1+2]; $c = $vbios[$p1+3]
    Write-Output "  PMU: ver=$v hdr=$h stride=$s count=$c"
    
    if ($v -ge 1 -and $v -le 3 -and $h -ge 4 -and $h -le 16 -and $s -ge 4 -and $s -le 32 -and $c -ge 1 -and $c -le 32) {
        Write-Output "  === VALID! Parsing entries ==="
        for ($i = 0; $i -lt $c; $i++) {
            $eOff = $p1 + $h + $i * $s
            $type = $vbios[$eOff]
            $dp = [BitConverter]::ToUInt32($vbios, $eOff + 2)
            $raw = [BitConverter]::ToString($vbios, $eOff, [Math]::Min($s, $vbios.Length - $eOff))
            Write-Output "    [$i]: type=0x$([Convert]::ToString($type, 16)) data_ptr=0x$([Convert]::ToString($dp, 16)) raw=$raw"
            
            if ($type -eq 0x85) {
                Write-Output "      *** FWSEC FOUND! ***"
                # Parse the ucode descriptor at this offset
                if ($dp -lt $vbios.Length - 44) {
                    $hdrW = [BitConverter]::ToUInt32($vbios, $dp)
                    $hSize = ($hdrW -shr 16) -band 0xFFFF
                    $hVer = ($hdrW -shr 8) -band 0xFF
                    $hValid = $hdrW -band 1
                    Write-Output "      Descriptor Hdr: ver=$hVer size=$hSize valid=$hValid"
                    
                    $dRaw = [BitConverter]::ToString($vbios, $dp, [Math]::Min(48, $vbios.Length - $dp))
                    Write-Output "      Raw: $dRaw"
                }
            }
        }
    }
}
