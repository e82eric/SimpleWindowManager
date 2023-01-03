#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "..\\SimpleWindowManager2\\SimpleWindowManager.h"

void search_all_drives_command_run(ScratchWindow *self, Monitor *monitor, int scratchWindowsScreenPadding)
{
    CHAR cmdBuff[4096];
    sprintf_s(
            cmdBuff,
            4096,
            "cmd /c powershell -c \
            \"$drive = (Get-PSDrive -PSProvider FileSystem).Root \
            | bin\\RunProcess.exe --title ScratchMenu%ls -l %d -t %d -w %d -h %d; \
            if($drive -eq $null) { exit }; \
            bin\\RunProcess.exe --namedCommand \"\"\"ld:fd . $($drive)\\\\\"\"\" \
            --loadCommand ld\" --title ScratchMenu%ls\" -l %d -t %d -w %d -h %d",
            self->uniqueStr,
            monitor->xOffset + scratchWindowsScreenPadding,
            scratchWindowsScreenPadding,
            monitor->w - (scratchWindowsScreenPadding * 2),
            monitor->h - (scratchWindowsScreenPadding * 2),
            self->uniqueStr,
            monitor->xOffset + scratchWindowsScreenPadding,
            scratchWindowsScreenPadding,
            monitor->w - (scratchWindowsScreenPadding * 2),
            monitor->h - (scratchWindowsScreenPadding * 2));

    process_with_stdout_start(cmdBuff, self->stdOutCallback);
}

BOOL should_float_be_focused(Client *client)
{
    if(wcsstr(L"Microsoft Teams Notification", client->data->title))
    {
        return FALSE;
    }

    return TRUE;
}

BOOL is_float_window_from_config(Client *client, LONG_PTR styles, LONG_PTR exStyles)
{
    UNREFERENCED_PARAMETER(styles);
    UNREFERENCED_PARAMETER(exStyles);

    if(styles & WS_POPUP)
    {
        if(wcsstr(client->data->className, L"Qt5152QWindowIcon"))
        {
            return FALSE;
        }
        if(wcsstr(client->data->className, L"Qt5QWindowIcon"))
        {
            return FALSE;
        }
        return TRUE;
    }

    return FALSE;
}

BOOL should_use_old_move_logic(Client* client)
{
    if(wcsstr(client->data->processImageName, L"Teams.exe"))
    {
        return TRUE;
    }
    return FALSE;
}

void configure(Configuration *configuration)
{
    configuration->isFloatWindowFunc = is_float_window_from_config;
    configuration->shouldFloatBeFocusedFunc = should_float_be_focused;
    configuration->useOldMoveLogicFunc = should_use_old_move_logic;

    WCHAR chromeTag = { 0xfa9e };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xf668 };
    WCHAR teamsTag = { 0xf865 };

    TCHAR *browserProcessImageNames[4] = { L"chrome.exe", L"brave.exe", L"firefox.exe", L"msedge.exe" };
    workspace_register_with_filter_data(L"Chrome", &chromeTag, &tileLayout, NULL, 0, browserProcessImageNames, 4, NULL, 0);

    TCHAR *terminalProcessImageName[1] = { L"WindowsTerminal.exe" };
    workspace_register_with_filter_data(L"Terminal", &terminalTag, &deckLayout, NULL, 0, terminalProcessImageName, 1, NULL, 0);

    TCHAR *riderTitles[2] = { L"rider64.exe", L"Code.exe" };
    workspace_register_with_filter_data(L"Rider", &riderTag, &monacleLayout, NULL, 0, riderTitles, 2, NULL, 0);

    TCHAR *teamsProcessImageName[1] = { L"Teams.exe" };
    workspace_register_with_filter_data(L"Teams", &teamsTag, &tileLayout, NULL, 0, teamsProcessImageName, 1, NULL, 0);

    workspace_register(L"5", NULL, L"5", &tileLayout);
    workspace_register(L"6", NULL, L"6", &tileLayout);
    workspace_register(L"7", NULL, L"7", &tileLayout);

    keybindings_register_defaults();

    keybinding_create_cmd_args(LWin, VK_T, start_app, L"wt.exe");

    scratch_terminal_register_with_unique_string(
            "cmd /c vifm",
            LAlt,
            VK_Q,
            L"4d237b71-4df7-4d2e-a60f-f9d7f69e80a7");

    scratch_terminal_register_with_unique_string(
            "cmd /c powershell -nologo",
            LAlt | LShift,
            VK_W,
            L"ea3f98ca-973f-4154-9a7b-dc7a9377b768");

    scratch_terminal_register_with_unique_string(
            "cmd /c nvim -c \"terminal powershell -nologo\"",
            LAlt,
            VK_W,
            L"643763f5-f5cd-416e-a5c9-bef1f516863c");

    scratch_menu_register(
            "--namedCommand \"ld:bin\\ListWindows.exe\" --loadCommand \"ld\"",
            open_windows_scratch_exit_callback,
            LAlt,
            VK_SPACE,
            L"127ac0a0-9f30-47ef-b225-1dda332beb19");

    scratch_menu_register_command_from_function(
            NULL,
            open_program_scratch_callback_not_elevated,
            LAlt,
            VK_E,
            L"8f631b05-9d5d-4c19-b3b7-f285835785ea",
            search_all_drives_command_run);

    scratch_menu_register(
            "--namedCommand \"ld:fd -t f -g \"\"*{.lnk,.exe}\"\" \
            \"\"%PROGRAMFILES%\"\" \
            \"\"%PROGRAMFILES(X86)%\"\" \
            \"\"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\"\" \
            \"\"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\"\" \
            \"\"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\"\" \
            --loadCommand \"ld\"",
            open_program_scratch_callback,
            LAlt | LShift,
            VK_P,
            L"dd07e19f-5d2f-427e-9c8f-ad548ef0acfb");

    scratch_menu_register(
            "--namedCommand \"ld:fd -t f -g \"\"*{.lnk,.exe}\"\" \
            \"\"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\"\" \
            \"\"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\"\" \
            \"\"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\"\" \
            \"\"%USERPROFILE%\\Utilites\"\"\" --loadCommand \"ld\"",
            open_program_scratch_callback_not_elevated,
            LAlt,
            VK_P,
            L"8f6579ce-727c-4d5b-9e76-2cdd1db52e52");

    scratch_menu_register(
            "--hasHeader \
            --loadCommand \"ld\" --namedCommand \"ld:bin\\ListProcesses.exe\" \
            --namedCommand \"procKill:cmd /c taskkill /f /pid {}\" \
            --bind \"ctl-k:procKill:reload\" \
            --returnRange 46,8 \
            --namedCommand \"windbg:powershell.exe -C '{}' -match '\\d{8}';windbg -p \"\"\"$([int]$Matches[0])\"\"\"\" \
            --namedCommand \"sortPvtBytes:bin\\ListProcesses.exe --sort privateBytes\" \
            --namedCommand \"sortWrkSet:bin\\ListProcesses.exe --sort workingSet\" \
            --namedCommand \"sortCpu:bin\\ListProcesses.exe --sort cpu\" \
            --bindLoadCommand \"ctl-1:sortPvtBytes:reload\" \
            --bindLoadCommand \"ctl-2:sortWrkSet:reload\" \
            --bindLoadCommand \"ctl-3:sortCpu:reload\" \
            --bindExecuteAndQuit \"ctl-d:windbg:quit\"",
            open_process_list_scratch_callback,
            LWin,
            VK_9,
            L"1a19cef1-1d1c-47b5-84c3-e5e2d1ffe141");
}
