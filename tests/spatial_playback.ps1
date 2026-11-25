param([Parameter(Mandatory)][string]$Player, [Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class VideoSample {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [DllImport("user32")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32")] public static extern bool SetProcessDPIAware();
}
"@
[void][VideoSample]::SetProcessDPIAware()
$app = Start-Process -FilePath (Resolve-Path $Player) -ArgumentList ('"' + (Resolve-Path $Fixture) + '"') -PassThru
function Invoke-Button([string]$Name) {
    $match = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name)
    $button = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $match)
    if (!$button -or !$button.Current.IsEnabled) { throw "Unavailable control: $Name" }
    if ($button.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $Name -ne 'Anime4K') {
        $button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        $button.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle()
    }
}
function Capture {
    $bounds = $root.Current.BoundingRectangle
    $x = [int]($bounds.X + $bounds.Width / 2 - 240)
    $y = [int]($bounds.Y + $bounds.Height * 0.4 - 96)
    $image = [Drawing.Bitmap]::new(480, 192)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        foreach ($dx in 0, 240, 479) {
            foreach ($dy in 0, 96, 191) {
                $point = [VideoSample+Point]::new()
                $point.X = $x + $dx
                $point.Y = $y + $dy
                [uint32]$owner = 0
                [void][VideoSample]::GetWindowThreadProcessId([VideoSample]::WindowFromPoint($point), [ref]$owner)
                if ($owner -ne $app.Id) { throw 'Video sample is obscured' }
            }
        }
        $graphics.CopyFromScreen($x, $y, 0, 0, $image.Size)
        return $image
    } finally { $graphics.Dispose() }
}
function Count-Changes($a, $b) {
    $count = 0
    for ($y = 0; $y -lt 192; $y += 2) {
        for ($x = 0; $x -lt 480; $x += 2) {
            $p = $a.GetPixel($x,$y)
            $q = $b.GetPixel($x,$y)
            if ([Math]::Abs($p.R-$q.R) + [Math]::Abs($p.G-$q.G) + [Math]::Abs($p.B-$q.B) -gt 16) { $count++ }
        }
    }
    return $count
}
try {
    Start-Sleep -Seconds 3
    $app.Refresh()
    $root = [Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    Invoke-Button 'Pause'
    Start-Sleep -Milliseconds 700
    $original = Capture
    Invoke-Button 'Anime4K'
    Start-Sleep -Milliseconds 900
    $enhanced = Capture
    Invoke-Button 'Anime4K'
    Start-Sleep -Milliseconds 900
    $restored = Capture
    $changed = Count-Changes $original $enhanced
    $residual = Count-Changes $original $restored
    if ($changed -lt 30 -or $residual -gt 5) { throw "Unexpected pixel counts: changed=$changed residual=$residual" }
    "Paused frame: $changed changed samples with Anime4K, $residual after disabling it"
} finally {
    foreach ($image in $original, $enhanced, $restored) { if ($image) { $image.Dispose() } }
    if (!$app.HasExited) {
        [void]$app.CloseMainWindow()
        if (!$app.WaitForExit(5000)) { $app.Kill(); throw 'Player did not close normally' }
    }
}
