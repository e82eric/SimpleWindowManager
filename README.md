# SimpleWindowManager

## There are already two other tiling managers for Windows that are much better than this one.
- Workspacer
- Komorebi

(Most features and concepts are copied from these or DWM or I3)

## Overview
My general feeling having tried to a bunch of different tiling managers for Microsoft Windows, is that trying to layout every window that gets created by Windows would be a endless battle against all of Windows's different intricasies.  So I created this window manager to target my specific workflow where 99% of the windows that I create come from a few different applications (browser, terminal, IDE, chat, email) and to create rules to manage those applications within workspaces and allow easy movement and navigation with common key bindings and to let any other windows that get created float.

## Features
- Workspaces
  - Define up to 9 different workspaces along with rules about which windows will be managed by the workspace.  (Usually defined by the process name, window class, window title or any combination of those).  Windows matching these rules will automatically be layed out by the workspace in its current layout
- Layouts
  - Tile Layout
  - Monacle
  - Deck (hybrid of Tile and Monacle where the master position is fixed and the windows in the secondary position can be cycled through) 
- Scratch Windows
  - Simple short lived terminal application that mapped to key bindings.  (My 2 most common are fzf commands for launching applications and killing processes)

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
| ALT+N         | Decrease width of master window |
| ALT+T         | Switch to tile layout |
| ALT+D         | Switch to deck layout |
| ALT+M         | Switch to monacle layout |
| ALT+Shirt+C   | Close selected Window |
| ALT+,         | Select next monitor |
| ALT+[1-9] | Select workspace by number|
| ALT+Shift+[1-9] | Move selected window to workspace by number |

Tile Layout

https://user-images.githubusercontent.com/811029/196101803-4f1824ca-a77f-4306-a97d-73b2794822a7.mp4


https://user-images.githubusercontent.com/811029/196353537-eb57a993-24aa-40d6-8d17-26221274cfd2.mp4


Monacle Layout

https://user-images.githubusercontent.com/811029/196101840-a48820c8-f158-4788-a513-70bb665927ed.mp4

Deck Layout

https://user-images.githubusercontent.com/811029/196101869-2a506130-ea4a-4159-85f0-5cbfe259f3f0.mp4

Multiple Workspaces

https://user-images.githubusercontent.com/811029/196101917-733c3f70-f0f7-414d-a44e-b5c73642843b.mp4

Scratch Windows

https://user-images.githubusercontent.com/811029/196101958-6297be42-c870-4269-b4fd-d58743276d25.mp4

Move windows between workspaces

https://user-images.githubusercontent.com/811029/196103144-62bddc4f-9f5c-4340-b5c7-4acb7d74180f.mp4

Multiple Monitors

https://user-images.githubusercontent.com/811029/196115233-af8056eb-74b2-4d89-b336-377c0d2a4def.mp4

