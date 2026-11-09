param (
    [int]$DurationSeconds = 15,
    [string]$OutputFile = "",
    [string]$Config = "release"
)

$exePath = if ($Config -eq "release" -and (Test-Path "target\release\src-tauri.exe")) {
    "target\release\src-tauri.exe"
} else {
    "target\debug\src-tauri.exe"
}

Write-Host "Starting Litube ($exePath) for memory measurement..."
$proc = Start-Process -FilePath $exePath -PassThru

$samples = @()
$startTime = Get-Date

try {
    while (((Get-Date) - $startTime).TotalSeconds -lt $DurationSeconds) {
        # Find the main process and all related child processes (msedgewebview2, etc.)
        $allProcs = Get-Process | Where-Object { 
            $_.Id -eq $proc.Id -or ($_.ProcessName -like "*webview2*" -and $_.StartTime -ge $startTime.AddSeconds(-2)) 
        }

        $totalPrivateBytes = ($allProcs | Measure-Object -Property PrivateMemorySize64 -Sum).Sum
        $totalWorkingSet = ($allProcs | Measure-Object -Property WorkingSet64 -Sum).Sum
        $procCount = $allProcs.Count

        $privMB = [math]::Round($totalPrivateBytes / 1MB, 2)
        $wsMB = [math]::Round($totalWorkingSet / 1MB, 2)

        $samples += [PSCustomObject]@{
            ElapsedSec = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
            ProcessCount = $procCount
            PrivateBytesMB = $privMB
            WorkingSetMB = $wsMB
        }

        Start-Sleep -Seconds 1
    }
}
finally {
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    }
}

$avgPriv = [math]::Round(($samples | Measure-Object -Property PrivateBytesMB -Average).Average, 2)
$maxPriv = ($samples | Measure-Object -Property PrivateBytesMB -Maximum).Maximum
$avgWS = [math]::Round(($samples | Measure-Object -Property WorkingSetMB -Average).Average, 2)
$maxWS = ($samples | Measure-Object -Property WorkingSetMB -Maximum).Maximum

$report = @"
# Litube Process Tree Memory Measurement (Phase 0 Spike)

- **Date:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
- **Duration:** $DurationSeconds seconds
- **Processes in Tree:** $($samples[-1].ProcessCount) (Rust host + WebView2 runtimes)
- **Private Bytes (Average):** $avgPriv MB
- **Private Bytes (Peak):** $maxPriv MB
- **Working Set (Average):** $avgWS MB
- **Working Set (Peak):** $maxWS MB

## Samples:
$($samples | Format-Table -AutoSize | Out-String)
"@

Write-Host $report

if ($OutputFile) {
    Set-Content -Path $OutputFile -Value $report
}

