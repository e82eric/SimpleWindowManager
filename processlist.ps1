$ErrorActionPreferece = "STOP"
$env:SHELL="powershell"

function GetOutput {
	param(
		$sortColumn,
		$desc)
	$header1 = "1 Reload:      ctrl-r"
	$header2 = "1 Kill Proces: ctrl-k"
	$header3 = "1 Sort By:     ctrl-a:Pid ctrl-s:Name ctrl-d:CPU ctrl-f:Private Bytes ctrl-g:WorkingSet ctrl-h:StartTime"

$getProcessOutput = Get-Process | Sort-Object $sortColumn -Descending:$desc | Format-Table `
	@{Label = "SC"; Expression = { $sortColumn } }, `
	@{Label = "PID"; Expression = { $_.Id.ToString("D8") } }, `
	ProcessName,
	@{Label = "CPU(s)"; Expression = { if($_.CPU) { $_.CPU.ToString("N0") }}; align = "right" }, `
	@{Label = "NonPaged(K)"; Expression = { if($_.NPM) { ($_.NPM / 1024).ToString("N0") }}; align = "right" },
	@{Label = "Paged/Private(K)"; Expression = {if ($_.PM) { ($_.PM / 1024).ToString("N0")}}; align = "right" },
	@{Label = "WorkingSet(K)"; Expression = {if($_.CPU) { ($_.WS / 1024).ToString("N0")}}; align = "right" },
	@{Label = "StartTime"; Expression = { ($_.StartTime.ToString("s"))} } `
	-AutoSize:$false

	$result = @($header1, $header2, $header3, $getProcessOutput)
	$result
}

function Run {
	GetOutput CPU $true | Fzf `
		--with-nth 2.. `
		--header-lines=6 `
		--reverse `
		--bind="ctrl-a:reload(. .\processlist.ps1; GetOutput Id 1)" `
		--bind="ctrl-s:reload(. .\processlist.ps1; GetOutput Name 0)" `
		--bind="ctrl-d:reload(. .\processlist.ps1; GetOutput CPU 1)" `
		--bind="ctrl-f:reload(. .\processlist.ps1; GetOutput PM 1)" `
		--bind="ctrl-g:reload(. .\processlist.ps1; GetOutput WS 1)" `
		--bind="ctrl-h:reload(. .\processlist.ps1; GetOutput StartTime 1)" `
		--bind="ctrl-r:reload(. .\processlist.ps1;GetOutput {1} 1)" `
		--bind="ctrl-k:execute(taskkill /f /pid {2})+reload(. .\processlist.ps1;GetOutput {1} 1)"
}
