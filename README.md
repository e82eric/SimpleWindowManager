# SimpleWindowManager

## There are already two other tiling managers for Windows that are much better than this one.
- Workspacer
- Komorebi

(Most features and concepts are copied from these or DWM or I3)

## Overview
I wrote this application to target my specific workflow where 90% of windows come from a few different applications (browser, terminal IDE, chat, email) and I want to have those windows auto layed out for me into tiles and workspaces so that I can quickly navigate between them using the keyboard.  All other windows I let float.

## Install
- You will need to compile the application to install it
  - Make sure that MSVC (C compiler) is installed
  - Run setvccars.bar to add nmake and cl.exe to the path.  (You make need to adjust the paths in the batch depending one where MSVC is installed)
  - Make any configuration/customizations
  - Run nmake
  - Run nmake publish
  - Run bin\SimpleWindowManager.exe (to run the application)

## Configuration/Customization
- Configuration is done in C and is compiled into the binary
- You will need to a Config.c
  - You will need to implement a void configure(Configuration *configuration) function
    - This is where you can adjust bar color/bar height etc on the configuration structure
    - This is also where you will define workspaces, keybindings, scratch windows and menu commands

There is a SampleConfig.c provided in the repository
- The makefile currently looks for this in ..\SimpleWindowManagerConfig\config.c but can be updated in the make file
## Features
- Workspaces
  - Define up to 9 different workspaces along with rules about which windows will be managed by the workspace.  (Usually defined by the process name, window class, window title or any combination of those).  Windows matching these rules will automatically be layed out by the workspace in its current layout
- Layouts
  - Tile Layout
  - Monacle
  - Deck (hybrid of Tile and Monacle where the master position is fixed and the windows in the secondary position can be cycled through) 
- Scratch Windows
  - Simple short lived terminal application mapped to key bindings.
- Scratch Menus
  - A really simple generic menu/filter is included that works similar to fzf and can be bound to keybindings
- KeyBindings
  - Map keybindings to arbitrary commands

## Some things to note
- Workspace rules and definitions are set in Config.c

## Key Bindings
| Key Binding   | Action              |
| ------------- | -------------       |
| ALT+J         | Select Next Window  |
| ALT+K         | Select Previous Window        |
| ALT+Enter     | Move selected window to master |
| ALT+Shift+J   | Move window to next postion |
| ALT+Shift+K   | Move window to previous position |
| ALT+L         | Increase width of master window |
| ALT+H         | Decrease width of master window |
| ALT+T         | Switch to tile layout |
| ALT+D         | Switch to deck layout |
| ALT+M         | Switch to monacle layout |
| ALT+Space     | Toggle next layout |
| ALT+Shirt+C   | Close selected Window |
| ALT+,         | Select next monitor |
| ALT+[1-9] | Select workspace by number|
| ALT+Shift+[1-9] | Move selected window to workspace by number |
| ALT+V         | Toggle Windows Taskbar |
| ALT+A         | Toggle mode where all windows that match a filter are added to the current workspace |
| ALT+Z         | Toggle mode where all windows are added to current workspace |
| ALT+X         | Toggle menu to view all available hotkeys |

## Tile Layout

https://user-images.githubusercontent.com/811029/196359396-70a04129-12e9-4ff3-8ef4-2f8f8a10c90a.mp4

## Monacle Layout

https://user-images.githubusercontent.com/811029/196359466-7dc2b2d1-75a7-4eb8-8fd7-f9d18356ecd9.mp4

## Deck Layout

https://user-images.githubusercontent.com/811029/196359506-d070da87-9949-45de-a045-3a8254fdd4c0.mp4

### Multiple Workspaces

https://user-images.githubusercontent.com/811029/196359562-a419c46f-ecd6-43f6-9deb-5f10eb2f231d.mp4

## Menus
Below is an example workflow using the Programs Menu (Alt-P) to open VS Code and then using the same menu to open notepad, then  the List Processes Menu (Super-9) to kill the notepad process, and then the List Windows Menu (Alt-Space) to find the NeoVim window switch to that workspace.  The definitions for these windows are included in the sample config,  adding custom workflow using menus is intended to be easy using shell commands or custom c functions.

https://user-images.githubusercontent.com/811029/218302288-3829d11d-67af-411c-8c8d-f174193583b9.mp4

## Scratch Windows
Below is an example workflow using a scratch terminal defined as the NeoVim terminal with powershell running inside of it to quickly toggle the terminal generate a new guid and copy it into a editor

https://user-images.githubusercontent.com/811029/218302270-1b8a65aa-1bd2-44a8-b023-62557ad45c55.mp4

## Moving windows between workspaces

