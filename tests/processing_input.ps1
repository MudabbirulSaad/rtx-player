param([string]$Player,[string]$Media,[switch]$Hdr)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
$app=Start-Process $Player -ArgumentList ('"'+$Media+'"') -PassThru
function Until($task,$why){
    $end=[datetime]::UtcNow.AddSeconds(20)
    do{ $v=& $task; if($v){return $v};Start-Sleep -Milliseconds 175 }while(!$app.HasExited -and [datetime]::UtcNow -lt $end)
    Write-Host ((Texts|Where-Object {$_}) -join " | ");throw $why
}
function Named($name){
    $root.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name))
}
function Click($name){(Until {Named $name} $name).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()}
function Texts {
    $root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) | ForEach-Object {$_.Current.Name}
}
function Radio($name){
    $o=Until {Named $name} $name
    $item=$o.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
    $item.Select();[void](Until {$item.Current.IsSelected} "Radio $name")
}
function Toggle($name){
    $o=Until {Named $name} $name
    $item=$o.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    $item.Toggle();[void](Until {$item.Current.ToggleState -eq 'On'} "Toggle $name")
}
try{
    $hwnd=Until {$app.Refresh();if($app.MainWindowTitle -eq 'RTX Player'){$app.MainWindowHandle}} 'No window'
    $root=[Windows.Automation.AutomationElement]::FromHandle($hwnd)
    [void](Until {$x=Named 'Pause'; $x -and $x.Current.IsEnabled} 'No media')
    Toggle 'Playback statistics'
    Click 'Enhancements';Click 'Experimental 720p24 input'
    Until {Texts|Where-Object {$_ -like 'Filter 1280×720 *'}|Select-Object -First 1} 'Failed to reduce to 720p'
    Until {Texts|Where-Object {$_ -match 'Source 30\.00 fps · Filter 24\.\d+ fps'}|Select-Object -First 1} 'No 24 fps input'
    if($Hdr){
        Radio 'RTX scale 3×'; Radio 'RIFE 6×'; Toggle 'RTX Super Resolution'; Toggle 'RTX HDR'
        Until {Texts|Where-Object {$_ -like 'Filter 3840×2160 pq*Target pq bt.2020*Windows HDR*HDR10 swap chain*'}|Select-Object -First 1} 'No reconstructed UHD PQ output'
    }else{
        Click 'Apply target FPS'
        Until {Texts|Where-Object {$_ -match 'Source 30\.00 fps · Filter (59|60)\.\d+ fps'}|Select-Object -First 1} 'No 60 fps filter'
    }
    Click 'Back to player'
    python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if($LASTEXITCODE){throw 'No visible frames'}
    Texts|Where-Object {$_ -match '(Source .* fps · Filter|^Filter .*Target|failed|unavailable\.)'}
}finally{if(!$app.HasExited){[void]$app.CloseMainWindow();if(!$app.WaitForExit(8000)){$app.Kill()}}}
