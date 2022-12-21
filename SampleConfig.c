#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "..\\SimpleWindowManager2\\SimpleWindowManager.h"

long barHeight = 29;
long gapWidth = 13;

COLORREF barBackgroundColor = 0x282828;
COLORREF barSelectedBackgroundColor = RGB(84, 133, 36);
COLORREF buttonSelectedTextColor = RGB(204, 36, 29);
COLORREF buttonWithWindowsTextColor = RGB(255, 255, 247);
COLORREF buttonWithoutWindowsTextColor = 0x504945;
COLORREF barTextColor =RGB(235, 219, 178);

TCHAR *scratchWindowTitle = L"SimpleWindowManager Scratch";

HFONT initalize_font()
{
    HDC hdc = GetDC(NULL);
    long lfHeight;
    lfHeight = -MulDiv(14, GetDeviceCaps(hdc, LOGPIXELSY), 72);
    HFONT font = CreateFontW(lfHeight, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, TEXT("Hack Regular Nerd Font Complete"));
    DeleteDC(hdc);
    return font;
}

BOOL chrome_workspace_filter(Client *client)
{
    if(
        wcsstr(client->data->processImageName, L"chrome.exe") ||
        wcsstr(client->data->processImageName, L"brave.exe") ||
        wcsstr(client->data->processImageName, L"firefox.exe") ||
        wcsstr(client->data->processImageName, L"msedge.exe"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL terminal_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"SimpleMenuTitle"))
    {
        return FALSE;
    }
    if(wcsstr(client->data->processImageName, L"WindowsTerminal.exe"))
    {
        TCHAR *cmdLine = client_get_command_line(client);
        if(wcsstr(cmdLine, L"SimpleMenuTitle"))
        {
            return FALSE;
        }
        return TRUE;
    }
    return FALSE;
}

BOOL terminal_with_uniqueStr_filter(ScratchWindow *self, Client *client)
{
    if(wcsstr(client->data->processImageName, L"WindowsTerminal.exe"))
    {
        TCHAR *cmdLine = client_get_command_line(client);
        if(wcsstr(cmdLine, self->uniqueStr))
        {
            return TRUE;
        }
    }
    return FALSE;
}

