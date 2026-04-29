# _find_fwsec7.ps1 - Parse FWSEC v3 descriptor at SPI 0x4A410
# Base offset = 0x1DA00 (PMU table SPI - BIT 'p' pointer = 0x9181B - 0x73E1B)
# This base is the offset between BIOS address space and SPI flash
# FWSEC at SPI 0x1DA00 + 0x2CA10 = 0x4A410
# All 4 Falcon ucodes (types 0x45, 0x85, 0x49, 0x89) have hdr=0x04AC0301

$vbios = [System.IO.File]::ReadAllBytes('c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\vbios_rtx3060.rom')
$base = 0x1DA00

# Parse the FWSEC descriptor (type 0x85)
$fwsecOff = $base + 0x2CA10  # = 0x4A410
Write-Output "=== FWSEC v3 Descriptor at SPI 0x$([Convert]::ToString($fwsecOff, 16)) ==="

$hdr = [BitConverter]::ToUInt32($vbios, $fwsecOff)
$hdrSize = ($hdr -shr 16) -band 0xFFFF
$hdrVer = ($hdr -shr 8) -band 0xFF
$hdrValid = $hdr -band 1
Write-Output "Hdr = 0x$([Convert]::ToString($hdr, 16)): ver=$hdrVer size=0x$([Convert]::ToString($hdrSize, 16)) valid=$hdrValid"

# nvkm_falcon_ucode_desc_v3:
# +0: Hdr (u32) [31:16]=size [15:8]=version [0]=valid
# +4: StoredSize (u32)
# +8: PKCDataOffset (u32)
# +12: InterfaceOffset (u32)
# +16: IMEMPhysBase (u32)
# +20: IMEMLoadSize (u32)
# +24: IMEMVirtBase (u32)
# +28: DMEMPhysBase (u32)
# +32: DMEMLoadSize (u32)
# +36: EngineIdMask (u16)
# +38: UcodeId (u8)
# +39: SignatureCount (u8)
# +40: SignatureVersions (u16)
# +42: Reserved (u16)

$storedSize = [BitConverter]::ToUInt32($vbios, $fwsecOff + 4)
$pkcDataOff = [BitConverter]::ToUInt32($vbios, $fwsecOff + 8)
$ifaceOff = [BitConverter]::ToUInt32($vbios, $fwsecOff + 12)
$imemBase = [BitConverter]::ToUInt32($vbios, $fwsecOff + 16)
$imemSize = [BitConverter]::ToUInt32($vbios, $fwsecOff + 20)
$imemVirt = [BitConverter]::ToUInt32($vbios, $fwsecOff + 24)
$dmemBase = [BitConverter]::ToUInt32($vbios, $fwsecOff + 28)
$dmemSize = [BitConverter]::ToUInt32($vbios, $fwsecOff + 32)
$engineId = [BitConverter]::ToUInt16($vbios, $fwsecOff + 36)
$ucodeId = $vbios[$fwsecOff + 38]
$sigCount = $vbios[$fwsecOff + 39]
$sigVers = [BitConverter]::ToUInt16($vbios, $fwsecOff + 40)
$reserved = [BitConverter]::ToUInt16($vbios, $fwsecOff + 42)

Write-Output "StoredSize = 0x$([Convert]::ToString($storedSize, 16)) = $storedSize bytes"
Write-Output "PKCDataOffset = 0x$([Convert]::ToString($pkcDataOff, 16))"
Write-Output "InterfaceOffset = 0x$([Convert]::ToString($ifaceOff, 16))"
Write-Output "IMEM PhysBase = 0x$([Convert]::ToString($imemBase, 16))"
Write-Output "IMEM LoadSize = 0x$([Convert]::ToString($imemSize, 16)) = $imemSize bytes"
Write-Output "IMEM VirtBase = 0x$([Convert]::ToString($imemVirt, 16))"
Write-Output "DMEM PhysBase = 0x$([Convert]::ToString($dmemBase, 16))"
Write-Output "DMEM LoadSize = 0x$([Convert]::ToString($dmemSize, 16)) = $dmemSize bytes"
Write-Output "EngineIdMask = 0x$([Convert]::ToString($engineId, 16))"
Write-Output "UcodeId = $ucodeId"
Write-Output "SignatureCount = $sigCount"
Write-Output "SignatureVersions = 0x$([Convert]::ToString($sigVers, 16))"
Write-Output "Reserved = 0x$([Convert]::ToString($reserved, 16))"

$totalUcode = $imemSize + $dmemSize
Write-Output ""
Write-Output "Total ucode = IMEM + DMEM = 0x$([Convert]::ToString($totalUcode, 16)) = $totalUcode bytes"

# Signatures at descriptor + 0x2C (44 bytes from start)
$sigOff = $fwsecOff + 0x2C
$sigSize = 96 * 4  # RSA-3072 = 384 bytes per signature
Write-Output ""
Write-Output "Signatures start at SPI 0x$([Convert]::ToString($sigOff, 16))"
Write-Output "Each signature = $sigSize bytes, count = $sigCount"
$totalSigs = $sigCount * $sigSize
Write-Output "Total signatures = $totalSigs bytes"

