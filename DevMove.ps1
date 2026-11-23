# DevMove Toolkit - Windows 10/11 GUI (PowerShell 5.1 + WinForms)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()
$ErrorActionPreference='Stop'
$script:base=Split-Path -Parent $MyInvocation.MyCommand.Path
$script:runtime=Join-Path $script:base '_runtime'
New-Item -ItemType Directory -Path $script:runtime -Force | Out-Null
$script:busy=$false
$script:activeProcess=$null
$script:statusFile=''
$script:logFile=''
$script:latestOutput=''
$bg=[Drawing.Color]::FromArgb(17,24,36)
$panelColor=[Drawing.Color]::FromArgb(30,40,57)
$fg=[Drawing.Color]::FromArgb(231,239,249)
$muted=[Drawing.Color]::FromArgb(162,176,197)
$accent=[Drawing.Color]::FromArgb(73,169,238)
$form=New-Object Windows.Forms.Form
$form.Text='DevMove Toolkit | Windows Migration'
$form.Size=New-Object Drawing.Size(940,800)
$form.MinimumSize=New-Object Drawing.Size(940,800)
$form.StartPosition='CenterScreen'
$form.BackColor=$bg
$form.ForeColor=$fg
$form.Font=New-Object Drawing.Font('Segoe UI',10)
$form.FormBorderStyle='FixedSingle'
$form.MaximizeBox=$false
function MakeLabel([string]$text,[int]$x,[int]$y,[int]$w,[int]$h,[int]$size,[bool]$bold=$false) {
    $l=New-Object Windows.Forms.Label
    $l.Text=$text; $l.Location=New-Object Drawing.Point($x,$y); $l.Size=New-Object Drawing.Size($w,$h)
    $style=[Drawing.FontStyle]::Regular
    if($bold){$style=[Drawing.FontStyle]::Bold}
    $l.Font=New-Object Drawing.Font('Segoe UI',$size,$style)
    $l.ForeColor=$fg
    $form.Controls.Add($l)
    return $l
}
function MakeButton([string]$name,[int]$x,[int]$y,[int]$w,[int]$h,[bool]$primary=$false){
    $b=New-Object Windows.Forms.Button
    $b.Text=$name; $b.Location=New-Object Drawing.Point($x,$y); $b.Size=New-Object Drawing.Size($w,$h)
    $b.FlatStyle='Flat'; $b.FlatAppearance.BorderSize=0
    $b.BackColor=$(if($primary){$accent}else{$panelColor})
    $b.ForeColor=$(if($primary){[Drawing.Color]::Black}else{$fg})
    $b.Font=New-Object Drawing.Font('Segoe UI',10,[Drawing.FontStyle]::Bold)
    $form.Controls.Add($b)
    return $b
}
function MakeText([int]$x,[int]$y,[int]$w,[int]$h,[string]$value=''){
    $t=New-Object Windows.Forms.TextBox
    $t.Location=New-Object Drawing.Point($x,$y)
    $t.Size=New-Object Drawing.Size($w,$h)
    $t.Text=$value; $t.BackColor=$panelColor; $t.ForeColor=$fg
    $t.BorderStyle='FixedSingle'; $t.Font=New-Object Drawing.Font('Segoe UI',10)
    $form.Controls.Add($t)
    return $t
}
$title=MakeLabel 'DevMove' 26 17 450 45 25 $true
$title.ForeColor=$accent
$sub=MakeLabel 'Select exactly what to carry from your old Windows to your clean install.' 28 64 850 27 10
$sub.ForeColor=$muted
$modeLabel=MakeLabel 'MODE' 28 108 150 25 10 $true
$mode=New-Object Windows.Forms.ComboBox
$mode.Location=New-Object Drawing.Point(28,136)
$mode.Size=New-Object Drawing.Size(420,32)
$mode.DropDownStyle='DropDownList'
$mode.BackColor=$panelColor; $mode.ForeColor=$fg
[void]$mode.Items.Add('Backup - before reinstall')
[void]$mode.Items.Add('Restore - after reinstall')
$mode.SelectedIndex=0
$form.Controls.Add($mode)
$tasksLabel=MakeLabel 'WHAT TO INCLUDE' 28 181 450 28 11 $true
$specs=@(
    @('apps','Installed apps (WinGet)', $true),
    @('vscode','VS Code settings + extensions', $true),
    @('chrome','Chrome bookmarks + history', $true),
    @('xampp','XAMPP configuration', $true),
    @('database','MySQL / MariaDB SQL dump', $true),
    @('node','Node, npm, PHP, Composer versions', $true),
    @('git','Git configuration', $true),
    @('htdocs','XAMPP htdocs projects (may be large)', $false),
    @('ssh','SSH private keys (SENSITIVE)', $false),
    @('custom','Additional folder of my choice', $false)
)
$script:boxes=@{}
for($i=0;$i -lt $specs.Count;$i++){
    $s=$specs[$i]
    $col=[math]::Floor($i/5); $row=$i%5
    $cb=New-Object Windows.Forms.CheckBox
    $cb.Text=[string]$s[1]
    $cb.Checked=[bool]$s[2]
    $cb.Location=New-Object Drawing.Point((30+455*$col),(213+35*$row))
    $cb.Size=New-Object Drawing.Size(440,30)
    $cb.ForeColor=$fg
    $form.Controls.Add($cb)
    $script:boxes[[string]$s[0]]=$cb
}
$hint=MakeLabel 'Passwords, passkeys and logged-in sessions cannot be reliably transferred from Chrome files. Enable Chrome Sync.' 28 398 885 28 9
$hint.ForeColor=[Drawing.Color]::FromArgb(247,192,110)
$customLabel=MakeLabel 'ADDITIONAL FOLDER (optional)' 28 430 450 24 9 $true
$custom=MakeText 28 456 772 28
$customBtn=MakeButton 'Browse' 812 451 99 37
$locLabel=MakeLabel 'BACKUP DESTINATION (choose D:, E: or external drive)' 28 493 650 24 9 $true
$path=MakeText 28 520 772 30
$pathBtn=MakeButton 'Browse' 812 515 99 38
$dbLabel=MakeLabel 'DATABASE USER' 28 563 175 23 9 $true
$dbUser=MakeText 28 587 200 28 'root'
$dbPwdLabel=MakeLabel 'DB PASSWORD (not stored)' 251 563 270 23 9 $true
$dbPwd=MakeText 251 587 270 28
$dbPwd.UseSystemPasswordChar=$true
$preview=MakeButton 'Scan my PC' 540 580 144 37
$verify=MakeButton 'Verify backup' 696 580 215 37
$go=MakeButton 'Start backup' 28 636 200 44 $true
$open=MakeButton 'Open folder' 242 636 154 44
$bar=New-Object Windows.Forms.ProgressBar
$bar.Location=New-Object Drawing.Point(415,648)
$bar.Size=New-Object Drawing.Size(496,21)
$bar.Minimum=0;$bar.Maximum=100
$form.Controls.Add($bar)
$state=MakeLabel 'Ready. Select backup items and an external drive.' 28 688 890 22 9
$state.ForeColor=$muted
$log=New-Object Windows.Forms.TextBox
$log.Location=New-Object Drawing.Point(28,711)
$log.Size=New-Object Drawing.Size(883,46)
$log.Multiline=$true
$log.ReadOnly=$true
$log.ScrollBars='Vertical'
$log.BackColor=$panelColor
$log.ForeColor=$fg
$log.Font=New-Object Drawing.Font('Consolas',8)
$form.Controls.Add($log)
function Report([string]$msg) {
    $log.AppendText($msg+[Environment]::NewLine)
    $log.SelectionStart=$log.TextLength; $log.ScrollToCaret()
}
function SwitchMode {
    if($mode.SelectedIndex -eq 0){
        $locLabel.Text='BACKUP DESTINATION (choose D:, E: or external drive)'
        $go.Text='Start backup'
        $state.Text='Ready. Select backup items and an external drive.'
        $dbPwd.Enabled=$true
    } else {
        $locLabel.Text='EXISTING BACKUP FOLDER (the DevMove_... directory)'
        $go.Text='Start restore'
        $state.Text='Restore overwrites selected settings. Close Chrome and VS Code first.'
        $dbPwd.Enabled=$false
    }
}
$mode.Add_SelectedIndexChanged({ SwitchMode })
$customBtn.Add_Click({
    $dlg=New-Object Windows.Forms.FolderBrowserDialog
    $dlg.Description='Select your additional folder to archive'
    if($dlg.ShowDialog() -eq 'OK'){$custom.Text=$dlg.SelectedPath;$script:boxes['custom'].Checked=$true}
    $dlg.Dispose()
})
$pathBtn.Add_Click({
    $dlg=New-Object Windows.Forms.FolderBrowserDialog
    $dlg.Description=$(if($mode.SelectedIndex -eq 0){'Select a destination OUTSIDE the Windows C: drive'}else{'Select the existing DevMove_... backup folder'})
    if($dlg.ShowDialog() -eq 'OK'){$path.Text=$dlg.SelectedPath}
    $dlg.Dispose()
})
$preview.Add_Click({
    $checks=@(
        @('WinGet', (Test-Path (Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps\winget.exe'))),
        @('VS Code user settings',(Test-Path (Join-Path $env:APPDATA 'Code\User'))),
        @('Chrome User Data',(Test-Path (Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data'))),
        @('XAMPP PHP',(Test-Path 'C:\xampp\php\php.exe')),
        @('XAMPP mysqldump',(Test-Path 'C:\xampp\mysql\bin\mysqldump.exe')),
        @('XAMPP htdocs',(Test-Path 'C:\xampp\htdocs'))
    )
    Report '--- Environment scan (nothing changed) ---'
    foreach($check in $checks) {Report ($check[0]+': '+$(if($check[1]){'Found'}else{'Missing'}))}
    Report ('Chrome currently open: '+[bool](Get-Process chrome -ErrorAction SilentlyContinue))
})
$verify.Add_Click({
    $folder=$path.Text.Trim()
    $file=Join-Path $folder 'manifest.json'
    if(-not(Test-Path -LiteralPath $file)){
        [Windows.Forms.MessageBox]::Show('No manifest.json was found. Choose the DevMove_... backup folder, not its parent.','Backup check','OK','Warning') | Out-Null
        return
    }
    try {
        $m=Get-Content -LiteralPath $file -Raw | ConvertFrom-Json
        $results=Join-Path $folder 'backup-result.json'
        Report ('Backup manifest found: '+$m.date+' | device: '+$m.computer+' | modules: '+(@($m.tasks) -join ', '))
        if(Test-Path $results) {
            $r=Get-Content -LiteralPath $results -Raw | ConvertFrom-Json
            Report ('Task failures: '+$r.failed+'; warnings: '+$r.warnings+'. Inspect backup-log.txt.')
        } else {Report 'No result file found: backup may have been interrupted.'}
        Report 'Verify important SQL dump and file contents manually before formatting Windows.'
    } catch {Report ('Invalid backup manifest: '+$_.Exception.Message)}
})
$open.Add_Click({
    $folder=$script:latestOutput
    if(-not $folder -or -not(Test-Path -LiteralPath $folder)){$folder=$script:base}
    Start-Process -FilePath explorer.exe -ArgumentList ('"'+$folder+'"')
})
$script:timer=New-Object Windows.Forms.Timer
$script:timer.Interval=750
$script:timer.Add_Tick({
    try {
        if($script:logFile -and (Test-Path -LiteralPath $script:logFile)) {
            $content=[IO.File]::ReadAllText($script:logFile)
            if($log.Text -ne $content) {
                $log.Text=$content
                $log.SelectionStart=$log.TextLength;$log.ScrollToCaret()
            }
        }
        if($script:statusFile -and (Test-Path -LiteralPath $script:statusFile)) {
            $st=Get-Content -LiteralPath $script:statusFile -Raw | ConvertFrom-Json
            $total=[math]::Max(1,[int]$st.total)
            $bar.Value=[math]::Min(100,[math]::Max(0,[int](100*$st.done/$total)))
            $state.Text=($st.state+' | '+$st.done+'/'+$st.total+' tasks | '+$st.step+' | '+$st.failed+' failed, '+$st.warnings+' warnings')
            if($st.output){$script:latestOutput=[string]$st.output}
            if($st.state -in @('Completed','CompletedWithWarnings','Failed')) {
                $script:timer.Stop();$script:busy=$false
                $go.Enabled=$true;$mode.Enabled=$true;$pathBtn.Enabled=$true
                $bar.Value=100
                $msg=($st.state+'. '+$st.failed+' failed task(s), '+$st.warnings+' warning(s).'+[Environment]::NewLine+'Location: '+$st.output)
                [Windows.Forms.MessageBox]::Show($msg,'DevMove result','OK','Information') | Out-Null
            }
        } elseif($script:activeProcess -and $script:activeProcess.HasExited) {
            $script:timer.Stop();$script:busy=$false;$go.Enabled=$true;$mode.Enabled=$true;$pathBtn.Enabled=$true
            Report 'Engine exited without status. See _runtime logs and check Engine.ps1.'
        }
    } catch { $state.Text=('Status read issue: '+$_.Exception.Message) }
})
$go.Add_Click({
    if($script:busy){return}
    $selected=@($script:boxes.Keys | Where-Object {$script:boxes[$_].Checked} | Sort-Object)
    if($selected.Count -eq 0){[Windows.Forms.MessageBox]::Show('Select at least one backup category.')|Out-Null;return}
    $folder=$path.Text.Trim()
    if(-not(Test-Path -LiteralPath $folder -PathType Container)){
        [Windows.Forms.MessageBox]::Show('Choose an existing folder using Browse.','Folder required','OK','Warning')|Out-Null;return
    }
    if('custom' -in $selected -and -not(Test-Path -LiteralPath $custom.Text -PathType Container)){
        [Windows.Forms.MessageBox]::Show('Select an existing additional folder, or uncheck that option.')|Out-Null;return
    }
    $what=$(if($mode.SelectedIndex -eq 0){'Backup'}else{'Restore'})
    if($what -eq 'Backup') {
        if([IO.Path]::GetPathRoot($folder) -eq ($env:SystemDrive+'\')) {
            if([Windows.Forms.MessageBox]::Show('The backup destination is on your Windows system drive. A clean Windows installation could erase it. Continue anyway?','Risk of data loss','YesNo','Warning') -ne 'Yes'){return}
        }
        if('ssh' -in $selected -or 'htdocs' -in $selected -or 'custom' -in $selected) {
            if([Windows.Forms.MessageBox]::Show('Selected data may contain private keys, passwords, tokens or .env secrets. Use an encrypted external drive. Continue?','Sensitive data','YesNo','Warning') -ne 'Yes'){return}
        }
    } else {
        if(-not(Test-Path -LiteralPath (Join-Path $folder 'manifest.json'))) {
            [Windows.Forms.MessageBox]::Show('Select the DevMove_... backup folder containing manifest.json.')|Out-Null;return
        }
        if([Windows.Forms.MessageBox]::Show('Restore may overwrite Chrome bookmarks/history, VS Code and Git settings, and install apps. Close Chrome and VS Code first. Proceed?','Confirm restore','YesNo','Warning') -ne 'Yes'){return}
    }
    $stamp=Get-Date -Format 'yyyyMMdd_HHmmss_fff'
    $configFile=Join-Path $script:runtime ('job_'+$stamp+'.json')
    $script:statusFile=Join-Path $script:runtime ('status_'+$stamp+'.json')
    $script:logFile=Join-Path $script:runtime ('log_'+$stamp+'.txt')
    $data=@{mode=$what;backupPath=$folder;tasks=$selected;extraFolder=$custom.Text.Trim();mysqlUser=$dbUser.Text.Trim()}
    $data | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $configFile -Encoding UTF8
    $log.Clear();$bar.Value=0;$script:latestOutput=''
    $state.Text='Starting '+$what.ToLower()+'...'
    $arg='-NoProfile -ExecutionPolicy Bypass -File "'+(Join-Path $script:base 'Engine.ps1')+'" -Config "'+$configFile+'" -StatusPath "'+$script:statusFile+'" -LogPath "'+$script:logFile+'"'
    try {
        $old=$env:DEVMOVE_DB_PASSWORD
        $env:DEVMOVE_DB_PASSWORD=$dbPwd.Text
        $script:activeProcess=Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList $arg -WindowStyle Hidden -PassThru
        if($null -eq $old){Remove-Item Env:\DEVMOVE_DB_PASSWORD -ErrorAction SilentlyContinue}else{$env:DEVMOVE_DB_PASSWORD=$old}
        $dbPwd.Clear()
        $script:busy=$true;$go.Enabled=$false;$mode.Enabled=$false;$pathBtn.Enabled=$false
        $script:timer.Start()
    } catch {
        Remove-Item Env:\DEVMOVE_DB_PASSWORD -ErrorAction SilentlyContinue
        $go.Enabled=$true
        [Windows.Forms.MessageBox]::Show('Could not start backup engine: '+$_.Exception.Message)|Out-Null
    }
})
$form.Add_FormClosing({
    if($script:busy) {
        if([Windows.Forms.MessageBox]::Show('A job is running. Closing the GUI will NOT cancel the background process. Close the window?','Job active','YesNo','Warning') -ne 'Yes'){$_.Cancel=$true}
    }
})
[Windows.Forms.Application]::Run($form)
