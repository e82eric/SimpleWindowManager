#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "fzf.h"
#include "SMenu.h"
#include "SimpleWindowManager.h"
#include "ListWindows.h"
#include "ListProcesses.h"
#include "ListServices.h"

MenuDefinition *searchDriveMenuDefinition;

void search_drive(char* stdOut)
{
    CHAR cmdBuf[MAX_PATH];
    sprintf_s(
            cmdBuf,
            MAX_PATH,
            "ld:cmd /c dir /s /b %s",
            stdOut);

    MenuDefinition_AddNamedCommand(searchDriveMenuDefinition, cmdBuf, FALSE, FALSE);
    MenuDefinition_ParseAndAddLoadCommand(searchDriveMenuDefinition, "ld", FALSE);
    searchDriveMenuDefinition->onSelection = open_program_scratch_callback_not_elevated;
    menu_run(searchDriveMenuDefinition);
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

    return TRUE;
}

BOOL should_client_use_hide(Client *client)
{
    //I think ConEmu has internal logic that detects that it has been moved out of visibile monitor space or hidden and moves it self back to visible space.  Minimize seems to work decent in these scenerios.  SSMS seems the same
    if(wcsstr(client->data->processImageName, L"ConEmu64.exe"))
    {
        return TRUE;
    }
    if(wcsstr(client->data->processImageName, L"ConEmu.exe"))
    {
        return TRUE;
    }
    if(wcsstr(client->data->processImageName, L"devenv.exe"))
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

void configure(Configuration *configuration)
{
    int modifiers = LAlt;
    keybindings_register_defaults_with_modifiers(modifiers);

    configuration_register_default_text_style(configuration, TEXT("JetBrainsMonoNL NFP Regular"), 12, 13);
    keybinding_create_with_shell_arg("NewTerminalWindow", LWin, VK_T, start_app, L"C:\\Users\\eric\\Utilites\\WezTerm\\wezterm.exe -e");

    WCHAR chromeTag = { 0xeb01 };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xeb0f };
    WCHAR teamsTag = { 0xf075 };

    Workspace *browserWorkspace = workspace_register(L"Chrome", &chromeTag, true, &deckLayout);
    workspace_register_processimagename_contains_filter(browserWorkspace, L"chrome.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"brave.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"firefox.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"msedge.exe");

    Workspace *terminalWorkspace = workspace_register(L"Terminal", &terminalTag, true, &deckLayout);
    workspace_register_processimagename_contains_filter(terminalWorkspace, L"WindowsTerminal.exe");

    Workspace *ideWorkspace = workspace_register(L"Rider", &riderTag, true, &monacleLayout);
    workspace_register_processimagename_contains_filter(ideWorkspace, L"rider64.exe");
    workspace_register_processimagename_contains_filter(ideWorkspace, L"Code.exe");
    workspace_register_processimagename_contains_filter(ideWorkspace, L"devenv.exe");

    Workspace *teamsWorkspace = workspace_register(L"Teams", &teamsTag, true, &deckLayout);
    workspace_register_processimagename_contains_filter(teamsWorkspace, L"Teams.exe");

    workspace_register(L"5", L"5", false, &tileLayout);
    workspace_register(L"6", L"6", false, &tileLayout);
    workspace_register(L"7", L"7", false, &tileLayout);
    workspace_register(L"8", L"8", false, &tileLayout);
    workspace_register(L"9", L"9", false, &tileLayout);
    workspace_register(L"Desktop", L"0", false, &deckLayout);

    ScratchWindow *powershellScratch = register_windows_terminal_scratch_with_unique_string(
            "Powershell",
            "powershell -nologo",
            L"643763f5-f5cd-416e-a5c9-bef1f516863d");
    keybinding_create_with_scratchwindow_arg("PowershellScratchWindow", modifiers | LShift, VK_F13, powershellScratch);

    register_list_windows_memu(modifiers, VK_SPACE);
    register_list_services_menu(modifiers, VK_F14);
    register_list_processes_menu(modifiers, VK_F15);
    register_keybindings_menu_with_modifiers(modifiers, VK_F17);

    char* notElevatedDirectories[] = {
        "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\"
    };
    register_program_launcher_menu(modifiers, VK_P, notElevatedDirectories, 3, FALSE);

    char* elevatedDirectories[] = {
        "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\"
    };
    register_program_launcher_menu(modifiers | LShift, VK_P, elevatedDirectories, 3, TRUE);

    searchDriveMenuDefinition = menu_create_and_register();
    MenuDefinition *searchAllDrivesMenu = menu_create_and_register();
    MenuDefinition_AddNamedCommand(searchAllDrivesMenu, "ld:powershell -c \"(Get-PSDrive -PSProvider FileSystem).Root\"", FALSE, FALSE);
    MenuDefinition_ParseAndAddLoadCommand(searchAllDrivesMenu, "ld", FALSE);
    searchAllDrivesMenu->onSelection = search_drive;
    keybinding_create_with_menu_arg("SearchAllDrivesMenu", modifiers, VK_F16, menu_run, searchAllDrivesMenu);

    TCHAR volumeIcon[2] = { 0xf028, '\0' };
    configuration_add_bar_segment(configuration, L" | ", false, 5, false, fill_system_time);
    configuration_add_bar_segment(configuration, L" | ", false, 5, false, fill_local_time);
    configuration_add_bar_segment(configuration, L" | ", false, 10, false, fill_local_date);
    configuration_add_bar_segment_with_header(configuration, L" | ", false, L"CPU:", false, 3, false, fill_cpu);
    configuration_add_bar_segment_with_header(configuration, L" | ", false, L"RAM:", false, 3, false, fill_memory_percent);
    configuration_add_bar_segment_with_header(configuration, L" | ", false, volumeIcon, true, 3, false, fill_volume_percent);
    configuration_add_bar_segment(configuration, L" | ", false, 1, true, fill_is_connected_to_internet);

    register_secondary_monitor_default_bindings_with_modifiers(modifiers, configuration->monitors[0], configuration->monitors[1], configuration->workspaces);
    keybindings_register_float_window_movements(LWin);
}
