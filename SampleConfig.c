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
            "ld:cmd /c fd . %s\\",
            stdOut);

    MenuDefinition_AddNamedCommand(searchDriveMenuDefinition, cmdBuf, FALSE, FALSE);
    MenuDefinition_ParseAndAddLoadCommand(searchDriveMenuDefinition, "ld");
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
    int modifiers = LAlt | LWin | LCtl;
    keybindings_register_defaults_with_modifiers(modifiers);

    configuration->windowsThatShouldNotFloatFunc = is_float_window_from_config;
    configuration->useOldMoveLogicFunc = should_use_old_move_logic;
    configuration->windowRoutingMode = FilteredRoutedNonFilteredCurrentWorkspace;
    configuration->clientShouldUseMinimizeToHide = should_client_use_hide;
    configuration->alwaysRedraw = TRUE;
    configuration->floatUwpWindows = FALSE;

    WCHAR chromeTag = { 0xfa9e };
    WCHAR terminalTag = { 0xf120 };
    WCHAR riderTag = { 0xf668 };
    WCHAR teamsTag = { 0xf865 };

    Workspace *browserWorkspace = workspace_register(L"Chrome", &chromeTag, &deckLayout);
    workspace_register_processimagename_contains_filter(browserWorkspace, L"chrome.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"brave.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"firefox.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"msedge.exe");

    Workspace *terminalWorkspace = workspace_register(L"Terminal", &terminalTag, &deckLayout);
    workspace_register_processimagename_contains_filter(terminalWorkspace, L"WindowsTerminal.exe");

    Workspace *ideWorkspace = workspace_register(L"Rider", &riderTag, &monacleLayout);
    workspace_register_processimagename_contains_filter(ideWorkspace, L"rider64.exe");
    workspace_register_processimagename_contains_filter(ideWorkspace, L"Code.exe");

    Workspace *teamsWorkspace = workspace_register(L"Teams", &teamsTag, &deckLayout);
    workspace_register_processimagename_contains_filter(teamsWorkspace, L"Teams.exe");

    workspace_register(L"5", L"5", &tileLayout);
    workspace_register(L"6", L"6", &tileLayout);
    workspace_register(L"7", L"7", &tileLayout);
    workspace_register(L"8", L"8", &tileLayout);
    workspace_register(L"9", L"9", &tileLayout);
    workspace_register(L"Desktop", L"0", &deckLayout);

    ScratchWindow *nPowershellScratch = register_scratch_terminal_with_unique_string(
            "Nvim Powershell",
            "cmd /c nvim -c \"terminal powershell -nologo\"",
            L"643763f5-f5cd-416e-a5c9-bef1f516863c");
    keybinding_create_with_scratchwindow_arg("NvimPowershellScratchWindow", modifiers, VK_F13, nPowershellScratch);

    register_list_windows_memu(modifiers, VK_SPACE);
    register_list_services_menu(modifiers, VK_F14);
    register_list_processes_menu(modifiers, VK_F15);
    register_keybindings_menu_with_modifiers(modifiers, VK_F17);

    char* notElevatedDirectories[] = {
        "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\",
        "%USERPROFILE%\\Utilites\\"
    };
    register_program_launcher_menu(modifiers, VK_P, notElevatedDirectories, 4, FALSE);

    char* elevatedDirectories[] = {
        "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\",
        "C:\\Program Files\\sysinternals\\",
        "%USERPROFILE%\\Utilites\\"
    };
    register_program_launcher_menu(modifiers | LShift, VK_P, elevatedDirectories, 5, TRUE);

    searchDriveMenuDefinition = menu_create_and_register();
    MenuDefinition *searchAllDrivesMenu = menu_create_and_register();
    MenuDefinition_AddNamedCommand(searchAllDrivesMenu, "ld:powershell -c \"(Get-PSDrive -PSProvider FileSystem).Root\"", FALSE, FALSE);
    MenuDefinition_ParseAndAddLoadCommand(searchAllDrivesMenu, "ld");
    searchAllDrivesMenu->onSelection = search_drive;
    keybinding_create_with_menu_arg("SearchAllDrivesMenu", modifiers, VK_F16, menu_run, searchAllDrivesMenu);

    configuration_add_bar_segment(configuration, L"UTC", 5, fill_system_time);
    configuration_add_bar_segment(configuration, L"Time", 16, fill_local_time);
    configuration_add_bar_segment(configuration, L"CPU", 6, fill_cpu);
    configuration_add_bar_segment(configuration, L"Memory", 6, fill_memory_percent);
    configuration_add_bar_segment(configuration, L"Volume", 7, fill_volume_percent);
    configuration_add_bar_segment(configuration, L"Internet", 3, fill_is_connected_to_internet);

    register_secondary_monitor_default_bindings_with_modifiers(modifiers, configuration->monitors[0], configuration->monitors[1], configuration->workspaces);
}
