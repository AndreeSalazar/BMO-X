$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')
$text = [System.Text.Encoding]::UTF8.GetString($bytes)

# Replace all occurrences of \" (backslash-quote, 0x5C 0x22) with just " (0x22)
# But only in the GSP section (lines 340-375 area)
# Let's find the GSP section boundaries
$startMarker = '4b. Load GSP firmware'
$endMarker = '// -- 5. Query GOP'

$startIdx = $text.IndexOf($startMarker)
$endIdx = $text.IndexOf($endMarker)

if ($startIdx -lt 0) {
    # Try alternate marker
    $endMarker = '5. Query GOP'
    $endIdx = $text.IndexOf($endMarker)
}

if ($startIdx -ge 0 -and $endIdx -ge 0) {
    Write-Output "Found GSP section from offset $startIdx to $endIdx"
    
    # Extract the section
    $before = $text.Substring(0, $startIdx)
    $gspSection = $text.Substring($startIdx, $endIdx - $startIdx)
    $after = $text.Substring($endIdx)
    
    # Count backslash-quotes in the section
    $count = ([regex]::Matches($gspSection, [regex]::Escape('\"'))).Count
    Write-Output "Found $count backslash-quote occurrences in GSP section"
    
    # Replace \" with " in the GSP section only
    $fixedSection = $gspSection.Replace('\"', '"')
    
    # Also replace the em-dash with double-dash for safety
    # $fixedSection = $fixedSection.Replace([char]0x2014, '--')
    
    # Reconstruct
    $newText = $before + $fixedSection + $after
    
    # Write back
    $newBytes = [System.Text.Encoding]::UTF8.GetBytes($newText)
    [System.IO.File]::WriteAllBytes('bootloader\src\main.rs', $newBytes)
    
    Write-Output "Fixed! Replaced $count backslash-quote occurrences."
} else {
    Write-Output "Could not find GSP section markers. startIdx=$startIdx endIdx=$endIdx"
}
