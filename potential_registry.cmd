set /p "qry=Enter Query String:" && for /f "delims=" %i in ('powershell -c "Get-ChildItem -Path """%^qry%""" -recurse | % { $_.Name }" ^| fzf --bind="ctrl-c:execute(req query {})"') do reg query "%i" /s
