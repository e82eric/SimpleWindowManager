#include <assert.h>
#include <windows.h>
#include <stdio.h>
#include <strsafe.h>
#include "fzf\\fzf.h"
#include "SMenu.h"
#include <assert.h>
#include <synchapi.h>
#include <commctrl.h>
#include <conio.h>

MenuView *mView;

void onEscape(void)
{
    PostQuitMessage(0);
}

void onSelection(char *stdOut)
{
    printf("%s", stdOut);
    ShowWindow(mView->hwnd, SW_HIDE);

    _getch();

    MenuDefinition *menuDefinition2 = calloc(1, sizeof(MenuDefinition));
    menuDefinition2->onEscape = onEscape;
    menuDefinition2->onSelection = onSelection;
    MenuDefinition_AddNamedCommand(menuDefinition2, "ld:fd . c:\\", FALSE, TRUE);
    MenuDefinition_ParseAndAddLoadCommand(menuDefinition2, "ld");
    menu_run_definition(mView, menuDefinition2);
}

int main(int argc, char* argv[])
{
    SetProcessDPIAware();
    int top = 250;
    int left = 250;
    int height = 940;
    int width = 2050;
    TCHAR title[BUF_LEN] = L"Blah";

    mView = menu_create_with_size(top, left, width, height, title);
    MenuDefinition *menuDefinition = menu_definition_create(mView);
    menuDefinition->onSelection = onSelection;
    menuDefinition->onEscape = onEscape;

    for(int i = 0; i < argc; i++)
    {
        if(strcmp(argv[i], "--loadCommand") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndAddLoadCommand(menuDefinition, argv[i]);
            }
        }
        if(strcmp(argv[i], "--bind") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndAddKeyBinding(menuDefinition, argv[i], FALSE);
            }
        }
        if(strcmp(argv[i], "--bindNoReload") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndAddKeyBinding(menuDefinition, argv[i], FALSE);
            }
        }
        if(strcmp(argv[i], "--bindLoadCommand") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndAddKeyBinding(menuDefinition, argv[i], TRUE);
            }
        }
        if(strcmp(argv[i], "--bindExecuteAndQuit") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndAddKeyBinding(menuDefinition, argv[i], FALSE);
            }
        }
        if(strcmp(argv[i], "--namedCommand") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_AddNamedCommand(menuDefinition, argv[i], FALSE, TRUE);
            }
        }
        if(strcmp(argv[i], "--returnRange") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                MenuDefinition_ParseAndSetRange(menuDefinition, argv[i]);
            }
        }
        if(strcmp(argv[i], "--hasHeader") == 0)
        {
            mView->itemsView->hasHeader = TRUE;
        }
    }

    menu_run_definition(mView, menuDefinition);

    MSG Msg;
    while(GetMessage(&Msg, NULL, 0, 0) > 0)
    {
        TranslateMessage(&Msg);
        DispatchMessage(&Msg);
    }

    return 0;
}
