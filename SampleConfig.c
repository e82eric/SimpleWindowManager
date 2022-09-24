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
        if(wcsstr(client->data->className, L"Chrome_WidgetWin_2"))
        {
            return FALSE;
        }
        if(
                wcsstr(client->data->className, L"MozillaDropShadowWindowClass")
                /* || wcsstr(client->data->className, L"MozillaHiddenWindowClass") */
                || wcsstr(client->data->className, L"MozillaDialogClass")
                /* || wcsstr(client->data->className, L"MozillaTempWindowClass") */
                /* || wcsstr(client->data->className, L"MozillaTransitionWindowClass") */
            )
        {
            return FALSE;
        }
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

BOOL teams_workspace(Client *client)
{
    if(wcsstr(client->data->title, L"toolbar"))
    {
        return FALSE;
    }
    if(wcsstr(client->data->title, L"Microsoft Teams Notification"))
    {
        return FALSE;
    }
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

    int* numberOfWorkspaces = malloc(sizeof(int));
    *numberOfWorkspaces = 9;

    Workspace** workspaces = calloc(*numberOfWorkspaces, sizeof(Workspace *));
    workspaces[0] = workspace_create(L"Chrome", chrome_workspace_filter, &chromeTag, &tileLayout, numberOfBars);
    workspaces[1] = workspace_create(L"Terminal", terminal_workspace_filter, &terminalTag, &deckLayout, numberOfBars);
    workspaces[2] = workspace_create(L"Rider", rider_workspace_filter, &riderTag, &monacleLayout, numberOfBars);
    workspaces[3] = workspace_create(L"4", null_filter, L"4", &tileLayout, numberOfBars);
    workspaces[4] = workspace_create(L"5", null_filter, L"5", &tileLayout, numberOfBars);
    workspaces[5] = workspace_create(L"6", null_filter, L"6", &tileLayout, numberOfBars);
    workspaces[6] = workspace_create(L"7", null_filter, L"7", &tileLayout, numberOfBars);
    workspaces[7] = workspace_create(L"8", null_filter, L"8", &tileLayout, numberOfBars);
    workspaces[8] = workspace_create(L"9", null_filter, L"9", &tileLayout, numberOfBars);

    *outSize = *numberOfWorkspaces;
    return workspaces;
}

TCHAR *cmdScratchCmd = L"cmd /k";
TCHAR *launcherCommand = L"cmd /c for /f \"delims=\" %i in ('fd -t f -g \"*{.lnk,.exe}\" \"%USERPROFILE\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \"C:\\Users\\eric\\AppData\\Local\\Microsoft\\WindowsApps\" ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
TCHAR *allFilesCommand = L"cmd /c set /p \"pth=Enter Path: \" && for /f \"delims=\" %i in ('fd . -t f %^pth% ^| fzf --bind=\"ctrl-c:execute(echo {} | clip)\"') do start \" \" \"%i\"";
TCHAR *processListCommand = L"/c tasklist /nh | sort | fzf -e --bind=\"ctrl-k:execute(for /f \\\"tokens=2\\\" %f in ({}) do taskkill /f /pid %f)+reload(tasklist /nh | sort)\" --bind=\"ctrl-r:reload(tasklist /nh | sort)\" --header \"ctrl-k Kill Pid\" --reverse";

