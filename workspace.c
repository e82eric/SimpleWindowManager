#include <windows.h>

#include "fzf\\fzf.h"
#include "SMenu.h"
#include "SimpleWindowManager.h"

void button_redraw(Button *button)
{
    if(button->hwnd)
    {
        InvalidateRect(
          button->hwnd,
          NULL,
          TRUE
        );
        UpdateWindow(button->hwnd);
    }
}

void button_set_has_clients(Button *button, BOOL value)
{
    if(button->hasClients != value)
    {
        button->hasClients = value;
        button_redraw(button);
    }
}

void clients_add_as_first_node(Client *client)
{
    client->previous = NULL;
    client->next = NULL;
}

void clients_add_as_root_node(Client *currentRootNode, Client *client)
{
    client->previous = NULL;
    client->next = currentRootNode;
    currentRootNode->previous = client;
}

void workspace_add_unminimized_client(Workspace *workspace, Client *client)
{
    if(workspace->clients)
    {
        clients_add_as_root_node(workspace->clients, client);
    }
    else
    {
        clients_add_as_first_node(client);
    }
    workspace->clients = client;
    workspace->selected = client;
}

void workspace_add_minimized_client(Workspace *workspace, Client *client)
{
    if(workspace->minimizedClients)
    {
        clients_add_as_root_node(workspace->minimizedClients, client);
    }
    else
    {
        clients_add_as_first_node(client);
    }
    workspace->minimizedClients = client;
}

int workspace_update_client_counts(Workspace *workspace)
{
    int numberOfClients = 0;
    Client *c  = workspace->clients;
    while(c)
    {
        if(!c->data->isMinimized)
        {
            numberOfClients++;
            if(!c->next)
            {
                workspace->lastClient = c;
            }
        }
        c = c->next;
    }

    workspace->numberOfClients = numberOfClients;
    for(int i = 0; i < workspace->numberOfButtons; i++)
    {
        BOOL hasClients = numberOfClients > 0;
        button_set_has_clients(workspace->buttons[i], hasClients);
    }

    return numberOfClients;
}

void workspace_add_client(Workspace *workspace, Client *client)
{
    client->workspace = workspace;
    if(client->data->isMinimized)
    {
        workspace_add_minimized_client(workspace, client);
    }
    else
    {
        workspace_add_unminimized_client(workspace, client);
    }

    workspace_update_client_counts(workspace);
}

void clients_remove_root_node(Client *client)
{
    if(client->next)
    {
        client->next->previous = NULL;
    }
}

void clients_remove_surrounded_node(Client *client)
{
    client->previous->next = client->next;
    client->next->previous = client->previous;
}

void clients_remove_end_node(Client *client)
{
    if(client->previous)
    {
        client->previous->next = NULL;
    }
}

void workspace_remove_minimized_client(Workspace *workspace, Client *client)
{
    if(client == workspace->minimizedClients)
    {
        if(client->next)
        {
            workspace->minimizedClients = client->next;
            clients_remove_root_node(client);
            workspace->selected = client->next;
        }
        else
        {
            workspace->minimizedClients = NULL;
            workspace->selected = NULL;
        }
    }
    else if(client->next && client->previous)
    {
        clients_remove_surrounded_node(client);
        workspace->selected = client->previous;
    }
    else if(client->previous && !client->next)
    {
        clients_remove_end_node(client);
        workspace->selected = client->previous;
    }
    client->next = NULL;
    client->previous = NULL;
}

void workspace_remove_unminimized_client(Workspace *workspace, Client *client)
{
    if(client == workspace->clients)
    {
        if(client->next)
        {
            workspace->clients = client->next;
            clients_remove_root_node(client);
            workspace->selected = client->next;
        }
        else
        {
            workspace->clients = NULL;
            workspace->lastClient= NULL;
            workspace->selected = NULL;
        }
    }
    else if(client->next && client->previous)
    {
        clients_remove_surrounded_node(client);
        workspace->selected = client->previous;
    }
    else if(client == workspace->lastClient)
    {
        clients_remove_end_node(client);
        if(client->previous)
        {
            workspace->lastClient = client->previous;
            workspace->selected = client->previous;
        }
        else
        {
            //This really shouldn't happen
            workspace->lastClient = NULL;
        }
    }
    client->next = NULL;
    client->previous = NULL;
}

