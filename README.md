# SimpleWindowManager

## There are already two other tiling managers for Windows that are much better than this one.
- Workspacer
- Komorebi

(Most features and concepts are copied from these or DWM or I3)

## Overview
My general feeling having tried to a bunch of different tiling managers for Microsoft Windows, is that trying to layout every window that gets created by Windows is a endless battle against all of Windows's different intricasies.  So I created this window manager to target my specific workflow where 99% of the windows that I create come from a few different applications (browser, terminal, IDE, chat, email) and to create rules to manage those applications within workspaces and allow easy movement and navigation with common key bindings and to let any other windows that get created float.

## Instalation
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

Tile Layout

https://user-images.githubusercontent.com/811029/196359396-70a04129-12e9-4ff3-8ef4-2f8f8a10c90a.mp4

Monacle Layout

https://user-images.githubusercontent.com/811029/196359466-7dc2b2d1-75a7-4eb8-8fd7-f9d18356ecd9.mp4

Deck Layout

https://user-images.githubusercontent.com/811029/196359506-d070da87-9949-45de-a045-3a8254fdd4c0.mp4

Multiple Workspaces

https://user-images.githubusercontent.com/811029/196359562-a419c46f-ecd6-43f6-9deb-5f10eb2f231d.mp4

Scratch Windows

https://user-images.githubusercontent.com/811029/196359594-922ecd56-64d2-4d48-9a32-f8fac95cf837.mp4

Move windows between workspaces

https://user-images.githubusercontent.com/811029/196359630-66a98674-c604-4a3a-959c-f85b0aed9d8c.mp4

Multiple Monitors

https://user-images.githubusercontent.com/811029/196359680-c7d45b94-3edc-409f-97f8-715abe833925.mp4

## Build
* Copy the SampleConfig.c file to Config.c and make customizations to Config.c
* Make sure visual studio c++ build tools are installed (this shouldn't require a full VS install)
* Run setvcvars.bat (the path in the batch is hard coded to the VS install directory and may need to be updated)
* Run build.cmd

## Configuration
* Defining workspaces:
  * Specifying workspaces: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L129
  * Providing a filter function: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L26
* Key Bindings 
  * Simple key binding mapped to function with no args: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L196
  * Key binding mapped to function with single argument: https://github.com/e82eric/SimpleWindowManager/blob/main/SampleConfig.c#L188