# IMEM blob starts at descriptor + hdrSize
$imemOff = $fwsecOff + $hdrSize
Write-Output ""
Write-Output "IMEM blob at SPI 0x$([Convert]::ToString($imemOff, 16))"
if ($imemOff + 32 -lt $vbios.Length) {
    $d = [BitConverter]::ToString($vbios, $imemOff, [Math]::Min(32, $vbios.Length - $imemOff))
    Write-Output "  First 32 bytes: $d"
}

# DMEM blob follows IMEM
$dmemOff = $imemOff + $imemSize
Write-Output ""
Write-Output "DMEM blob at SPI 0x$([Convert]::ToString($dmemOff, 16))"
if ($dmemOff + 32 -lt $vbios.Length) {
    $d = [BitConverter]::ToString($vbios, $dmemOff, [Math]::Min(32, $vbios.Length - $dmemOff))
    Write-Output "  First 32 bytes: $d"
}

# Verify the ucode ends within the SPI flash
$ucodeEnd = $imemOff + $totalUcode
Write-Output ""
Write-Output "Ucode ends at SPI 0x$([Convert]::ToString($ucodeEnd, 16))"
Write-Output "VBIOS size: 0x$([Convert]::ToString($vbios.Length, 16))"
if ($ucodeEnd -le $vbios.Length) {
    Write-Output "Ucode fits within VBIOS - GOOD"
} else {
    Write-Output "ERROR: Ucode extends beyond VBIOS!"
}

# Summary
Write-Output ""
Write-Output "========================================="
Write-Output "=== FWSEC EXTRACTION SUMMARY ==="
Write-Output "========================================="
Write-Output "FWSEC descriptor at SPI offset: 0x$([Convert]::ToString($fwsecOff, 16))"
Write-Output "FWSEC IMEM at SPI offset: 0x$([Convert]::ToString($imemOff, 16)), size: 0x$([Convert]::ToString($imemSize, 16))"
Write-Output "FWSEC DMEM at SPI offset: 0x$([Convert]::ToString($dmemOff, 16)), size: 0x$([Convert]::ToString($dmemSize, 16))"
Write-Output "FWSEC IMEM PhysBase = 0x$([Convert]::ToString($imemBase, 16))"
Write-Output "FWSEC DMEM PhysBase = 0x$([Convert]::ToString($dmemBase, 16))"
Write-Output "FWSEC InterfaceOffset = 0x$([Convert]::ToString($ifaceOff, 16))"
Write-Output "FWSEC PKCDataOffset = 0x$([Convert]::ToString($pkcDataOff, 16))"
Write-Output "FWSEC EngineId = 0x$([Convert]::ToString($engineId, 16))"
Write-Output "FWSEC UcodeId = $ucodeId"
Write-Output "FWSEC SigCount = $sigCount"
Write-Output "FWSEC SigVersions = 0x$([Convert]::ToString($sigVers, 16))"
Write-Output ""
Write-Output "Base offset for all PMU data_ptrs = 0x$([Convert]::ToString($base, 16))"
Write-Output "Formula: SPI_offset = 0x$([Convert]::ToString($base, 16)) + data_ptr"
Write-Output ""

# Dump first 64 bytes of descriptor raw
$raw = [BitConverter]::ToString($vbios, $fwsecOff, [Math]::Min(64, $vbios.Length - $fwsecOff))
Write-Output "Descriptor raw 64 bytes:"
Write-Output "  $raw"

# Also parse the other entries for comparison
Write-Output ""
Write-Output "=== All PMU Falcon ucodes ==="
$allEntries = @(
    @{type=0x01; ptr=0x15454; name="PMU"},
    @{type=0x07; ptr=0x3b8bc; name="Type07"},
    @{type=0x08; ptr=0x55898; name="Type08"},
    @{type=0x45; ptr=0x1db64; name="SEC2_FRTS?"},
    @{type=0x85; ptr=0x2ca10; name="FWSEC_PROD"},
    @{type=0x49; ptr=0x4b558; name="Type49"},
    @{type=0x89; ptr=0x506f8; name="Type89"}
)

foreach ($e in $allEntries) {
    $off = $base + $e.ptr
    if ($off + 44 -lt $vbios.Length) {
        $h = [BitConverter]::ToUInt32($vbios, $off)
        $v = ($h -shr 8) -band 0xFF
        $s = ($h -shr 16) -band 0xFFFF
        $val = $h -band 1
        $im = [BitConverter]::ToUInt32($vbios, $off + 20)
        $dm = [BitConverter]::ToUInt32($vbios, $off + 32)
        $eid = [BitConverter]::ToUInt16($vbios, $off + 36)
        $uid = $vbios[$off + 38]
        $sc = $vbios[$off + 39]
        Write-Output "  type=0x$([Convert]::ToString($e.type, 16)) '$($e.name)' at SPI 0x$([Convert]::ToString($off, 16)): ver=$v sz=0x$([Convert]::ToString($s, 16)) valid=$val IMEM=0x$([Convert]::ToString($im, 16)) DMEM=0x$([Convert]::ToString($dm, 16)) EngId=0x$([Convert]::ToString($eid, 16)) UcId=$uid SigCnt=$sc"
    } else {
        Write-Output "  type=0x$([Convert]::ToString($e.type, 16)) '$($e.name)' at SPI 0x$([Convert]::ToString($off, 16)): BEYOND VBIOS"
    }
}