KeyBinding** create_keybindings(int* outSize, Workspace** workspaces)
{
    int* numberOfKeyBindings = malloc(sizeof(int));
    *numberOfKeyBindings = 39;

    KeyBinding** keyBindings = calloc(*numberOfKeyBindings, sizeof(KeyBinding*));
    keyBindings[0] = keybinding_create_no_args(LAlt, VK_J, select_next_window);
    keyBindings[1] = keybinding_create_no_args(LAlt, VK_OEM_COMMA, monitor_select_next);
    keyBindings[2] = keybinding_create_cmd_args(LShift | LAlt, VK_P, start_launcher, launcherCommand);
    keyBindings[3] = keybinding_create_cmd_args(LAlt, VK_P, start_scratch_not_elevated, launcherCommand);
    keyBindings[4] = keybinding_create_cmd_args(LAlt, VK_K, start_launcher, processListCommand);
    keyBindings[5] = keybinding_create_no_args(LAlt, VK_SPACE, toggle_selected_monitor_layout);
    keyBindings[6] = keybinding_create_no_args(LAlt, VK_N, arrange_clients_in_selected_workspace);
    keyBindings[7] = keybinding_create_no_args(LAlt, VK_L, workspace_increase_master_width_selected_monitor);
    keyBindings[8] = keybinding_create_no_args(LAlt, VK_H, workspace_decrease_master_width_selected_monitor);
    keyBindings[9] = keybinding_create_no_args(LAlt, VK_RETURN, move_focused_window_to_master);

    keyBindings[10] = keybinding_create_workspace_arg(LAlt, VK_1, swap_selected_monitor_to, workspaces[0]);
    keyBindings[11] = keybinding_create_workspace_arg(LAlt, VK_2, swap_selected_monitor_to, workspaces[1]);
    keyBindings[12] = keybinding_create_workspace_arg(LAlt, VK_3, swap_selected_monitor_to, workspaces[2]);
    keyBindings[13] = keybinding_create_workspace_arg(LAlt, VK_4, swap_selected_monitor_to, workspaces[3]);
    keyBindings[14] = keybinding_create_workspace_arg(LAlt, VK_5, swap_selected_monitor_to, workspaces[4]);
    keyBindings[15] = keybinding_create_workspace_arg(LAlt, VK_6, swap_selected_monitor_to, workspaces[5]);
    keyBindings[16] = keybinding_create_workspace_arg(LAlt, VK_7, swap_selected_monitor_to, workspaces[6]);
    keyBindings[17] = keybinding_create_workspace_arg(LAlt, VK_8, swap_selected_monitor_to, workspaces[7]);
    keyBindings[18] = keybinding_create_workspace_arg(LAlt, VK_9, swap_selected_monitor_to, workspaces[8]);

    keyBindings[19] = keybinding_create_no_args(LShift | LAlt, VK_J, move_focused_client_next);

    keyBindings[20] = keybinding_create_workspace_arg(LShift | LAlt, VK_1, move_focused_window_to_workspace, workspaces[0]);
    keyBindings[21] = keybinding_create_workspace_arg(LShift | LAlt, VK_2, move_focused_window_to_workspace, workspaces[1]);
    keyBindings[22] = keybinding_create_workspace_arg(LShift | LAlt, VK_3, move_focused_window_to_workspace, workspaces[2]);
    keyBindings[23] = keybinding_create_workspace_arg(LShift | LAlt, VK_4, move_focused_window_to_workspace, workspaces[3]);
    keyBindings[24] = keybinding_create_workspace_arg(LShift | LAlt, VK_5, move_focused_window_to_workspace, workspaces[4]);
    keyBindings[25] = keybinding_create_workspace_arg(LShift | LAlt, VK_6, move_focused_window_to_workspace, workspaces[5]);
    keyBindings[26] = keybinding_create_workspace_arg(LShift | LAlt, VK_7, move_focused_window_to_workspace, workspaces[6]);
    keyBindings[27] = keybinding_create_workspace_arg(LShift | LAlt, VK_8, move_focused_window_to_workspace, workspaces[7]);
    keyBindings[28] = keybinding_create_workspace_arg(LShift | LAlt, VK_9, move_focused_window_to_workspace, workspaces[8]);

    keyBindings[29] = keybinding_create_no_args(LShift | LAlt, VK_C, close_focused_window);
    keyBindings[30] = keybinding_create_no_args(LShift | LAlt, VK_C, kill_focused_window);
    keyBindings[31] = keybinding_create_cmd_args(LAlt, VK_S, start_scratch_not_elevated, allFilesCommand);
    keyBindings[32] = keybinding_create_cmd_args(LShift | LAlt, VK_S, start_launcher, allFilesCommand);
    keyBindings[33] = keybinding_create_cmd_args(LWin, VK_1, start_scratch_not_elevated, cmdScratchCmd);
    keyBindings[34] = keybinding_create_cmd_args(LShift | LWin, VK_1, start_launcher, cmdScratchCmd);
    keyBindings[35] = keybinding_create_no_args(LAlt, VK_V, taskbar_toggle);

    keyBindings[36] = keybinding_create_no_args(LAlt, VK_M, swap_selected_monitor_to_monacle_layout);
    keyBindings[37] = keybinding_create_no_args(LAlt, VK_D, swap_selected_monitor_to_deck_layout);
    keyBindings[38] = keybinding_create_no_args(LAlt, VK_T, swap_selected_monitor_to_tile_layout);

    *outSize = *numberOfKeyBindings;
    return keyBindings;
}

BOOL should_use_old_move_logic(Client* client)
{
    if(wcsstr(client->data->processImageName, L"Teams.exe"))
    {
        return TRUE;
    }
    return FALSE;
}