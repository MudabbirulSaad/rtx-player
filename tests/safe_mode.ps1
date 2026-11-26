param([Parameter(Mandatory)][string]$Player,[Parameter(Mandatory)][string]$Fixture)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -Namespace SafeModeTest -Name Native -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
"@
$exe=(Resolve-Path -LiteralPath $Player).Path
$clip=(Resolve-Path -LiteralPath $Fixture).Path
$folder=Split-Path -Parent $clip
if(!(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $exe) 'portable.mode'))){throw 'Use a portable test build'}
$report=Join-Path $folder 'rtx-player-diagnostics.json'
if(Test-Path -LiteralPath $report){throw 'Use a new test output folder'}
$app=Start-Process $exe -ArgumentList ('--safe-mode "'+$clip+'"') -NoNewWindow -PassThru
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
    if((Named $root 'RTX Super Resolution').Current.IsEnabled){throw 'Safe mode allowed RTX processing'}
    Click 'Settings'
    [void](Wait-For {Named $root 'Safe mode is active. Playback uses software decoding and disables enhancements.'} 'Missing safe mode indicator')
    Click 'Save diagnostic report'
    $dialog=Wait-For {
        $r=Named $root 'Save playback diagnostics'
        if(!$r){$r=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.ProcessId -eq $app.Id -and $_.Current.Name -eq 'Save playback diagnostics'}|Select-Object -First 1}
        $r
    } 'Save dialog unavailable'
    [void](Wait-For {
        $dialog.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -eq ("Address: "+$folder) -or $_.Current.Name -eq $folder -or $_.Current.Name -eq (Split-Path $folder -Leaf)}|Select-Object -First 1
    } 'The picker did not identify the test directory')
    $save=[SafeModeTest.Native]::GetDlgItem($dialog.Current.NativeWindowHandle,1)
    if(!$save -or ![SafeModeTest.Native]::PostMessage($save,0xf5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Cannot accept save dialog'}
    [void](Wait-For {Test-Path -LiteralPath $report -PathType Leaf} 'Diagnostic report missing')
    $raw=Get-Content -Raw -LiteralPath $report
    $doc=$raw|ConvertFrom-Json
    if(!$doc.playback.safe_mode -or $doc.playback.decoder -ne 'no' -or !$doc.recent_events.Count){throw 'Report does not verify safe-mode decoding'}
    if($raw.Contains([IO.Path]::GetFileName($clip)) -or $raw.Contains($env:USERPROFILE.Replace('\','\\'))){throw 'Private media identifier in report'}
    Click 'Back to player'
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'Software playback did not show moving colors'}
    'Software video visible, enhancements disabled and sanitized diagnostic report verified.'
}finally{if(!$app.HasExited){[void]$app.CloseMainWindow();if(!$app.WaitForExit(7000)){$app.Kill();Write-Warning 'Player shutdown timed out'}}}