https://user-images.githubusercontent.com/811029/196359630-66a98674-c604-4a3a-959c-f85b0aed9d8c.mp4

## Multiple Monitors

https://user-images.githubusercontent.com/811029/196359680-c7d45b94-3edc-409f-97f8-715abe833925.mp4

## Configuration
### Defining workspaces:
* Specifying workspaces: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L77
* Can be configured with simple contains matching using the exe name, windows title, or window class.
* It is also possible to use a C function to do advanced filtering using the Client struct

_Example Workspace defined using exe name filters_
```C
    WCHAR chromeTag = { 0xfa9e };

    Workspace *browserWorkspace = workspace_register(L"Chrome", &chromeTag, &tileLayout);
    workspace_register_processimagename_contains_filter(browserWorkspace, L"chrome.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"brave.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"firefox.exe");
    workspace_register_processimagename_contains_filter(browserWorkspace, L"msedge.exe");
```

### Key Bindings
* A number of keybindings are registered by default: https://github.com/e82eric/SimpleWindowManager/blob/main/SimpleWindowManager.c#L3966
  * These can be overridden by registering a keybinding using the same modifier key combination
* Simple key binding mapped to function with no args: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L196
* Key binding mapped to function with single argument: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L188

### Menus
* Both menu items and OnSelection actions, can be provided by the stdout of a shell command or by a custom C function.

_Example Menu items defined with shell command_
```C
    MenuDefinition *programLauncher = menu_create_and_register();
    MenuDefinition_AddNamedCommand(programLauncher, "ld:cmd /c fd -t f -g \"*{.lnk,.exe}\" \
            \"%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\" \
            \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\" \
            \"%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\" \
            \"%USERPROFILE%\\Utilites\"",
            FALSE,
            FALSE);
    MenuDefinition_ParseAndAddLoadCommand(programLauncher, "ld");
    programLauncher->onSelection = open_program_scratch_callback;
    keybinding_create_with_menu_arg("ProgramLauncherNotElevatedMenu", LAlt | LShift, VK_P, menu_run, programLauncher);
```

_Example Menu items defined with C function_
```C
    MenuDefinition *listProcessMenu = menu_create_and_register();
    menu_definition_set_load_action(listProcessMenu, list_processes_run_no_sort);
    MenuDefinition_AddNamedCommand(listProcessMenu, "procKill:cmd /c taskkill /f /pid {}", TRUE, FALSE);
    MenuDefinition_AddNamedCommand(listProcessMenu, "windbg:powershell.exe -C '{}' -match '\\d{8}';windbg -p \"\"\"$([int]$Matches[0])\"\"\"", FALSE, TRUE);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_1, list_processes_run_sorted_by_private_bytes);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_2, list_processes_run_sorted_by_working_set);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_3, list_processes_run_sorted_by_cpu);
    MenuDefinition_AddLoadActionKeyBinding(listProcessMenu, VK_CONTROL, VK_4, list_processes_run_sorted_by_pid);
    MenuDefinition_ParseAndSetRange(listProcessMenu, "46,8");
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-k:procKill", FALSE);
    MenuDefinition_ParseAndAddKeyBinding(listProcessMenu, "ctl-d:windbg", FALSE);
    listProcessMenu->hasHeader = TRUE;
    keybinding_create_with_menu_arg("ProcessListMenu", LWin, VK_9, menu_run, listProcessMenu);
```

* OnSelection action:
  * Shell: 
    * C function: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L150
    * Custom actions can also be defined using shell commands: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L156

### Scratch Terminals

_Example scratch terminal to toggle neovim terminal with powershell running in it_
```C
    ScratchWindow *nPowershellScratch = register_scratch_terminal_with_unique_string(
            "Nvim Powershell",
            "cmd /c nvim -c \"terminal powershell -nologo\"",
            L"643763f5-f5cd-416e-a5c9-bef1f516863c");
    keybinding_create_with_scratchwindow_arg("NvimPowershellScratchWindow", LAlt, VK_W, nPowershellScratch);
```

### Bar segments:
* These are defined using header text: width of variable part of segment: C function for getting the variable text:
  
_Example bar segment definitions_
```C
    configuration_add_bar_segment(configuration, L"UTC", 5, fill_system_time);
    configuration_add_bar_segment(configuration, L"Time", 16, fill_local_time);
    configuration_add_bar_segment(configuration, L"CPU", 6, fill_cpu);
    configuration_add_bar_segment(configuration, L"Memory", 6, fill_memory_percent);
    configuration_add_bar_segment(configuration, L"Volume", 6, fill_volume_percent);
    configuration_add_bar_segment(configuration, L"Internet", 3, fill_is_connected_to_internet);
```
