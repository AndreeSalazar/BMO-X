$bytes = [System.IO.File]::ReadAllBytes("C:\Users\andre\Documents\FastOS\target_build\kernel\x86_64-unknown-none\release\fastos-kernel")
for ($i = 0; $i -lt 6; $i++) {
    $o = 64 + $i*56
    $t = [BitConverter]::ToInt32($bytes, $o)
    $v = [BitConverter]::ToInt64($bytes, $o+16)
    $p = [BitConverter]::ToInt64($bytes, $o+24)
    $ms = [BitConverter]::ToInt64($bytes, $o+40)
    $fs = [BitConverter]::ToInt64($bytes, $o+32)
    $fl = [BitConverter]::ToInt32($bytes, $o+48)
    $vhex = "0x{0:X}" -f $v
    $phex = "0x{0:X}" -f $p
    $mshex = "0x{0:X}" -f $ms
    $fshex = "0x{0:X}" -f $fs
    $vend = "0x{0:X}" -f ($v + $ms)
    Write-Host "PH${i}: type=$t vaddr=$vhex paddr=$phex filesz=$fshex memsz=$mshex end=$vend flags=$fl"
}
