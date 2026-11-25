param(
    [Parameter(Mandatory)][string]$Player,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][ValidateSet('VSR','HDR','VSR-HDR','NativeHDR')][string]$Mode,
    [Parameter(Mandatory)][int]$Width,
    [Parameter(Mandatory)][int]$Height,
    [switch]$CheckRendererOnly,
    [ValidateSet('1','2','2.5','3','4','5','6')][string]$Interpolation='1'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -Namespace RtxTest -Name NativeWindow -MemberDefinition @'
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
'@ -ErrorAction SilentlyContinue
$app = Start-Process -FilePath (Resolve-Path $Player) -ArgumentList ('"' + (Resolve-Path $Fixture) + '"') -PassThru
function Find-Control([string]$Name) {
    $match = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name)
    $button = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Button)
    $root.FindFirst([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.AndCondition]::new($match, $button))
}
function Wait-For([scriptblock]$Check, [string]$Message) {
    $deadline = [datetime]::UtcNow.AddSeconds(9)
    do {
        if ($app.HasExited) { throw 'Player exited early' }
        if (& $Check) { return }
        Start-Sleep -Milliseconds 150
    } while ([datetime]::UtcNow -lt $deadline)
    throw $Message
}
function Select-Toggle([string]$Name) {
    $control = Find-Control $Name
    if (!$control -or !$control.Current.IsEnabled) { throw "Control unavailable: $Name" }
    $control.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle()
    Wait-For { (Find-Control $Name).GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -eq 'On' } "Activation unconfirmed: $Name"
}
function Status-Text {
    $root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { $_.Current.Name } | Where-Object { $_ -like 'Filter *' } | Select-Object -First 1
}
try {
    Wait-For { $app.Refresh(); $app.MainWindowHandle -ne 0 -and $app.MainWindowTitle -eq 'RTX Player' } 'No desktop window'
    $window = $app.MainWindowHandle
    $root = [Windows.Automation.AutomationElement]::FromHandle($window)
    Wait-For { ($c = Find-Control 'Pause') -and $c.Current.IsEnabled } 'Media did not open'
    Select-Toggle 'Playback statistics'
    if ($Interpolation -ne '1') {
        (Find-Control 'Enhancements').GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        $name=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,"RIFE ${Interpolation}×")
        $radio=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::RadioButton)
        $query=[Windows.Automation.AndCondition]::new($name,$radio)
        Wait-For {$null -ne $root.FindFirst([Windows.Automation.TreeScope]::Descendants,$query)} 'Interpolation controls unavailable'
        $item=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$query)
        $choice=$item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
        $choice.Select()
        Wait-For {$choice.Current.IsSelected} 'RIFE selection was not applied'
        (Find-Control 'Back to player').GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        Start-Sleep -Milliseconds 1200
    }
    if ($Mode -like 'VSR*') { Select-Toggle 'RTX Super Resolution' }
    if ($Mode -in 'HDR','VSR-HDR') { Select-Toggle 'RTX HDR' }
    if ($Mode -like 'VSR*') { $Width *= 2; $Height *= 2 }
    $expected = "Filter ${Width}×${Height} "
    try {
        Wait-For { (Status-Text) -like "$expected*" } "Expected processed size ${Width}×${Height}"
        if ($Mode -like '*HDR') {
            Wait-For { (Status-Text) -match ' pq · Target pq bt\.2020 .*Windows HDR \d+-bit · HDR10 swap chain' } 'Missing PQ/BT.2020 HDR10 swap chain on the active HDR display'
            if (!$CheckRendererOnly -and (Status-Text) -notmatch 'Windows HDR (10|12|16)-bit') {
                throw 'Windows reports fewer than ten bits per channel on the display link'
            }
        }
    } finally { Write-Output (Status-Text) }
    if ($Interpolation -ne '1') {
        $timing=$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) |
            ForEach-Object { $_.Current.Name } | Where-Object { $_ -match ' · Source .* · Filter ' } |Select-Object -First 1
        if ($timing -notmatch 'Source ([\d.]+) fps · Filter ([\d.]+) fps' -or [Math]::Abs([double]$Matches[1]*[double]$Interpolation-[double]$Matches[2]) -gt 1) {throw "Unexpected interpolation timing: $timing"}
        Write-Output $timing
    }
    & python "$PSScriptRoot/visible_playback.py" --pid $app.Id
    if ($LASTEXITCODE) { throw 'Visible playback check failed' }
} finally {
    if (!$app.HasExited) {
        if ($window) { [void][RtxTest.NativeWindow]::PostMessage($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
        if (!$app.WaitForExit(8000)) { $app.Kill(); throw 'Player did not close normally' }
    }
}
