# Follow the Falcon Data token to find FWSEC
$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')

$romBase = 0x9200

# Token 'p' (0x70) Falcon Data: ptr=0x401 -> abs=0x9601, data = 1B-3E-07-00
$falconDataAbs = $romBase + 0x401  # = 0x9601
$ucTableRelPtr = [BitConverter]::ToUInt32($vbios, $falconDataAbs)
Write-Output ("Falcon ucode_table_ptr (relative to ROM) = 0x$([Convert]::ToString($ucTableRelPtr, 16))")

$ucTableAbs = $romBase + $ucTableRelPtr
Write-Output ("ucode_table abs = 0x$([Convert]::ToString($ucTableAbs, 16))")

if ($ucTableAbs + 64 -gt $vbios.Length) {
    Write-Output "ERROR: ucode table pointer out of bounds"
    exit 1
}

# Dump raw ucode table
$rawTbl = [System.BitConverter]::ToString($vbios, $ucTableAbs, 64)
Write-Output ("Raw ucode table: " + $rawTbl)
Write-Output ""

# Parse PMU lookup table header
# nouveau nvkm/subdev/bios/pmu.c nvbios_pmuTe():
# struct {
#   u8 version;
#   u8 hdr_size;
#   u8 entry_count;
#   u8 entry_size;
#   u8 sub_hdr_size;  (for v2)
#   u8 sub_entry_count;
#   u8 sub_entry_size;
# }
# Note: count and size might be swapped depending on version

$pmuVer = $vbios[$ucTableAbs]
$pmuHdrSz = $vbios[$ucTableAbs + 1]
$pmuField2 = $vbios[$ucTableAbs + 2]
$pmuField3 = $vbios[$ucTableAbs + 3]
$pmuField4 = $vbios[$ucTableAbs + 4]
$pmuField5 = $vbios[$ucTableAbs + 5]

Write-Output ("PMU table: byte0=$pmuVer byte1=$pmuHdrSz byte2=$pmuField2 byte3=$pmuField3 byte4=$pmuField4 byte5=$pmuField5")

# Try version 1: hdr(1) hdr_sz(1) count(1) entry_sz(1) 
# Try version 2: hdr(1) hdr_sz(1) entry_sz(1) count(1) sub_hdr(1) sub_count(1) sub_sz(1)
# Or: version(1) header_size(1) entry_count(1) entry_size(1)

# Let's try both interpretations
Write-Output ""
Write-Output "=== Interpretation 1: ver=$pmuVer hdr=$pmuHdrSz count=$pmuField2 entry_sz=$pmuField3 ==="
$entriesStart1 = $ucTableAbs + $pmuHdrSz
for ($e = 0; $e -lt $pmuField2; $e++) {
    $eOff = $entriesStart1 + $e * $pmuField3
    if ($eOff + $pmuField3 -gt $vbios.Length) { break }
    $rawE = [System.BitConverter]::ToString($vbios, $eOff, [Math]::Min($pmuField3, 16))
    $appId = $vbios[$eOff]
    $target = $vbios[$eOff + 1]
    Write-Output ("  [$e] app_id=0x$([Convert]::ToString($appId, 16)) target=0x$([Convert]::ToString($target, 16)) raw=$rawE")
    if ($appId -eq 0x85) { Write-Output "    *** FWSEC_PROD ***" }
}

Write-Output ""
Write-Output "=== Interpretation 2: ver=$pmuVer hdr=$pmuHdrSz entry_sz=$pmuField2 count=$pmuField3 ==="
$entriesStart2 = $ucTableAbs + $pmuHdrSz
for ($e = 0; $e -lt $pmuField3; $e++) {
    $eOff = $entriesStart2 + $e * $pmuField2
    if ($eOff + $pmuField2 -gt $vbios.Length) { break }
    $rawE = [System.BitConverter]::ToString($vbios, $eOff, [Math]::Min($pmuField2, 16))
    $appId = $vbios[$eOff]
    $target = $vbios[$eOff + 1]
    Write-Output ("  [$e] app_id=0x$([Convert]::ToString($appId, 16)) target=0x$([Convert]::ToString($target, 16)) raw=$rawE")
    if ($appId -eq 0x85) { Write-Output "    *** FWSEC_PROD ***" }
}

# Also try: maybe the table itself uses a different format
# nouveau uses: nvbios_pmuTe(bios, ...) with version/header/count/size fields
# Let's also try ver=2 format: ver(1) hdr(1) entry_sz(1) entry_count(1) sub_hdr_sz(1) sub_entry_count(1) sub_entry_sz(1)
Write-Output ""
Write-Output "=== Interpretation 3 (v2 format): ver=$pmuVer hdr=$pmuHdrSz entry_sz=$pmuField2 count=$pmuField3 sub_hdr=$pmuField4 sub_count=$pmuField5 ==="
for ($e = 0; $e -lt $pmuField3; $e++) {
    $eOff = $entriesStart2 + $e * ($pmuField2 + $pmuField5 * $vbios[$ucTableAbs + 6])
    if ($eOff + $pmuField2 -gt $vbios.Length) { break }
    $rawE = [System.BitConverter]::ToString($vbios, $eOff, [Math]::Min(16, $vbios.Length - $eOff))
    $appId = $vbios[$eOff]
    $target = $vbios[$eOff + 1]
    Write-Output ("  [$e] app_id=0x$([Convert]::ToString($appId, 16)) target=0x$([Convert]::ToString($target, 16)) raw=$rawE")
    if ($appId -eq 0x85) { Write-Output "    *** FWSEC_PROD ***" }
}

# Brute-force: just search near the ucode table for app_id=0x85
Write-Output ""
Write-Output "=== Brute-force: search for 0x85 near ucode table ==="
$searchStart = [Math]::Max(0, $ucTableAbs - 64)
$searchEnd = [Math]::Min($vbios.Length - 8, $ucTableAbs + 512)
for ($i = $searchStart; $i -lt $searchEnd; $i++) {
    if ($vbios[$i] -eq 0x85) {
        $ctx = [System.BitConverter]::ToString($vbios, [Math]::Max(0, $i-4), 16)
        $relOff = $i - $ucTableAbs
        Write-Output ("  0x85 at abs=0x$([Convert]::ToString($i, 16)) (table+$relOff): ctx=$ctx")
    }
}
