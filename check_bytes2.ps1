$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')
$text = [System.Text.Encoding]::UTF8.GetString($bytes)

# Find 'info!(' near the GSP section
$idx = $text.IndexOf('info!(\\"Attempting')
if ($idx -ge 0) {
    Write-Output "Found double-backslash-quote at offset $idx"
    # Show hex of the info!( area
    $startByte = [System.Text.Encoding]::UTF8.GetByteCount($text.Substring(0, $idx))
    $hexChunk = $bytes[$startByte..($startByte+59)]
    $hexStr = ($hexChunk | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
    Write-Output "Hex: $hexStr"
    $asciiStr = ($hexChunk | ForEach-Object { if ($_ -ge 32 -and $_ -le 126) { [char]$_ } else { '.' } }) -join ''
    Write-Output "ASCII: $asciiStr"
} else {
    Write-Output "Double-backslash-quote NOT found"
}

# Try just looking for the raw bytes of backslash-quote: 5C 22
$found = @()
for ($i = 0; $i -lt $bytes.Length - 1; $i++) {
    if ($bytes[$i] -eq 0x5C -and $bytes[$i+1] -eq 0x22) {
        $found += $i
    }
}
Write-Output "Found $($found.Count) occurrences of \`" (0x5C 0x22) in file"
if ($found.Count -gt 0) {
    foreach ($pos in $found) {
        $start = [Math]::Max(0, $pos - 10)
        $end = [Math]::Min($bytes.Length - 1, $pos + 10)
        $chunk = $bytes[$start..$end]
        $asciiStr = ($chunk | ForEach-Object { if ($_ -ge 32 -and $_ -le 126) { [char]$_ } else { '.' } }) -join ''
        Write-Output "  At byte $pos : $asciiStr"
    }
}