BOOL workspace_remove_client(Workspace *workspace, Client *client)
{
    if(client->data->isMinimized)
    {
        workspace_remove_minimized_client(workspace, client);
    }
    else
    {
        workspace_remove_unminimized_client(workspace, client);
    }

    workspace_update_client_counts(workspace);
    //FIX THIS
    return TRUE;
}

void client_move_to_location_on_screen(Client *client, HDWP hdwp, BOOL setZOrder)
{
    RECT wrect;
    RECT xrect;
    GetWindowRect(client->data->hwnd, &wrect);
    DwmGetWindowAttribute(client->data->hwnd, 9, &xrect, sizeof(RECT));

    long leftBorderWidth = xrect.left - wrect.left;
    long rightBorderWidth = wrect.right - xrect.right;
    long topBorderWidth = wrect.top - xrect.top;
    long bottomBorderWidth = wrect.bottom - xrect.bottom;

    long targetTop = client->data->y - topBorderWidth;
    long targetHeight = client->data->h + topBorderWidth + bottomBorderWidth;
    long targetLeft = client->data->x - leftBorderWidth;
    long targetWidth = client->data->w + leftBorderWidth + rightBorderWidth;

    if(!client->isVisible)
    {
        targetLeft = g_windowManagerState.hiddenWindowMonitor->xOffset;
    }

    if( targetTop == wrect.top &&
            targetLeft == wrect.left &&
            targetTop + targetHeight == wrect.bottom &&
            targetLeft + targetWidth == wrect.right)
    {
        return;
    }

    BOOL useOldMoveLogic = FALSE;
    if(configuration->useOldMoveLogicFunc)
    {
        if(configuration->useOldMoveLogicFunc(client))
        {
            useOldMoveLogic = TRUE;
        }
    }
    if(useOldMoveLogic)
    {
        MoveWindow(client->data->hwnd, targetLeft, targetTop, targetWidth, targetHeight, FALSE);
        ShowWindow(client->data->hwnd, SW_RESTORE);
        if(client->isVisible)
        {
            SetWindowPos(client->data->hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
            SetWindowPos(client->data->hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
        }
        else
        {
            SetWindowPos(client->data->hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE);
        }
    }
    else
    {
        UINT flags = SWP_SHOWWINDOW;
        if(client->data->useMinimizeToHide)
        {
            if(client->workspace->monitor->isHidden || !client->isVisible)
            {
                ShowWindow(client->data->hwnd, SW_MINIMIZE);
                return;
            }
        }
        if(!configuration->alwaysRedraw)
        {
            flags = SWP_NOREDRAW;
        }
        if(!setZOrder)
        {
            flags |= SWP_NOZORDER;
        }
        DeferWindowPos(
            hdwp,
            client->data->hwnd,
            NULL,
            targetLeft,
            targetTop,
            targetWidth,
            targetHeight,
            flags);
        if(client->data->useMinimizeToHide)
        {
            if(!client->workspace->monitor->isHidden)
            {
                ShowWindow(client->data->hwnd, SW_NORMAL);
                return;
            }
        }
    }
}

void workspace_arrange_clients(Workspace *workspace, HDWP hdwp)
{
    Client *c = workspace->clients;
    Client *lastClient = NULL;
    while(c)
    {
        if(!c->next)
        {
            lastClient = c;
        }
        c = c->next;
    }

    //Doing this in reverse so first client gets added last and show on top.
    //(I have no clue how to get ZOrder work)
    c = lastClient;
    while(c)
    {
        client_move_to_location_on_screen(c, hdwp, TRUE);
        c = c->previous;
    }
}

void workspace_arrange_windows_with_defer_handle(Workspace *workspace, HDWP hdwp)
{
    workspace->layout->apply_to_workspace(workspace);
    workspace_arrange_clients(workspace, hdwp);
}

void workspace_arrange_windows(Workspace *workspace)
{
    int numberOfWorkspaceClients = workspace_get_number_of_clients(workspace);
    HDWP hdwp = BeginDeferWindowPos(numberOfWorkspaceClients + 1);
    workspace_arrange_windows_with_defer_handle(workspace, hdwp);
    EndDeferWindowPos(hdwp);
}
