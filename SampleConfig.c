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
    int modifiers = LAlt;
    keybindings_register_defaults_with_modifiers(modifiers);

    configuration->isFloatWindowFunc = is_float_window_from_config;
    configuration->shouldFloatBeFocusedFunc = should_float_be_focused;
    configuration->useOldMoveLogicFunc = should_use_old_move_logic;
    /* configuration->windowRoutingMode = FilteredRoutedNonFilteredCurrentWorkspace; */
    configuration->alwaysRedraw = TRUE;

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

    workspace_register(L"5", L"4", &tileLayout);
    workspace_register(L"6", L"6", &tileLayout);
    workspace_register(L"7", L"7", &tileLayout);
    workspace_register(L"8", L"8", &tileLayout);
    workspace_register(L"9", L"9", &tileLayout);

    workspace_register(L"Desktop", L"0", &deckLayout);

    register_default_scratch_windows();

    ScratchWindow *vifmScratch = register_scratch_terminal_with_unique_string(
            "VIFM",
            "cmd /c vifm",
            L"4d237b71-4df7-4d2e-a60f-f9d7f69e80a7");

    ScratchWindow *powershellScratch = register_scratch_terminal_with_unique_string(
            "Powershell",
            "cmd /c powershell -nologo",
            L"ea3f98ca-973f-4154-9a7b-dc7a9377b768");

    ScratchWindow *nPowershellScratch = register_scratch_terminal_with_unique_string(
            "Nvim Powershell",
            "cmd /c nvim -c \"terminal powershell -nologo\"",
            L"643763f5-f5cd-416e-a5c9-bef1f516863c");
    keybinding_create_with_scratchwindow_arg("NvimPowershellScratchWindow", modifiers, VK_F13, nPowershellScratch);

    MenuDefinition *listWindowsMenu = menu_create_and_register();
    listWindowsMenu->hasHeader = TRUE;
    menu_definition_set_load_action(listWindowsMenu, list_windows_run);
    listWindowsMenu->onSelection = open_windows_scratch_exit_callback;
    keybinding_create_with_menu_arg("ListWindowsMenu", modifiers, VK_SPACE, menu_run, listWindowsMenu);

    MenuDefinition *listServicesMenu = menu_create_and_register();
    listServicesMenu->hasHeader = TRUE;
    menu_definition_set_load_action(listServicesMenu, list_services_run_no_sort);
    NamedCommand *serviceStartCommand = MenuDefinition_AddNamedCommand(listServicesMenu, "start:sc start \"\"{}\"\"", TRUE, FALSE);
    NamedCommand *serviceStopCommand = MenuDefinition_AddNamedCommand(listServicesMenu, "stop:sc stop \"\"{}\"\"", TRUE, FALSE);
    NamedCommand_SetTextRange(serviceStartCommand, 0, 45, TRUE);
    NamedCommand_SetTextRange(serviceStopCommand, 0, 45, TRUE);
    MenuDefinition_ParseAndAddKeyBinding(listServicesMenu, "ctl-s:start", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listServicesMenu, "ctl-x:stop", FALSE);
    keybinding_create_with_menu_arg("ListServicesMenu", modifiers, VK_F14, menu_run, listServicesMenu);

    MenuDefinition *programLauncher = menu_create_and_register();
    MenuDefinition_AddNamedCommand(programLauncher, "ld:cmd /c fd -t f -g \"*{.lnk,.exe}\" \
            \"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \
            \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \
            \"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\" \
            \"C:\\Program Files\\sysinternals\" \
            \"%USERPROFILE%\\Utilites\"",
            FALSE,
            FALSE);
    MenuDefinition_ParseAndAddLoadCommand(programLauncher, "ld");
    programLauncher->onSelection = open_program_scratch_callback;
    keybinding_create_with_menu_arg("ProgramLauncherNotElevatedMenu", modifiers, VK_P, menu_run, programLauncher);

    MenuDefinition *programLauncherNotElevatedMenu = menu_create_and_register();
    MenuDefinition_AddNamedCommand(programLauncherNotElevatedMenu, "ld:cmd /c fd -t f -g \"*{.lnk,.exe}\" \
            \"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \
            \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \
            \"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\" \
            \"%USERPROFILE%\\Utilites\"",
            FALSE,
            FALSE);
    MenuDefinition_ParseAndAddLoadCommand(programLauncherNotElevatedMenu, "ld");
    programLauncherNotElevatedMenu->onSelection = open_program_scratch_callback_not_elevated;
    keybinding_create_with_menu_arg("ProgramLauncherNotElevatedMenu", modifiers, VK_P, menu_run, programLauncherNotElevatedMenu);

    MenuDefinition *listProcessMenu = menu_create_and_register();
    menu_definition_set_load_action(listProcessMenu, list_processes_run_no_sort);
    NamedCommand *killProcessCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "procKill:cmd /c taskkill /f /pid {}", TRUE, FALSE);
    NamedCommand_SetTextRange(killProcessCommand, 76, 8, TRUE);
    NamedCommand *windbgCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "windbg:windbgx -p {}", FALSE, TRUE);
    NamedCommand_SetTextRange(windbgCommand, 76, 8, TRUE);
    NamedCommand *procDumpNamedCommand = MenuDefinition_AddNamedCommand(listProcessMenu, "procdump:cmd /c procdump -accepteula -ma {} %USERPROFILE%\\memory_dumps", FALSE, FALSE);
    NamedCommand_SetTextRange(procDumpNamedCommand, 76, 8, TRUE);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_1, list_processes_run_sorted_by_private_bytes, "Sort Pvt Bytes");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_2, list_processes_run_sorted_by_working_set, "Sort Wrk Set");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_3, list_processes_run_sorted_by_cpu, "Sort Cpu");
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_4, list_processes_run_sorted_by_pid, "Sort Pid");
    MenuDefinition_ParseAndSetRange(listProcessMenu, "76,8");
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-k:procKill", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-d:windbg", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-x:procdump", FALSE);
    listProcessMenu->hasHeader = TRUE;
    keybinding_create_with_menu_arg("ProcessListMenu", modifiers, VK_F15, menu_run, listProcessMenu);

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
