param([Parameter(Mandatory)][string]$Output,[int]$Seconds=200)
$ErrorActionPreference='Stop'
$rows=[System.Collections.Generic.List[object]]::new()
try {
  $until=(Get-Date).AddSeconds($Seconds)
  while((Get-Date) -lt $until -and !(Test-Path ($Output+'.stop'))) {
    # Wildcards must be resolved again as each newly launched app creates instances.
    $pair=@(Get-Counter '\GPU Engine(*)\Utilization Percentage' -SampleInterval 1 -MaxSamples 2 -ErrorAction SilentlyContinue)
    if($pair.Count -lt 2){continue}
    $sample=$pair[-1];$stamp=$sample.Timestamp.ToUniversalTime().ToString('o')
    $active=@($sample.CounterSamples | Where-Object { $_.CookedValue -gt 0.0001 -and $_.Status -eq 0 } | ForEach-Object {
      if($_.InstanceName -match '^pid_(\d+)_(.+)$') { @{pid=[int]$Matches[1];engine=$Matches[2];percent=$_.CookedValue} }
    })
    $rows.Add(@{timestamp=$stamp;intervalStart=$pair[0].Timestamp.ToUniversalTime().ToString('o');active=$active})
  }
} catch { if($_.Exception.Message -ne 'Sampling complete'){ $rows.Add(@{error=$_.Exception.Message}) } }
$rows | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Output -Encoding utf8
