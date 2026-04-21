$bytes = [System.IO.File]::ReadAllBytes('bootloader\src\main.rs')
$text = [System.Text.Encoding]::UTF8.GetString($bytes)

# Find the end marker - try various forms
$markers = @(
    '// -- 5. Query GOP',
    '// ── 5. Query GOP',
    '5. Query GOP'
)

foreach ($m in $markers) {
    $idx = $text.IndexOf($m)
    Write-Output "Marker '$m' -> index $idx"
}

# Let's just look at what's around the end of the GSP section
$gspStart = $text.IndexOf('4b. Load GSP firmware')
if ($gspStart -ge 0) {
    # Find the next comment section marker after GSP
    # Look for "5." or "Query GOP" after the GSP section
    $searchFrom = $gspStart + 100
    $nextSection = $text.IndexOf('Query GOP', $searchFrom)
    Write-Output "Next 'Query GOP' after GSP: $nextSection"
    
    if ($nextSection -ge 0) {
        # Go back to find the start of that comment line
        $lineStart = $text.LastIndexOf("`n", $nextSection)
        Write-Output "Line start before 'Query GOP': $lineStart"
        
        # Extract GSP section
        $gspSection = $text.Substring($gspStart, $lineStart - $gspStart + 1)
        
        # Show first and last 100 chars
        Write-Output "GSP section length: $($gspSection.Length)"
        Write-Output "First 100: $($gspSection.Substring(0, [Math]::Min(100, $gspSection.Length)))"
        Write-Output "Last 100: $($gspSection.Substring([Math]::Max(0, $gspSection.Length - 100)))"
        
        # Count and replace backslash-quotes
        $count = 0
        $fixed = $gspSection
        while ($fixed.Contains('\\"')) {
            $fixed = $fixed.Replace('\\"', '"')
            $count++
        }
        # Actually count properly
        $count = ([regex]::Matches($gspSection, '\\\\\"')).Count
        Write-Output "Backslash-quote occurrences: $count"
        
        # Do the replacement
        $fixedSection = $gspSection.Replace('\\"', '"')
        
        $before = $text.Substring(0, $gspStart)
        $after = $text.Substring($gspStart + $gspSection.Length)
        $newText = $before + $fixedSection + $after
        
        # Write back as UTF-8 without BOM
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText('bootloader\src\main.rs', $newText, $utf8NoBom)
        
        Write-Output "File written successfully!"
    }
}