BOOL rider_workspace_filter(Client *client)
{
    if(wcsstr(client->data->processImageName, L"rider64.exe"))
    {
        return TRUE;
    }
    if(wcsstr(client->data->className, L"CASCADIA_HOSTING_WINDOW_CLASS"))
    {
        return TRUE;
    }
    if(wcsstr(client->data->processImageName, L"Code.exe"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL paint_workspace_filter(Client *client)
{
    if(wcsstr(client->data->className, L"MSPaintApp"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL teams_workspace_filter(Client *client)
{
    if(wcsstr(client->data->processImageName, L"Teams.exe"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL null_filter(Client *client)
{
    UNREFERENCED_PARAMETER(client);
    return FALSE;
}

BOOL virtualboxmanager_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"Oracle VM VirtualBox Manager"))
    {
        return TRUE;
    }
    return FALSE;
}

BOOL windowsvm_workspace_filter(Client *client)
{
    if(wcsstr(client->data->title, L"NewWindows10"))
    {
        return TRUE;
    }
    return FALSE;
}

Workspace** create_workspaces(int* outSize)
{
    WCHAR chromeTag = { 0xfa9e };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xf668 };
    WCHAR archTag = { 0xf303 };
    WCHAR windows10Tag = { 0xe70f };
    WCHAR teamsTag = { 0xf865 };

    int* numberOfWorkspaces = malloc(sizeof(int));
    *numberOfWorkspaces = 9;

    Workspace** workspaces = calloc(*numberOfWorkspaces, sizeof(Workspace *));
    workspaces[0] = workspace_create(L"Chrome", chrome_workspace_filter, &chromeTag, &tileLayout, numberOfBars);
    workspaces[1] = workspace_create(L"Terminal", terminal_workspace_filter, &terminalTag, &deckLayout, numberOfBars);
    workspaces[2] = workspace_create(L"Rider", rider_workspace_filter, &riderTag, &monacleLayout, numberOfBars);
    workspaces[3] = workspace_create(L"4", null_filter, L"4", &tileLayout, numberOfBars);
    workspaces[4] = workspace_create(L"5", null_filter, L"5", &tileLayout, numberOfBars);
    workspaces[5] = workspace_create(L"Teams", teams_workspace_filter, &teamsTag, &tileLayout, numberOfBars);
    workspaces[6] = workspace_create(L"7", null_filter, L"7", &tileLayout, numberOfBars);
    workspaces[7] = workspace_create(L"8", null_filter, L"8", &tileLayout, numberOfBars);
    workspaces[8] = workspace_create(L"9", null_filter, L"9", &tileLayout, numberOfBars);

    *outSize = *numberOfWorkspaces;
    return workspaces;
}

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

TCHAR *terminalAppPath = L"wt.exe";

void create_keybindings(Workspace** workspaces)
{
    keybinding_create_no_args(LAlt, VK_J, select_next_window);
    keybinding_create_no_args(LAlt, VK_K, select_previous_window);
    keybinding_create_no_args(LAlt, VK_OEM_COMMA, monitor_select_next);
    keybinding_create_no_args(LAlt, VK_N, arrange_clients_in_selected_workspace);
    keybinding_create_no_args(LAlt, VK_L, workspace_increase_master_width_selected_monitor);
    keybinding_create_no_args(LAlt, VK_H, workspace_decrease_master_width_selected_monitor);
    keybinding_create_no_args(LAlt | LShift, VK_RIGHT, move_focused_window_right);
    keybinding_create_no_args(LAlt | LShift, VK_LEFT, move_focused_window_left);
    keybinding_create_no_args(LAlt | LShift, VK_UP, move_focused_window_up);
    keybinding_create_no_args(LAlt | LShift, VK_DOWN, move_focused_window_down);
    keybinding_create_no_args(LAlt, VK_RETURN, move_focused_window_to_master);
    keybinding_create_no_args(LShift | LAlt, VK_DOWN, mimimize_focused_window);

    keybinding_create_workspace_arg(LAlt, VK_1, swap_selected_monitor_to, workspaces[0]);
    keybinding_create_workspace_arg(LAlt, VK_2, swap_selected_monitor_to, workspaces[1]);
    keybinding_create_workspace_arg(LAlt, VK_3, swap_selected_monitor_to, workspaces[2]);
    keybinding_create_workspace_arg(LAlt, VK_4, swap_selected_monitor_to, workspaces[3]);
    keybinding_create_workspace_arg(LAlt, VK_5, swap_selected_monitor_to, workspaces[4]);
    keybinding_create_workspace_arg(LAlt, VK_6, swap_selected_monitor_to, workspaces[5]);
    keybinding_create_workspace_arg(LAlt, VK_7, swap_selected_monitor_to, workspaces[6]);
    keybinding_create_workspace_arg(LAlt, VK_8, swap_selected_monitor_to, workspaces[7]);
    keybinding_create_workspace_arg(LAlt, VK_9, swap_selected_monitor_to, workspaces[8]);

    keybinding_create_no_args(LShift | LAlt, VK_J, move_focused_client_next);
    keybinding_create_no_args(LShift | LAlt, VK_K, move_focused_client_previous);

    keybinding_create_workspace_arg(LShift | LAlt, VK_1, move_focused_window_to_workspace, workspaces[0]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_2, move_focused_window_to_workspace, workspaces[1]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_3, move_focused_window_to_workspace, workspaces[2]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_4, move_focused_window_to_workspace, workspaces[3]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_5, move_focused_window_to_workspace, workspaces[4]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_6, move_focused_window_to_workspace, workspaces[5]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_7, move_focused_window_to_workspace, workspaces[6]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_8, move_focused_window_to_workspace, workspaces[7]);
    keybinding_create_workspace_arg(LShift | LAlt, VK_9, move_focused_window_to_workspace, workspaces[8]);

    keybinding_create_no_args(LShift | LAlt, VK_C, close_focused_window);
    keybinding_create_no_args(LShift | LAlt, VK_C, kill_focused_window);
    keybinding_create_no_args(LAlt, VK_V, taskbar_toggle);

    keybinding_create_no_args(LAlt, VK_B, toggle_create_window_in_current_workspace);
    keybinding_create_no_args(LAlt, VK_Z, toggle_ignore_workspace_filters);
    keybinding_create_no_args(LAlt, VK_F, client_stop_managing);

    keybinding_create_no_args(LAlt, VK_M, swap_selected_monitor_to_monacle_layout);
    keybinding_create_no_args(LAlt, VK_D, swap_selected_monitor_to_deck_layout);
    keybinding_create_no_args(LAlt, VK_T, swap_selected_monitor_to_tile_layout);
    keybinding_create_no_args(LAlt, VK_R, redraw_focused_window);

    keybinding_create_cmd_args(LWin, VK_T, start_app, terminalAppPath);
    keybinding_create_no_args(LAlt, VK_F9, quit);

    scratch_terminal_register(
            "cmd /c vifm",
            LAlt,
            VK_Q,
            L"4d237b71-4df7-4d2e-a60f-f9d7f69e80a7",
            terminal_with_uniqueStr_filter);

    scratch_terminal_register(
            "cmd /c powershell -nologo",
            LAlt | LShift,
            VK_W,
            L"ea3f98ca-973f-4154-9a7b-dc7a9377b768",
            terminal_with_uniqueStr_filter);

    scratch_terminal_register(
            "cmd /c nvim -c \"terminal powershell -nologo\"",
            LAlt,
            VK_W,
            L"643763f5-f5cd-416e-a5c9-bef1f516863c",
            terminal_with_uniqueStr_filter);

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
            \"\"%USERPROFILE%\\Utilites\"\"\" --loadCommand \"ld\"",
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
            --namedCommand \"procKill:cmd /c taskkill /f /pid {}\" --bind \"ctl-k:procKill\" --returnRange 46,8 \
            --namedCommand \"windbg:powershell.exe -C '{}' -match '\\d{8}';windbg -p \"\"\"$([int]$Matches[0])\"\"\"\" \
            --namedCommand \"sortPvtBytes:bin\\ListProcesses.exe --sort privateBytes\" \
            --namedCommand \"sortWrkSet:bin\\ListProcesses.exe --sort workingSet\" \
            --namedCommand \"sortCpu:bin\\ListProcesses.exe --sort cpu\" \
            --bindLoadCommand \"ctl-1:sortPvtBytes\" \
            --bindLoadCommand \"ctl-2:sortWrkSet\" \
            --bindLoadCommand \"ctl-3:sortCpu\" \
            --bind \"ctl-d:windbg\"",
            open_process_list_scratch_callback,
            LWin,
            VK_9,
            L"1a19cef1-1d1c-47b5-84c3-e5e2d1ffe141");
}

BOOL should_float_be_focused(Client *client)
{
    if(wcsstr(L"Microsoft Teams Notification", client->data->title))
    {
        return FALSE;
    }

    return TRUE;
}

BOOL should_always_exclude(Client* client)
{
    if(wcsstr(client->data->title, scratchWindowTitle))
    {
        return TRUE;
    }

    return FALSE;
}

BOOL is_float_window(Client *client, LONG_PTR styles, LONG_PTR exStyles)
{
    if(exStyles & WS_EX_APPWINDOW)
    {
        return FALSE;
    }

    if(exStyles & WS_EX_TOOLWINDOW)
    {
        return TRUE;
    }

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

    if(!(styles & WS_SIZEBOX))
    {
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
