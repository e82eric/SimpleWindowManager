#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include "SimpleWindowManagerLib.h"

Layout deckLayout = {
    .select_next_window = deckLayout_select_next_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = deckLayout_move_client_next,
    .apply_to_workspace = deckLayout_apply_to_workspace,
    .next = NULL,
    .tag = L"D"
};

Layout monacleLayout = {
    .select_next_window = monacleLayout_select_next_client,
    .move_client_to_master = noop_move_client_to_master,
    .move_client_next = move_client_next,
    .apply_to_workspace = calc_new_sizes_for_monacle_workspace,
    .next = &deckLayout,
    .tag = L"M"
};

Layout tileLayout = {
    .select_next_window = stackBasedLayout_select_next_window,
    .move_client_to_master = tileLayout_move_client_to_master,
    .move_client_next = move_client_next,
    .apply_to_workspace = calc_new_sizes_for_workspace,
    .next = &monacleLayout,
    .tag = L"T"
};

Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout, int numberOfButtons)
{
    Button ** buttons = (Button **) calloc(numberOfButtons, sizeof(Button *));
    Workspace *result = calloc(1, sizeof(Workspace));
    result->name = _wcsdup(name);
    result->windowFilter = windowFilter;
    result->buttons = buttons;
    result->tag = _wcsdup(tag);
    result->layout = layout;
    return result;
}

KeyBinding* keybinding_create_no_args(int modifiers, unsigned int key, void (*action) (void))
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->action = action;
    return result;
}

KeyBinding* keybinding_create_cmd_args(int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *cmdArg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->cmdAction = action;
    result->cmdArg = cmdArg;
    return result;
}

KeyBinding* keybinding_create_workspace_arg(int modifiers, unsigned int key, void (*action) (Workspace*), Workspace *arg)
{
    KeyBinding *result = calloc(1, sizeof(KeyBinding));
    result->modifiers = modifiers;
    result->key = key;
    result->workspaceAction = action;
    result->workspaceArg = arg;
    return result;
}
