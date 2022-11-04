#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "SimpleWindowManager.h"

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
    if(wcsstr(client->data->processImageName, L"WindowsTerminal.exe"))
    {
        return TRUE;
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

Workspace** create_workspaces(int* outSize)
{
    WCHAR chromeTag = { 0xfa9e };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xf668 };
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

TCHAR *cmdScratchCmd = L"cmd /c powershell -NoLogo";
TCHAR *launcherCommand = L"cmd /c for /f \"delims=\" %i in ('fd -t f -g \"*{.lnk,.exe}\" \"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\" \"%USERPROFILE%\\Utilites\" ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
TCHAR *allFilesCommand = L"cmd /c set /p \"pth=Enter Path: \" && for /f \"delims=\" %i in ('fd . -t f %^pth% ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
TCHAR *terminalAppPath = L"wt.exe";
TCHAR *processListCommand1 = L"cmd /c powershell -NoLogo -NoProfile -c . .\\processlist.ps1;Run";

void create_keybindings(Workspace** workspaces)
{
    keybinding_create_no_args(LAlt, VK_J, select_next_window);
    keybinding_create_no_args(LAlt, VK_OEM_COMMA, monitor_select_next);
    keybinding_create_no_args(LAlt, VK_N, arrange_clients_in_selected_workspace);
    keybinding_create_no_args(LAlt, VK_L, workspace_increase_master_width_selected_monitor);
    keybinding_create_no_args(LAlt, VK_H, workspace_decrease_master_width_selected_monitor);
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

    keybinding_create_cmd_args(LAlt, VK_S, start_scratch_not_elevated, allFilesCommand);
    keybinding_create_cmd_args(LShift | LAlt, VK_S, start_launcher, allFilesCommand);
    keybinding_create_cmd_args(LWin, VK_0, start_scratch_not_elevated, cmdScratchCmd);
    keybinding_create_cmd_args(LShift | LWin, VK_0, start_launcher, cmdScratchCmd);
    keybinding_create_cmd_args(LShift | LAlt, VK_P, start_launcher, launcherCommand);
    keybinding_create_cmd_args(LAlt, VK_P, start_scratch_not_elevated, launcherCommand);
    keybinding_create_cmd_args(LWin, VK_9, start_launcher, processListCommand1);
    keybinding_create_no_args(LAlt, VK_SPACE, open_windows_scratch_start);

    keybinding_create_no_args(LAlt, VK_F9, quit);
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
