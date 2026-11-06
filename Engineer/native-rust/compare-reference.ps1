# Correctness comparison against existing C++ images; performs no performance work.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$pairs = @(@('cpp-panel.png','rust-panel.png'), @('cpp-panel-minimum.png','rust-panel-minimum.png'), @('cpp-panel-portrait.png','rust-panel-portrait.png'), @('cpp-panel-coolant.png','rust-panel-coolant.png'))
$results = foreach ($pair in $pairs) {
    $reference = [Drawing.Bitmap]::new((Join-Path $PSScriptRoot "../native-cpp-optimized/artifacts/$($pair[0])"))
    $rust = [Drawing.Bitmap]::new((Join-Path $PSScriptRoot "artifacts/$($pair[1])"))
    try {
        if ($reference.Width -ne $rust.Width -or $reference.Height -ne $rust.Height) { throw 'Image dimensions differ' }
        $bounds = [Drawing.Rectangle]::new(0,0,$reference.Width,$reference.Height)
        $a = $reference.LockBits($bounds,[Drawing.Imaging.ImageLockMode]::ReadOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $b = $rust.LockBits($bounds,[Drawing.Imaging.ImageLockMode]::ReadOnly,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $aa = [byte[]]::new($a.Stride * $reference.Height)
            $bb = [byte[]]::new($b.Stride * $rust.Height)
            [Runtime.InteropServices.Marshal]::Copy($a.Scan0,$aa,0,$aa.Length)
            [Runtime.InteropServices.Marshal]::Copy($b.Scan0,$bb,0,$bb.Length)
            $max = 0; $different = 0
            for ($i=0; $i -lt $aa.Length; ++$i) {
                $delta = [Math]::Abs([int]$aa[$i]-[int]$bb[$i])
                if ($delta -gt $max) { $max = $delta }
                if ($delta -gt 0) { ++$different }
            }
        } finally { $reference.UnlockBits($a); $rust.UnlockBits($b) }
        if ($max -gt 2) { throw "Image differs: $($pair[1]) maximum channel difference $max" }
        @{ reference=$pair[0]; rust=$pair[1]; width=$rust.Width; height=$rust.Height; maxChannelDifference=$max; differentChannels=$different }
    } finally { $reference.Dispose(); $rust.Dispose() }
}
$results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $PSScriptRoot 'artifacts/cpp-visual-comparison.json')
$results | ConvertTo-Json -Depth 4
