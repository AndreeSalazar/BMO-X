# Quick test: check if _find_fwsec.ps1 has encoding issues
$lines = Get-Content 'c:\Users\andre\OneDrive\Documentos\FastOS\_find_fwsec.ps1'
Write-Output "Total lines: $($lines.Count)"
# Check line 415 area
for ($i = 412; $i -lt [Math]::Min(416, $lines.Count); $i++) {
    $hex = [BitConverter]::ToString([System.Text.Encoding]::UTF8.GetBytes($lines[$i]))
    Write-Output "Line $($i+1): $hex"
}
