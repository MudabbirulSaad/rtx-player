param([Parameter(Mandatory)][string]$Player,[Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient,System.Drawing,System.Windows.Forms
Add-Type -Namespace CaptureTest -Name Native -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
"@
$exe=(Resolve-Path -LiteralPath $Player).Path
$clip=(Resolve-Path -LiteralPath $Fixture).Path
$folder=Split-Path -Parent $clip

$report=Join-Path $folder 'video-frame.png'
if(Test-Path -LiteralPath $report){throw 'Use a new test output folder'}
$app=Start-Process $exe -ArgumentList ('"'+$clip+'"') -NoNewWindow -PassThru
function Named($parent,$name){
    $match=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name)
    $parent.FindFirst([Windows.Automation.TreeScope]::Descendants,$match)
}
function Wait-For([scriptblock]$check,$reason){
    $end=[datetime]::UtcNow.AddSeconds(10)
    do{$value=& $check;if($value){return $value};Start-Sleep -Milliseconds 120}while(!$app.HasExited -and [datetime]::UtcNow -lt $end)
    throw $reason
}
function Click($name){(Wait-For {Named $root $name} "Missing $name").GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()}
try{
    $hwnd=Wait-For {$app.Refresh();if($app.MainWindowTitle -eq 'RTX Player'){$app.MainWindowHandle}} 'No player window'
    $root=[Windows.Automation.AutomationElement]::FromHandle($hwnd)
    [void](Wait-For {$p=Named $root 'Pause';if($p -and $p.Current.IsEnabled){$p}} 'No media')
    Click 'Pause'
    Click 'Enhancements';Click 'Performance'
    [void](Wait-For {(Named $root 'Anime4K').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -eq 'On'} 'Preset did not enable Anime4K')
    Click 'Back to player'
    $compare=Named $root 'Compare original scaling'
    $compare.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle()
    [void](Wait-For {(Named $root 'Anime4K').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -eq 'Off'} 'Spatial comparison did not restore original')
    (Named $root 'Compare original scaling').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle()
    [void](Wait-For {(Named $root 'Anime4K').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -eq 'On'} 'Returning to enhancement failed')
    $initialWidth=$root.Current.BoundingRectangle.Width
    $compact=(Named $root 'Compact player').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    $compact.Toggle();[void](Wait-For {$compact.Current.ToggleState -eq 'On' -and $root.Current.BoundingRectangle.Width -lt $initialWidth} 'No compact window')
    $compact.Toggle();[void](Wait-For {$compact.Current.ToggleState -eq 'Off' -and $root.Current.BoundingRectangle.Width -eq $initialWidth} 'Window size was not restored')
    $beforeCursor=[Windows.Forms.Cursor]::Position
    try{
        $bar=(Named $root 'Seek').Current.BoundingRectangle
        [Windows.Forms.Cursor]::Position=[Drawing.Point]::new([int]($bar.X+$bar.Width*.25),[int]($bar.Y+$bar.Height/2))
        [void](Wait-For {$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -like 'Preview at * seconds'}|Select-Object -First 1} 'No decoded timeline preview')
    }finally{[Windows.Forms.Cursor]::Position=$beforeCursor}
    Click 'Save video frame'
    $dialog=Wait-For {
        $r=$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -eq 'Save video frame' -and $_.Current.ControlType -eq [Windows.Automation.ControlType]::Window}|Select-Object -First 1
        if(!$r){$r=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.ProcessId -eq $app.Id -and $_.Current.Name -eq 'Save video frame'}|Select-Object -First 1}
        $r
    } 'Save dialog unavailable'
    [void](Wait-For {
        $dialog.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -eq ("Address: "+$folder) -or $_.Current.Name -eq $folder -or $_.Current.Name -eq (Split-Path $folder -Leaf)}|Select-Object -First 1
    } 'The picker did not identify the test directory')
    $save=[CaptureTest.Native]::GetDlgItem($dialog.Current.NativeWindowHandle,1)
    if(!$save -or ![CaptureTest.Native]::PostMessage($save,0xf5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Cannot accept save dialog'}
    [void](Wait-For {Test-Path -LiteralPath $report -PathType Leaf} 'Screenshot missing')
    $bitmap=[Drawing.Bitmap]::new($report)
    try{
        if($bitmap.Width -lt 300 -or $bitmap.Height -lt 160){throw 'Wrong screenshot dimensions'}
        $colors=0
        for($i=1;$i -lt 32;$i++){for($j=1;$j -lt 18;$j++){
            $p=$bitmap.GetPixel([int]($bitmap.Width*$i/32),[int]($bitmap.Height*$j/18))
            if([Math]::Max($p.R,[Math]::Max($p.G,$p.B))-[Math]::Min($p.R,[Math]::Min($p.G,$p.B)) -gt 80){$colors++}
        }}
        if($colors -lt 100){throw 'Saved frame lacks the expected test pattern'}
        "PNG $($bitmap.Width)x$($bitmap.Height), $colors saturated samples. Seek preview, spatial reference, preset and compact window verified."
    }finally{$bitmap.Dispose()}
}finally{if(!$app.HasExited){[void]$app.CloseMainWindow();if(!$app.WaitForExit(7000)){$app.Kill();Write-Warning 'Player shutdown timed out'}}}
