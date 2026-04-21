$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')
$text = [System.Text.Encoding]::UTF8.GetString($bytes)

# Find the GSP section
$idx = $text.IndexOf('info!(\"Attempting')
if ($idx -ge 0) {
    Write-Output "Found normal quotes at offset $idx"
} else {
    Write-Output "Normal quotes NOT found"
}

# Check for backslash-escaped quotes
$idx2 = $text.IndexOf('info!(\"')
if ($idx2 -ge 0) {
    Write-Output "Found backslash-quote at offset $idx2"
} else {
    Write-Output "Backslash-quote NOT found"
}

# Show raw bytes around line 340 area
# Find "4b. Load GSP"
$idx3 = $text.IndexOf('4b. Load GSP')
if ($idx3 -ge 0) {
    Write-Output "Found '4b. Load GSP' at offset $idx3"
    # Show next 200 chars as hex + ascii
    $chunk = $text.Substring($idx3, [Math]::Min(400, $text.Length - $idx3))
    Write-Output "--- TEXT ---"
    Write-Output $chunk
    Write-Output "--- END ---"
    
    # Show hex of first 100 bytes after the marker
    $startByte = [System.Text.Encoding]::UTF8.GetByteCount($text.Substring(0, $idx3))
    Write-Output "Hex bytes starting at byte offset $startByte :"
    $hexChunk = $bytes[$startByte..($startByte+199)]
    $hexStr = ($hexChunk | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
    Write-Output $hexStr
} else {
    Write-Output "'4b. Load GSP' NOT found"
}
