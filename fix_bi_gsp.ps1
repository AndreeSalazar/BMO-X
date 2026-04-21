# Read the file as raw bytes to avoid encoding issues
$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')
$text = [System.Text.Encoding]::UTF8.GetString($bytes)

# Find the exact text to replace
$old = "// GSP not loaded by bootloader`r`n        bi.gsp_addr = 0;`r`n        bi.gsp_size = 0;"
$new = "// GSP firmware (loaded in step 4b, or 0 if not found)`r`n        bi.gsp_addr = gsp_addr;`r`n        bi.gsp_size = gsp_size;"

$idx = $text.IndexOf($old)
Write-Output "Index of old text: $idx"

if ($idx -ge 0) {
    $text = $text.Remove($idx, $old.Length).Insert($idx, $new)
    $newBytes = [System.Text.Encoding]::UTF8.GetBytes($text)
    [System.IO.File]::WriteAllBytes('bootloader\src\main.rs', $newBytes)
    Write-Output "Replacement done!"
} else {
    # Try without the leading spaces
    Write-Output "Trying alternate search..."
    $old2 = "GSP not loaded by bootloader"
    $idx2 = $text.IndexOf($old2)
    Write-Output "Index of 'GSP not loaded by bootloader': $idx2"
    
    if ($idx2 -ge 0) {
        # Show context
        $context = $text.Substring([Math]::Max(0, $idx2 - 20), 200)
        Write-Output "Context: [$context]"
        
        # Show hex of the area
        $byteOffset = [System.Text.Encoding]::UTF8.GetByteCount($text.Substring(0, $idx2))
        $hexChunk = $bytes[($byteOffset-10)..($byteOffset+100)]
        $hex = ($hexChunk | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
        Write-Output "Hex: $hex"
    }
}
