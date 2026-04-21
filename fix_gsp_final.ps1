# Read raw bytes
$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')

# Build new byte array, replacing 5C 22 with just 22
# But only between byte offsets 12100 and 13500 (the GSP section)
$newBytes = New-Object System.Collections.Generic.List[byte]
$i = 0
$replaced = 0
while ($i -lt $bytes.Length) {
    if ($i -ge 12100 -and $i -le 13500 -and $i -lt ($bytes.Length - 1) -and $bytes[$i] -eq 0x5C -and $bytes[$i+1] -eq 0x22) {
        # Skip the backslash, keep the quote
        $newBytes.Add(0x22)
        $i += 2
        $replaced++
    } else {
        $newBytes.Add($bytes[$i])
        $i++
    }
}

Write-Output "Replaced $replaced backslash-quote pairs"
Write-Output "Original size: $($bytes.Length), New size: $($newBytes.Count)"

[System.IO.File]::WriteAllBytes('bootloader\src\main.rs', $newBytes.ToArray())
Write-Output "File written successfully!"
