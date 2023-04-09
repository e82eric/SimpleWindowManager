#include <assert.h>
#include <windows.h>
#include <stdio.h>
#include <strsafe.h>
#include "fzf\\fzf.h"
#include "SMenu.h"
#include <assert.h>
#include <synchapi.h>
#include <commctrl.h>

#define IDI_ICON 101
#define IDC_LISTBOX_TEXT 1000
#define IDC_EDIT_TEXT 1001
#define IDC_STATIC_TEXT 1002
#define IDC_SUMMARY_TEXT 1003
#define IDT_LOADINGTIMER 1004
#define IDC_HELP_TEXT 1005

#define VK_P 0x50
#define VK_R 0x52
#define VK_N 0x4E
#define VK_K 0x4B
#define VK_C 0x43

#define CHUNK_SIZE 20000

#define WM_REDRAW_DISPLAY_LIST    (WM_USER + 1)

COLORREF backgroundColor = 0x282828;
COLORREF selectionBackgroundTextColor = RGB(168, 153, 132);
COLORREF highlightBackgroundColor = RGB(60,56,54);
COLORREF textColor = RGB(168, 153, 132);
COLORREF fuzzmatchCharTextColor = RGB(69, 133, 136);
COLORREF headerTextColor = RGB(250, 189, 47);
COLORREF searchTextColor = RGB(168, 153, 132);

HBRUSH backgrounBrush;
HBRUSH highlightedBackgroundBrush;

int listBoxItemHeight = 25;

HFONT font;

void ItemsView_SetLoading(ItemsView *self);
void ItemsView_SetDoneLoading(ItemsView *self);
VOID CALLBACK ItemsView_ProcessChunkWorker(PTP_CALLBACK_INSTANCE Instance, PVOID lpParam, PTP_WORK Work);
void DisplayItemList_MergeIntoRight(SearchView *searchView, DisplayItemList *self, DisplayItemList *resultList2);
void ItemsView_SetHeader(ItemsView *self, char* headerLine);
NamedCommand* ItemsView_FindNamedCommandByName(ItemsView *self, char *name);
DWORD WINAPI ProcessCmdOutputJobWorker(LPVOID lpParam);
void ProcessWithItemStreamOutput_Start(char *cmdArgs, ProcessCmdOutputJob *job);
void ItemsView_Clear(ItemsView *self);
void ItemsView_AddToEnd(ItemsView *self, char* text);

DWORD WINAPI SearchView_Worker(LPVOID lpParam)
{
    SearchView *self = (SearchView*)lpParam;

    SYSTEMTIME st;
    GetSystemTime(&st);

    EnterCriticalSection(&self->cancelSearchCriticalSection); 
    self->cancelSearch = TRUE;
    WaitForSingleObject(self->searchEvent, 50000);
    self->isSearching = TRUE;
    ResetEvent(self->searchEvent);
    self->cancelSearch = FALSE;
    LeaveCriticalSection(&self->cancelSearchCriticalSection);

    self->itemsView->isSearching = TRUE;

    ProcessChunkJob *jobs[250];
    int numberOfJobs = 0;

    PTP_WORK completeWorkHandles[50];
    Chunk *currentChunk = self->itemsView->chunks;
    while(currentChunk)
    {
        jobs[numberOfJobs] = calloc(1, sizeof(ProcessChunkJob));
        jobs[numberOfJobs]->fzfSlab = fzf_make_default_slab();
        jobs[numberOfJobs]->fzfPattern = fzf_parse_pattern(CaseSmart, false, self->searchString, true);
        jobs[numberOfJobs]->chunk = currentChunk;
        jobs[numberOfJobs]->allItemsWithScores = calloc(CHUNK_SIZE, sizeof(ItemMatchResult*));
        jobs[numberOfJobs]->jobNumber = numberOfJobs;
        jobs[numberOfJobs]->searchString = self->searchString;

        jobs[numberOfJobs]->displayItemList = calloc(1, sizeof(DisplayItemList));
        jobs[numberOfJobs]->displayItemList->items = calloc(self->itemsView->maxDisplayItems, sizeof(ItemMatchResult*));
        jobs[numberOfJobs]->displayItemList->maxItems = self->itemsView->maxDisplayItems;
        jobs[numberOfJobs]->searchView = self;

        completeWorkHandles[numberOfJobs] = CreateThreadpoolWork(ItemsView_ProcessChunkWorker, jobs[numberOfJobs], NULL);
        SubmitThreadpoolWork(completeWorkHandles[numberOfJobs]);

        numberOfJobs++;
        currentChunk = currentChunk->next;
    }

    for(int i = 0; i < numberOfJobs; i++)
    {
        WaitForThreadpoolWorkCallbacks(completeWorkHandles[i], FALSE);
    }

    DisplayItemList *mergedDisplayItemList = calloc(1, sizeof(DisplayItemList));
    mergedDisplayItemList->items = calloc(self->itemsView->maxDisplayItems, sizeof(ItemMatchResult*));
    mergedDisplayItemList->maxItems = self->itemsView->maxDisplayItems;

    int totalMatches = 0;
    for(int i = 0; i < numberOfJobs; i++)
    {
        totalMatches += jobs[i]->numberOfMatches;
        DisplayItemList_MergeIntoRight(self, jobs[i]->displayItemList, mergedDisplayItemList);
    }
    self->itemsView->numberOfItemsMatched = totalMatches;

    EnterCriticalSection(&self->searchCriticalSection);
    self->itemsView->numberOfDisplayItems = 0;
    for(int i = 0; i < mergedDisplayItemList->count; i++)
    {
        self->itemsView->displayItems[i] = mergedDisplayItemList->items[i]->item;
        self->itemsView->numberOfDisplayItems++;
    }
    LeaveCriticalSection(&self->searchCriticalSection);

    free(mergedDisplayItemList->items);
    free(mergedDisplayItemList);

    for(int i = 0; i < numberOfJobs; i++)
    {
        ProcessChunkJob *job = jobs[i];
        for(int j = 0; j < job->numberOfMatches; j++)
        {
            free(jobs[i]->allItemsWithScores[j]);
        }
        free(jobs[i]->allItemsWithScores);
    }

    for(int i = 0; i < numberOfJobs; i++)
    {
        fzf_free_slab(jobs[i]->fzfSlab);
        fzf_free_pattern(jobs[i]->fzfPattern);
        free(jobs[i]->displayItemList->items);
        free(jobs[i]->displayItemList);
        free(jobs[i]);
    }

    self->itemsView->isSearching = FALSE;
    SetEvent(self->searchEvent);
    PostMessage(self->hwnd, WM_REDRAW_DISPLAY_LIST, (WPARAM)NULL, (LPARAM)NULL);
    return TRUE;
}

void ItemsView_HandleDisplayItemsUpdate(ItemsView *self)
{
    SendMessageA(self->hwnd, WM_SETREDRAW, FALSE, 0);
    SendMessageA(self->hwnd, LB_RESETCONTENT, 0, 0);
    EnterCriticalSection(&self->searchView->searchCriticalSection);
    for(int i = 0; i < self->numberOfDisplayItems; i++)
    {
        SendMessageA(self->hwnd, LB_INSERTSTRING, i, (LPARAM)self->displayItems[i]->text);
    }
    LeaveCriticalSection(&self->searchView->searchCriticalSection);

    LRESULT lResult = SendMessageA(self->hwnd, LB_FINDSTRING, 0, (LPARAM)self->selectedString);
    if(lResult == LB_ERR || !self->itemSelected)
    {
        LRESULT newSelection = SendMessageA(self->hwnd, LB_SETCURSEL, 0, 0);
        char achBuffer[BUF_LEN];
        SendMessageA(self->hwnd, LB_GETTEXT, newSelection, (LPARAM)achBuffer); 
        memcpy(self->selectedString, achBuffer, BUF_LEN);
    }
    else
    {
        SendMessageA(self->hwnd, LB_SETCURSEL, lResult, 0);
    }
    SendMessageA(self->hwnd, WM_SETREDRAW, TRUE, 0);
}

void TriggerSearch(SearchView *self)
{
    DWORD dwThreadIdArray[1];
    CHAR searchString[BUF_LEN];
    GetWindowTextA(self->hwnd, searchString, BUF_LEN);
    if(self->searchString)
    {
        free(self->searchString);
    }
    self->searchString = _strdup(searchString); 
    HANDLE threadHandle = CreateThread( 
            NULL,
            0,
            SearchView_Worker,
            self,
            0,
            &dwThreadIdArray[0]);

    CloseHandle(threadHandle);
}

LRESULT CALLBACK Summary_MessageProcessor(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);
    ItemsView *self = (ItemsView*)dwRefData;

    switch (uMsg)
    {
         case WM_PAINT:
             static int spinnerCtr = 0;
             wint_t spinner[10] = { 0x280B, 0x2819, 0x2839, 0x2838,0x283C,0x2834,0x2827,0x2807,0x280F };
             wchar_t countBuffer[BUF_LEN];
             wchar_t spinnerBuffer[10];

             swprintf(spinnerBuffer, 10, L"%lc ", spinner[spinnerCtr]);
             swprintf(countBuffer, 256, L"%d/%d", self->numberOfItemsMatched, self->numberOfItems);
             if(spinnerCtr < 8)
             {
                 spinnerCtr++;
             }
             else
             {
                 spinnerCtr = 0;
             }

             PAINTSTRUCT ps;
             HDC hdc = BeginPaint(hWnd, &ps);

             SetTextAlign( hdc, TA_LEFT);
             FillRect(hdc, &ps.rcPaint, backgrounBrush);
             SelectObject(hdc, font);
             SetBkMode(hdc, TRANSPARENT);

             SIZE sz;
             int width = ps.rcPaint.right - ps.rcPaint.left;
             SetTextColor(hdc, textColor);

             size_t cch;
             StringCchLength(countBuffer, BUF_LEN, &cch);

             GetTextExtentPoint32(hdc, countBuffer, (int)cch, &sz); 
             int offset = width - sz.cx - 2;
             TextOut(hdc, offset, 2, countBuffer, (int)cch);
             if(self->isReading || self->isSearching)
             {
                 GetTextExtentPoint32(hdc, spinnerBuffer, 2, &sz); 
                 SetTextColor(hdc, fuzzmatchCharTextColor);
                 TextOut(hdc, offset - sz.cx, 2, spinnerBuffer, 1);
             }
             EndPaint(hWnd, &ps); 
             return 0;
    }

    return DefSubclassProc(hWnd, uMsg, wParam, lParam);
}

void ItemsView_HandleSelection(ItemsView *self)
{
    CHAR str[BUF_LEN] = "";
    CHAR result[BUF_LEN];
    LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
    if(dwSel != LB_ERR)
    {
        SendMessageA(self->hwnd, LB_GETTEXT, dwSel, (LPARAM)str);
        if(self->hasReturnRange)
        {
            sprintf_s(result, BUF_LEN, "%.*s", self->returnRangeEnd, str + self->returnRangeStart);
        }
        else
        {
            sprintf_s(result, BUF_LEN, "%s", str);
        }
    }

    ItemsView_Clear(self->searchView->itemsView);
    self->searchView->onEscape();
    if(self->onSelection)
    {
        self->onSelection(result);
    }
}

void ItemsView_SetHeader(ItemsView *self, char* headerLine)
{
    SetWindowTextA(self->headerHwnd, headerLine);
}

void ItemsView_SelectNext(ItemsView *self)
{
    LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
    if (dwSel != LB_ERR)
    {
        SendMessageA(self->hwnd, LB_SETCURSEL, dwSel + 1, 0);
        char achBuffer[BUF_LEN];
        SendMessageA(self->hwnd, LB_GETTEXT, dwSel + 1, (LPARAM)achBuffer); 
        memcpy(self->selectedString, achBuffer, BUF_LEN);
        self->itemSelected = TRUE;
    }
}

void ItemsView_SelectPrevious(ItemsView *self)
{
    LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
    if (dwSel != LB_ERR && dwSel > 0)
    {
        SendMessageA(self->hwnd, LB_SETCURSEL, dwSel + -1, 0);
        char achBuffer[BUF_LEN];
        SendMessageA(self->hwnd, LB_GETTEXT, dwSel + 1, (LPARAM)achBuffer); 
        memcpy(self->selectedString, achBuffer, BUF_LEN);
        self->itemSelected = TRUE;
    }
}

void ItemsView_StartBindingProcess(ItemsView *itemsView, char *cmdArgs, WAITORTIMERCALLBACK callback, BOOL reloadAfter, BOOL quitAfter)
{
    PROCESS_INFORMATION pi = { 0 };

    SECURITY_ATTRIBUTES sattr;
    sattr.nLength = sizeof(SECURITY_ATTRIBUTES);
    sattr.lpSecurityDescriptor = NULL;
    sattr.bInheritHandle = TRUE;

    STARTUPINFOA si = { 0 };

    if(!CreateProcessA(
        NULL,
        cmdArgs,
        NULL,
        NULL,
        TRUE,
        CREATE_NO_WINDOW,
        NULL,
        NULL,
        &si,
        &pi)
    ) 
    {
        return;
    }

    ProcessAsyncState *processAsyncState = calloc(1, sizeof(ProcessAsyncState));
    processAsyncState->thread = pi.hThread;
    processAsyncState->processId = pi.hProcess;
    processAsyncState->itemsView = itemsView;
    processAsyncState->reloadAfter = reloadAfter;
    processAsyncState->quitAfter = quitAfter;

    BOOL result = RegisterWaitForSingleObject(
        &processAsyncState->waitHandle,
        pi.hProcess,
        callback,
        (void*)processAsyncState,
        INFINITE,
        WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE);

    if (!result)
    {
    }
}

void NamedCommand_Parse(char *dest, char *curSel, NamedCommand *command, size_t maxLen)
{
    int destCtr = 0;
    for(int i = 0; i < command->expressionLen; i++)
    {
        if(destCtr > maxLen)
        {
            break;
        }
        if(i == command->indexOfReplacement)
        {
            for(int j = 0; j < strlen(curSel); j++)
            {
                dest[destCtr] = curSel[j];
                destCtr++;
            }
            //Skip second } in replacement
            i++;
        }
        else
        {
            dest[destCtr] = command->expression[i];
            destCtr++;
        }
    }

    dest[destCtr] = '\0';
}

void ItemsView_StartLoadItemsFromStdIn(ItemsView *self)
{
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = self;
    job->readHandle = GetStdHandle(STD_INPUT_HANDLE);

    DWORD   dwThreadIdArray[1];
    HANDLE  hThreadArray[1]; 
    hThreadArray[0] = CreateThread( 
            NULL,
            0,
            ProcessCmdOutputJobWorker,
            job,
            0,
            &dwThreadIdArray[0]);

    CloseHandle(hThreadArray[0]);
}

void ItemsView_StartLoadItems_FromCommand(ItemsView *self, NamedCommand *namedCommand)
{
    ItemsView_Clear(self);
    UNREFERENCED_PARAMETER(self);
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = self;

    ProcessWithItemStreamOutput_Start(namedCommand->expression, job);
}

void ItemsView_LoadFromAction(ItemsView *self, int (itemsAction)(int maxItems, CHAR **items))
{
    ItemsView_Clear(self);

    ItemsView_SetLoading(self);
    self->chunks = calloc(1, sizeof(Chunk));
    self->chunks->items = calloc(CHUNK_SIZE, sizeof(Item*));

    CHAR *itemsBuf[CHUNK_SIZE];
    int numberOfItems = itemsAction(CHUNK_SIZE, itemsBuf);

    for(int i = 0; i < numberOfItems; i++)
    {
        ItemsView_AddToEnd(self, itemsBuf[i]);
        free(itemsBuf[i]);
    }

    TriggerSearch(self->searchView);
    ItemsView_SetDoneLoading(self);
}

void ItemsView_StartLoadItems(ItemsView *self)
{
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = self;

    if(self->loadCommand)
    {
        ProcessWithItemStreamOutput_Start(self->loadCommand->expression, job);
    }
    else if(self->loadAction)
    {
        ItemsView_LoadFromAction(self, self->loadAction);
    }
}

void ItemsView_Clear(ItemsView *self)
{
    EnterCriticalSection(&self->loadCriticalSection); 
    if(self->isLoading)
    {
        self->cancelLoad = TRUE;
    }
    LeaveCriticalSection(&self->loadCriticalSection); 
    WaitForSingleObject(self->loadEvent, 50000);

    EnterCriticalSection(&self->searchView->searchCriticalSection); 
    if(self->searchView->isSearching)
    {
        self->searchView->cancelSearch = TRUE;
    }
    LeaveCriticalSection(&self->searchView->searchCriticalSection); 
    WaitForSingleObject(self->searchView->searchEvent, 50000);

    SendMessageA(self->hwnd, LB_SETCURSEL, 0, -1);
    SendMessage(self->hwnd, LB_RESETCONTENT, 0, 0);;
    self->itemSelected = FALSE;

    Chunk *currentChunk = self->chunks;
    while(currentChunk)
    {
        for(int i = 0; i < currentChunk->count; i++)
        {
            free(currentChunk->items[i]->text);
            free(currentChunk->items[i]);
        }

        free(currentChunk->items);
        Chunk *temp = currentChunk;
        currentChunk = currentChunk->next;
        free(temp);
    }

    self->chunks = NULL;
    self->numberOfItems = 0;
    self->headerSet = FALSE;

    self->cancelLoad = FALSE;
    self->searchView->cancelSearch = FALSE;
}

void ItemsView_ReloadFromCommand(ItemsView *self, NamedCommand *command)
{
    ItemsView_Clear(self);

    ItemsView_StartLoadItems_FromCommand(self, command);
}

void ItemsView_HandleReload(ItemsView *self)
{
    ItemsView_Clear(self);
    ItemsView_StartLoadItems(self);
}

void CALLBACK BindProcessFinishCallback(void* lpParameter, BOOLEAN isTimeout)
{
    UNREFERENCED_PARAMETER(isTimeout);
    ProcessAsyncState *asyncState = (ProcessAsyncState*)lpParameter;

    if(asyncState->reloadAfter && !asyncState->quitAfter)
    {
        ItemsView_HandleReload(asyncState->itemsView);
    }
    UnregisterWait(asyncState->waitHandle);
    CloseHandle(asyncState->thread);
    CloseHandle(asyncState->processId);
    if(asyncState->quitAfter)
    {
        asyncState->itemsView->searchView->onEscape();
        ItemsView_Clear(asyncState->itemsView);
    }

    free(asyncState);
}

void ItemsView_HandleBinding(ItemsView *self, NamedCommand *command)
{
    if(self->namedCommands)
    {
        if(command)
        {
            LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
            char newBindCommand[BUF_LEN];
            CHAR curSelStr[BUF_LEN] = "";
            SendMessageA(self->hwnd, LB_GETTEXT, dwSel, (LPARAM)curSelStr);
            CHAR text[BUF_LEN];
            int numberOfTextChars = 0;

            if(command->hasTextRange)
            {
                numberOfTextChars = sprintf_s(text, BUF_LEN, "%.*s", command->textRangeEnd, curSelStr + command->textRangeStart);
            }
            else
            {
                numberOfTextChars = sprintf_s(text, BUF_LEN, "%s", curSelStr);
            }

            if(command->trimEnd)
            {
                for(size_t i = numberOfTextChars - 1; i >= 0; i--)
                {
                    if(isspace(text[i]))
                    {
                        text[i] = '\0';
                    }
                    else
                    {
                        break;
                    }
                }
            }

            NamedCommand_Parse(newBindCommand, text, command, BUF_LEN);

            if(command->action)
            {
                command->action(text);
            }
            else if(command->action2)
            {
                command->action2(command->action2State);
            }
            else
            {
                ItemsView_StartBindingProcess(self, newBindCommand, BindProcessFinishCallback, command->reloadAfter, command->quitAfter);
            }
        }
    }
}

void SearchView_ShowHelp(SearchView *self)
{
    CHAR helpStr[2048];
    MenuKeyBinding *current = self->keyBindings;
    int ctr = 0;

    while(current)
    {
        CHAR modifiersKeyName[MAX_PATH];
        if(current->modifier == VK_CONTROL)
        {
            memcpy(modifiersKeyName, "CTL+", 50);
        }
        else
        {
            memcpy(modifiersKeyName, "", 50);
        }

        unsigned int scanCode = MapVirtualKey(current->key, MAPVK_VK_TO_VSC);

        switch (current->key)
        {
            case VK_LEFT: case VK_UP: case VK_RIGHT: case VK_DOWN:
            case VK_PRIOR: case VK_NEXT:
            case VK_END: case VK_HOME:
            case VK_INSERT: case VK_DELETE:
            case VK_DIVIDE:
            case VK_NUMLOCK:
                {
                    scanCode |= 0x100;
                    break;
                }
        }

        CHAR keyStrBuff[MAX_PATH];
        GetKeyNameTextA(scanCode << 16, keyStrBuff, MAX_PATH);

        CHAR bindStr[25];
        sprintf_s(
                bindStr,
                25,
                "%s%s",
                modifiersKeyName,
                keyStrBuff);
        if(ctr == 0)
        {
            sprintf_s(
                    helpStr,
                    2048,
                    "%-25s%-25s%-25s",
                    bindStr,
                    current->command->name,
                    current->command->expression);
        }
        else
        {
            sprintf_s(
                    helpStr,
                    2048,
                    "%s\n%-25s%-25s%-25s",
                    helpStr,
                    bindStr,
                    current->command->name,
                    current->command->expression);
        }

        current = current->next;
        ctr++;
    }

    SetWindowTextA(self->helpHwnd, helpStr);
    ShowWindow(self->helpHwnd, SW_SHOW);
    SetWindowPos(self->helpHwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    SendMessage(self->hwnd, EM_SETREADONLY, (WPARAM)TRUE, (LPARAM)NULL);
    SendMessage(self->helpHwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);

    self->mode =  Help;
}

void SearchView_HideHelp(SearchView *self)
{
    ShowWindow(self->helpHwnd, SW_HIDE);
    SendMessage(self->hwnd, EM_SETREADONLY, (WPARAM)FALSE, (LPARAM)NULL);
    self->mode = Normal;
}

void SearchView_HandleEscape(SearchView *self)
{
    if(self->mode == Help)
    {
        SearchView_HideHelp(self);
    }
    else
    {
        self->onEscape();
        ItemsView_Clear(self->itemsView);
    }
}

LRESULT CALLBACK SearchView_MessageProcessor(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);

    SearchView *self = (SearchView*)dwRefData;

    switch (uMsg)
    {
        case WM_REDRAW_DISPLAY_LIST:
            {
                ItemsView_HandleDisplayItemsUpdate(self->itemsView);
            }
            break;
    }

    if((uMsg == WM_KEYDOWN))
    {
        MenuKeyBinding *current = self->keyBindings;
        while(current)
        {
            if(wParam == current->key)
            {
                if(current->modifier == 0)
                {
                    ItemsView_HandleBinding(self->itemsView, current->command);
                    return 1;
                }
                else if(GetAsyncKeyState(current->modifier) & 0x8000)
                {
                    if(current->isLoadCommand)
                    {
                        if(current->loadAction)
                        {
                            ItemsView_LoadFromAction(self->itemsView, current->loadAction);
                        }
                        else
                        {
                            ItemsView_ReloadFromCommand(self->itemsView, current->command);
                        }
                    }
                    else
                    {
                        ItemsView_HandleBinding(self->itemsView, current->command);
                    }

                    return 1;
                }
            }
            current = current->next;
        }
    }

    if((uMsg == WM_CHAR || uMsg == WM_KEYDOWN))
    {
        //Prevent BEEP
        if(GetAsyncKeyState(VK_CONTROL))
        {
            return 0;
        }
    }

    return DefSubclassProc(hWnd, uMsg, wParam, lParam);
}

void DisplayItemList_UpdateMinScore(DisplayItemList *self)
{
    self->minScore = self->items[self->count - 1]->score;
}

void DisplayItemList_TryAdd(DisplayItemList *self, ItemMatchResult *matchResult)
{
    BOOL itemsCheck = self->count < self->maxItems;
    BOOL minScoreCheck = matchResult->score > self->minScore;

    if(itemsCheck || minScoreCheck)
    {
        if(self->count == 0)
        {
            self->items[0] = matchResult;
            self->count++;
            return;
        }

        for(int i = 0; i < self->count; i++)
        {
            ItemMatchResult *iItem = self->items[i];
            if(matchResult->score > iItem->score)
            {
                for(int j = self->count; j >= i; j--)
                {
                    if(j + 1 < self->maxItems)
                    {
                        self->items[j + 1] = self->items[j];
                    }
                }
                self->items[i] = matchResult;
                if(self->count < self->maxItems)
                {
                    self->count++;
                }
                DisplayItemList_UpdateMinScore(self);
                return;
            }
        }

        if(self->count < self->maxItems)
        {
            self->items[self->count] = matchResult;
            self->count++;
            DisplayItemList_UpdateMinScore(self);
            return;
        }
    }
}

VOID CALLBACK ItemsView_ProcessChunkWorker(PTP_CALLBACK_INSTANCE Instance, PVOID lpParam, PTP_WORK Work)
{
    UNREFERENCED_PARAMETER(Work);
    UNREFERENCED_PARAMETER(Instance);

    ProcessChunkJob *job = (ProcessChunkJob*)lpParam;
    Chunk *chunk = job->chunk;

    for(int i = 0; i < chunk->count; i++)
    {
        Item *item = chunk->items[i];
        if(job->searchView->cancelSearch)
        {
            return;
        }

        int score = fzf_get_score(item->text, job->fzfPattern, job->fzfSlab);

        if(score > 0)
        {
            ItemMatchResult *matchResult = calloc(1, sizeof(ItemMatchResult));
            matchResult->item = item;
            matchResult->score = score;
            job->numberOfMatches++;
            job->allItemsWithScores[job->numberOfMatches - 1] = matchResult;
            DisplayItemList_TryAdd(job->displayItemList, matchResult);
        }
    }

    return;
}

void DisplayItemList_MergeIntoRight(SearchView *searchView, DisplayItemList *self, DisplayItemList *resultList2)
{
    for(int i = 0; i < self->count; i++)
    {
        ItemMatchResult *matchResult = self->items[i];
        if(searchView->cancelSearch)
        {
            return;
        }

        if(matchResult->score > 0)
        {
            DisplayItemList_TryAdd(resultList2, matchResult);
        }
    }
}

void ItemsView_AddToEnd(ItemsView *self, char* text)
{
    if(self->numberOfItems == 0 && self->hasHeader && !self->headerSet)
    {
        ItemsView_SetHeader(self, text);
        self->headerSet = TRUE;
        return;
    }

    Item *item = calloc(1, sizeof(Item));
    item->text = _strdup(text);

    Chunk *current = self->chunks;
    while(current->next)
    {
        if(current->count < CHUNK_SIZE)
        {
            assert(!current->next);
        }
        current = current->next;
    }

    if(current->count + 1 < CHUNK_SIZE)
    {
        current->items[current->count] = item;
        current->count++;
    }
    else if(current->count + 1 == CHUNK_SIZE)
    {
        current->next = calloc(1, sizeof(Chunk));
        current->next->items = calloc(CHUNK_SIZE, sizeof(Item*));
        current->items[current->count] = item;
        current->count++;
    }
    else
    {
        assert(FALSE);
    }

    self->numberOfItems++;
    assert(current->count <= CHUNK_SIZE);
}

BOOL FillLineFromBuffer(char *target, char *buffer, int max, int *charsRead, int *charsWritten)
{
    for(int i = 0; i < max; i++)
    {
        (*charsRead)++;
        (*charsWritten)++;
        if(buffer[i] == '\n')
        {
            if(buffer[i -1] == '\r')
            {
                target[i - 1] = '\0';
                (*charsWritten) = i;
                return TRUE;
            }
            else
            {
                target[i] = '\0';
                (*charsWritten) = i + 1;
                return TRUE;
            }
        }
        else
        {
            target[i] = buffer[i];
        }
    }

    return FALSE;
}

void ItemsView_StartLoad(ItemsView *self)
{
    EnterCriticalSection(&self->loadCriticalSection);
    self->isLoading = TRUE;
    ItemsView_SetLoading(self);
    ResetEvent(self->loadEvent);
    LeaveCriticalSection(&self->loadCriticalSection);
}

void ItemsView_CompleteLoad(ItemsView *self)
{
    EnterCriticalSection(&self->loadCriticalSection);
    self->isLoading = FALSE;
    ItemsView_SetDoneLoading(self);
    SetEvent(self->loadEvent);
    LeaveCriticalSection(&self->loadCriticalSection);
}

BOOL ProcessCmdOutJob_ProcessStream(ProcessCmdOutputJob *self)
{
    ItemsView_StartLoad(self->itemsView);
    CHAR buffer[BUF_LEN];
    CHAR line[BUF_LEN];
    DWORD readBytes = 0;

    if(!self->itemsView->chunks)
    {
        self->itemsView->chunks = calloc(1, sizeof(Chunk));
        self->itemsView->chunks->items = calloc(CHUNK_SIZE, sizeof(Item*));
    }

    self->itemsView->numberOfItems = 0;
    self->itemsView->headerSet = FALSE;
    int searchTriggerCtr = 0;
    int lineIndex = 0;

    while(ReadFile(self->readHandle, buffer, BUF_LEN, &readBytes, NULL))
    {
        int bufferIndex = 0;
        BOOL isFullLine = TRUE;
        while(isFullLine)
        {
            int charsRead = 0;
            int lineLen = 0;
            isFullLine = FillLineFromBuffer(line + lineIndex, buffer + bufferIndex, readBytes - bufferIndex, &charsRead, &lineLen);
            if(isFullLine)
            {
                if(self->itemsView->cancelLoad)
                {
                    ItemsView_CompleteLoad(self->itemsView);
                    return FALSE;
                }
                ItemsView_AddToEnd(self->itemsView, line);
                searchTriggerCtr++;
                if(searchTriggerCtr > 50000)
                {
                    TriggerSearch(self->itemsView->searchView);
                    searchTriggerCtr = 0;
                }

                lineIndex = 0;
            }
            else
            {
                lineIndex += lineLen;
            }

            bufferIndex += charsRead;
        }
    }

    TriggerSearch(self->itemsView->searchView);
    ItemsView_CompleteLoad(self->itemsView);
    return TRUE;
}

DWORD WINAPI ProcessCmdOutputJobWorker(LPVOID lpParam)
{
    ProcessCmdOutputJob *processCmdOutputJob = (ProcessCmdOutputJob*)lpParam;

    ProcessCmdOutJob_ProcessStream(processCmdOutputJob);

    CloseHandle(processCmdOutputJob->readHandle);
    free(processCmdOutputJob);
    return 0;
}

void ProcessWithItemStreamOutput_Start(char *cmdArgs, ProcessCmdOutputJob *job)
{
    PROCESS_INFORMATION pi = { 0 };
    HANDLE stdOutWriteHandle;

    SECURITY_ATTRIBUTES sattr;
    sattr.nLength = sizeof(SECURITY_ATTRIBUTES);
    sattr.lpSecurityDescriptor = NULL;
    sattr.bInheritHandle = TRUE;

    if(!CreatePipe(&job->readHandle, &stdOutWriteHandle, &sattr, 0))
    {
        return;
    }

    if(SetHandleInformation(&job->readHandle, HANDLE_FLAG_INHERIT, 0))
    {
        return;
    }

    STARTUPINFOA si = { 0 };
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = stdOutWriteHandle;
    si.hStdError = stdOutWriteHandle;

    if(!CreateProcessA(
        NULL,
        cmdArgs,
        NULL,
        NULL,
        TRUE,
        CREATE_NO_WINDOW,
        NULL,
        NULL,
        &si,
        &pi)
    ) 
    {
        CloseHandle(job->readHandle);
        CloseHandle(stdOutWriteHandle);
        return;
    }

    CloseHandle(stdOutWriteHandle);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);

    DWORD   dwThreadIdArray[1];
    HANDLE  hThreadArray[1]; 
    hThreadArray[0] = CreateThread( 
            NULL,
            0,
            ProcessCmdOutputJobWorker,
            job,
            0,
            &dwThreadIdArray[0]);

    CloseHandle(hThreadArray[0]);
}

void ItemsView_SetLoading(ItemsView *self)
{
    self->isReading = TRUE;
}

void ItemsView_SetDoneLoading(ItemsView *self)
{
    self->isReading = FALSE;
}

void ItemsView_Move(ItemsView *self, int left, int top, int width, int height)
{
    int listTop;
    int listHeight;

    if(self->hasHeader)
    {
        ShowWindow(self->headerHwnd, SW_SHOW);
        int headerTop = top;
        int headerHeight = 30;
        MoveWindow(self->headerHwnd, left, headerTop, width, headerHeight, TRUE);

        listHeight = height - headerHeight;
        listTop = headerHeight + headerTop;
    }
    else
    {
        ShowWindow(self->headerHwnd, SW_HIDE);
        listTop = top;
        listHeight = height;
    }

    MoveWindow(self->hwnd, left, listTop, width, listHeight, TRUE);
    self->height = listHeight;
    self->maxDisplayItems = (listHeight - 3) / listBoxItemHeight;
}

void HelpView_Move(HWND hwnd, int left, int top, int width, int height)
{
    MoveWindow(hwnd, left, top, width, height, TRUE);
}

void MenuView_FitChildControls(MenuView *self)
{
    RECT xrect;
    GetClientRect(self->hwnd, &xrect);

    int padding = 10;
    int width = self->width - (padding * 2);

    int summaryWidth = 400;

    int searchWidth = width - summaryWidth;
    int summaryLeft = searchWidth + padding;
    int searchTop = padding;
    int searchHeight = 30;
    MoveWindow(self->searchView->hwnd, padding, searchTop, searchWidth, searchHeight, TRUE);
    MoveWindow(self->itemsView->summaryHwnd, summaryLeft, searchTop, summaryWidth, searchHeight, TRUE);

    int itemsTop = searchTop + searchHeight + padding;
    ItemsView_Move(
            self->itemsView,
            padding,
            itemsTop,
            width,
            self->height - itemsTop - padding);
    SetFocus(self->searchView->hwnd);

    int helpTop = padding;
    int helpLeft = padding;
    int helpWidth = width;
    int helpHeight = self->height - (padding * 2);
    HelpView_Move(
            self->searchView->helpHwnd,
            helpLeft,
            helpTop,
            helpWidth,
            helpHeight);
}

void MenuView_CreateChildControls(MenuView *self)
{
    HINSTANCE hinstance = (HINSTANCE)GetWindowLongPtr(self->hwnd, GWLP_HINSTANCE);
    self->searchView->hwnd = CreateWindowA(
            "Edit", 
            NULL, 
            WS_VISIBLE | WS_CHILD | WS_CLIPSIBLINGS,
            10, 
            10, 
            600, 
            10,
            self->hwnd, 
            (HMENU)IDC_EDIT_TEXT,
            hinstance,
            NULL);
    SetWindowSubclass(self->searchView->hwnd, SearchView_MessageProcessor, 0, (DWORD_PTR)self->searchView);
    SendMessage(self->searchView->hwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);

    self->itemsView->headerHwnd = CreateWindowA(
            "STATIC", 
            NULL, 
            WS_VISIBLE | WS_CHILD | SS_LEFT | WS_BORDER | WS_CLIPSIBLINGS,
            10, 
            10, 
            600, 
            10,
            self->hwnd, 
            (HMENU)IDC_STATIC_TEXT,
            hinstance,
            NULL);
    SendMessageA(self->itemsView->headerHwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);

    self->itemsView->summaryHwnd = CreateWindow(
            L"STATIC", 
            NULL, 
            WS_VISIBLE | WS_CHILD | SS_RIGHT | SS_OWNERDRAW | WS_CLIPSIBLINGS,
            10, 
            10, 
            600, 
            10,
            self->hwnd, 
            (HMENU)IDC_SUMMARY_TEXT,
            hinstance,
            NULL);
    SendMessageA(self->itemsView->summaryHwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);
    SetWindowSubclass(self->itemsView->summaryHwnd, Summary_MessageProcessor, 0, (DWORD_PTR)self->itemsView);

    self->itemsView->hwnd = CreateWindowA(
            "LISTBOX", 
            NULL, 
            WS_VISIBLE | WS_CHILD | LBS_STANDARD | LBS_HASSTRINGS | LBS_OWNERDRAWFIXED | WS_CLIPSIBLINGS,
            260, 
            260, 
            900, 
            600,
            self->hwnd, 
            (HMENU)IDC_LISTBOX_TEXT, 
            (HINSTANCE)GetWindowLongPtr(self->hwnd, GWLP_HINSTANCE),
            NULL);
    SendMessageA(self->itemsView->hwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);

    self->searchView->helpHwnd = CreateWindowA(
            "STATIC", 
            NULL, 
            WS_VISIBLE | WS_CHILD | SS_LEFT | WS_BORDER | WS_CLIPSIBLINGS,
            260, 
            260, 
            900, 
            600,
            self->hwnd, 
            (HMENU)IDC_HELP_TEXT, 
            (HINSTANCE)GetWindowLongPtr(self->hwnd, GWLP_HINSTANCE),
            NULL);
    ShowWindow(self->searchView->helpHwnd, SW_HIDE);

    SetTimer(self->hwnd,
            IDT_LOADINGTIMER,
            75,
            (TIMERPROC) NULL);
    SetFocus(self->searchView->hwnd);
}

LRESULT CALLBACK Menu_MessageProcessor(
        HWND hWnd,
        UINT uMsg,
        WPARAM wParam,
        LPARAM lParam)
{
    MenuView *self;
    if (uMsg == WM_CREATE)
    {
        CREATESTRUCT *pCreate = (CREATESTRUCT*)lParam;
        self = (MenuView*)pCreate->lpCreateParams;
        SetWindowLongPtr(hWnd, GWLP_USERDATA, (LONG_PTR)self);
        //Do I need this
        if(self->hwnd == 0)
        {
            self->hwnd = hWnd;
        }
        MenuView_CreateChildControls(self);
    }
    else
    {
        LONG_PTR ptr = GetWindowLongPtr(hWnd, GWLP_USERDATA);
        self = (MenuView*)ptr;
    }

    switch (uMsg)
    {
        case WM_CTLCOLOREDIT:
            {
                HDC hdcStatic = (HDC)wParam;
                SetTextColor(hdcStatic, searchTextColor);
                SetBkColor(hdcStatic, backgroundColor);
                return (INT_PTR)backgrounBrush;
            }
        case WM_CTLCOLORSTATIC:
            {
                if((HWND)lParam == self->itemsView->headerHwnd)
                {
                    HDC hdcStatic = (HDC)wParam;
                    SetTextColor(hdcStatic, headerTextColor);
                    SetBkColor(hdcStatic, backgroundColor);
                    return (INT_PTR)backgrounBrush;
                }
            }
        case WM_CTLCOLORLISTBOX:
            {
                HDC hdcStatic = (HDC)wParam;
                SetTextColor(hdcStatic, textColor);
                SetBkColor(hdcStatic, backgroundColor);
                return (INT_PTR)backgrounBrush;
            }
        case WM_SIZE:
            {
                self->height = HIWORD(lParam);
                self->width = LOWORD(lParam);
                MenuView_FitChildControls(self);
            }
            break;
        /* case WM_MOVE: */
        /*     { */
        /*         /1* SetFocus(menuView->searchView->hwnd); *1/ */
        /*         /1* MenuView_FitChildControls(menuView); *1/ */
        /*     } */
        /*     break; */
        case WM_DESTROY:
            {
                PostQuitMessage(0);
                return 0;
            }
        case WM_TIMER:
            switch (wParam)
            {
                case IDT_LOADINGTIMER:
                    SetFocus(self->searchView->hwnd);
                    InvalidateRect(self->itemsView->summaryHwnd, NULL, FALSE);
            }
        case WM_COMMAND:
            {
                switch(LOWORD(wParam))
                {
                    case IDC_EDIT_TEXT:
                        {
                            if(HIWORD(wParam) == EN_CHANGE)
                            {
                                TriggerSearch(self->searchView);
                            }
                        }
                }
                return 0;
            }
            break; 

        case WM_MEASUREITEM:
        {
            MEASUREITEMSTRUCT*  pmis = (LPMEASUREITEMSTRUCT)lParam;
            switch (pmis->CtlID)
            {
                case IDC_LISTBOX_TEXT:
                    pmis->itemHeight = listBoxItemHeight;
                    break;
                default:
                    break;
            }
            return TRUE;
        }
        case WM_DRAWITEM: 
            CHAR achBuffer[BUF_LEN];
            size_t cch;
            TEXTMETRIC tm; 
            HRESULT hr; 

            PDRAWITEMSTRUCT pdis = (PDRAWITEMSTRUCT) lParam;
 
            if (pdis->itemID == -1)
            {
                break;
            }
 
            switch (pdis->itemAction)
            {
                case ODA_SELECT:
                case ODA_DRAWENTIRE:
                    COLORREF colorText;
                    COLORREF colorBack;
                    if(pdis->itemState & ODS_SELECTED) 
                    {
                        FillRect(pdis->hDC, &pdis->rcItem, highlightedBackgroundBrush);
                        colorText = selectionBackgroundTextColor;
                        colorBack = SetBkColor(pdis->hDC, highlightBackgroundColor);
                    }
                    else
                    {
                        FillRect(pdis->hDC, &pdis->rcItem, backgrounBrush);
                        colorText = textColor;
                        colorBack = SetBkColor(pdis->hDC, backgroundColor);
                    }

                    SendMessageA(pdis->hwndItem, LB_GETTEXT, pdis->itemID, (LPARAM)achBuffer); 

                    GetTextMetrics(pdis->hDC, &tm); 

                    hr = StringCchLengthA(achBuffer, BUF_LEN, &cch);
                    if (FAILED(hr))
                    {
                        // TODO: Handle error.
                    }

                    SetTextColor(pdis->hDC, textColor);
                    TextOutA(pdis->hDC, 0, pdis->rcItem.top + 2, achBuffer, (int)cch);

                    CHAR searchStringBuff[BUF_LEN];
                    GetWindowTextA(self->searchView->hwnd, searchStringBuff, BUF_LEN);
                    fzf_pattern_t *fzfPattern = fzf_parse_pattern(CaseSmart, false, searchStringBuff, true);

                    if(fzfPattern)
                    {
                        fzf_position_t *pos = fzf_get_positions(achBuffer, fzfPattern, self->itemsView->fzfSlab);

                        SIZE sz;

                        if(pos && pos->size > 0)
                        {
                            SetTextColor(pdis->hDC, fuzzmatchCharTextColor);
                            for(int i = 0; i < pos->size; i++)
                            {
                                GetTextExtentPoint32A(pdis->hDC, achBuffer, pos->data[i], &sz); 
                                TextOutA(pdis->hDC, sz.cx, pdis->rcItem.top + 2, achBuffer + pos->data[i], 1);
                            }
                        }

                        fzf_free_positions(pos);
                        fzf_free_pattern(fzfPattern);
                    }

                    if (pdis->itemState & ODS_SELECTED) 
                    {
                        DrawFocusRect(pdis->hDC, &pdis->rcItem);
                        SetTextColor(pdis->hDC, colorText);
                        SetBkColor(pdis->hDC, colorBack);
                    }
                    break; 
 
                case ODA_FOCUS: 
                    // Do not process focus changes. The focus caret 
                    // (outline rectangle) indicates the selection. 
                    // The IDOK button indicates the final 
                    // selection. 
                    break; 
            }

        default:
            return (DefWindowProc(hWnd, uMsg, wParam, lParam));
    }

    return 0;
}

NamedCommand* ItemsView_FindNamedCommandByName(ItemsView *self, char *name)
{
    NamedCommand *currentCommand = self->namedCommands;
    NamedCommand *command = NULL;
    while(currentCommand)
    {
        if(strcmp(name, currentCommand->name) == 0)
        {
            command = currentCommand;
            break;
        }

        currentCommand = currentCommand->next;
    }
    return command;
}

NamedCommand* MenuDefinition_FindNamedCommandByName(MenuDefinition *self, char *name)
{
    NamedCommand *currentCommand = self->namedCommands;
    NamedCommand *command = NULL;
    while(currentCommand)
    {
        if(strcmp(name, currentCommand->name) == 0)
        {
            command = currentCommand;
            break;
        }

        currentCommand = currentCommand->next;
    }
    return command;
}

void MenuDefinition_AddLoadActionKeyBinding(MenuDefinition *self, unsigned int modifier, unsigned int key, int (*loadAction)(int, CHAR**))
{
    MenuKeyBinding *binding = calloc(1, sizeof(MenuKeyBinding));
    binding->modifier = modifier;
    binding->key = key;
    binding->isLoadCommand = TRUE;
    binding->loadAction = loadAction;

    if(!self->keyBindings)
    {
        self->keyBindings = binding;
        self->lastKeyBinding = binding;
    }
    else
    {
        self->lastKeyBinding->next = binding;
        self->lastKeyBinding = binding;
    }
}

void MenuDefinition_AddKeyBindingToNamedCommand(MenuDefinition *self, NamedCommand *namedCommand, unsigned int modifier, unsigned int key, BOOL isLoadCommand)
{
    MenuKeyBinding *binding = calloc(1, sizeof(MenuKeyBinding));
    binding->modifier = modifier;
    binding->key = key;
    binding->command = namedCommand;
    binding->isLoadCommand = isLoadCommand;

    if(!self->keyBindings)
    {
        self->keyBindings = binding;
        self->lastKeyBinding = binding;
    }
    else
    {
        self->lastKeyBinding->next = binding;
        self->lastKeyBinding = binding;
    }
}

void MenuDefinition_ParseAndAddKeyBinding(MenuDefinition *self, char *argText, BOOL isLoadCommand)
{
    size_t len = strlen(argText);
    const int startOfName = 6;

    if(len < 8)
    {
        return;
    }

    if(argText[3] != '-' || argText[5] != ':')
    {
        return;
    }

    short virtualKey = VkKeyScanW(argText[4]);
    unsigned int modifier = 0;

    if(argText[0] == 'c' && argText[1] == 't' && argText[2] == 'l')
    {
        modifier = VK_CONTROL;
    }
    if(argText[0] == 'a' && argText[1] == 't' && argText[2] == 'l')
    {
        /* binding->modifier = VM_LALT; */
    }

    char nameBuff[BUF_LEN];
    for(int i = 6; i < len; i++)
    {
        nameBuff[i - startOfName] = argText[i];
    }

    nameBuff[len - startOfName] = '\0';

    NamedCommand *command = MenuDefinition_FindNamedCommandByName(self, nameBuff);

    if(modifier && command)
    {
        MenuDefinition_AddKeyBindingToNamedCommand(self, command, modifier, virtualKey, isLoadCommand);
    }
}

void MenuDefinition_ParseAndAddLoadCommand(MenuDefinition *self, char *argText)
{
    NamedCommand *loadCommand = MenuDefinition_FindNamedCommandByName(self, argText);
    if(loadCommand)
    {
        self->hasLoadCommand = TRUE;
        menu_definition_set_load_command(self, loadCommand);
    }
}

void MenuDefinition_ParseAndSetRange(MenuDefinition *self, char *argText)
{
    char *nextToken = NULL;
    char* startStr = strtok_s(argText, ",", &nextToken);
    char* endStr = strtok_s(NULL, ",", &nextToken);
    self->hasReturnRange = TRUE;
    self->returnRangeStart = atoi(startStr);
    self->returnRangeEnd = atoi(endStr);
}

NamedCommand *MenuDefinition_FindOrAddNamedCommand(MenuDefinition *self, CHAR *nameBuff)
{
    NamedCommand *command = MenuDefinition_FindNamedCommandByName(self, nameBuff);
    if(!command)
    {
        command = calloc(1, sizeof(NamedCommand));
        command->name = _strdup(nameBuff);

        if(!self->namedCommands)
        {
            self->namedCommands = command;
            self->lastNamedCommand = command;
        }
        else
        {
            self->lastNamedCommand->next = command;
            self->lastNamedCommand = command;
        }
    }

    return command;
}

NamedCommand *MenuDefinition_AddNamedCommand(MenuDefinition *self, char *argText, BOOL reloadAfter, BOOL quitAfter)
{
    char nameBuff[BUF_LEN];
    char expressionBuff[BUF_LEN];

    BOOL parsingName = TRUE;

    int indexOfReplacement = 0;
    BOOL hasReplacement = FALSE;

    size_t len = strlen(argText);
    if(len > BUF_LEN)
    {
        len = BUF_LEN;
    }

    int expressionCtr = 0;
    for(int i = 0; i < len; i++)
    {
        if(argText[i] == ':' && parsingName)
        {
            nameBuff[i] = '\0';
            parsingName = FALSE;
        }
        else if(argText[i] == '{' && i + 1 < len && argText[i + 1] == '}')
        {
            hasReplacement = TRUE;
            indexOfReplacement = expressionCtr;
            expressionBuff[expressionCtr] = argText[i];
            expressionCtr++;
            i++;
            expressionBuff[expressionCtr] = argText[i];
            expressionCtr++;
        }
        else if(parsingName)
        {
            nameBuff[i] = argText[i];
        }
        else
        {
            expressionBuff[expressionCtr] = argText[i];
            expressionCtr++;
        }
    }

    expressionBuff[expressionCtr] = '\0';

    NamedCommand *command = MenuDefinition_FindOrAddNamedCommand(self, nameBuff);
    command->expression = _strdup(expressionBuff);
    command->hasReplacement = hasReplacement;
    command->indexOfReplacement = indexOfReplacement;
    command->expressionLen = expressionCtr;
    command->quitAfter = quitAfter;
    command->reloadAfter = reloadAfter;
    command->hasTextRange = FALSE;

    return command;
}

void NamedCommand_SetTextRange(NamedCommand *self, int start, int end, BOOL trimEnd)
{
    self->hasTextRange = TRUE;
    self->textRangeStart = start;
    self->textRangeEnd = end;
    self->trimEnd = trimEnd;
}

NamedCommand* MenuDefinition_AddActionNamedCommand(MenuDefinition *self, CHAR *nameBuff, void (*action)(CHAR *text), BOOL reloadAfter, BOOL quitAfter)
{
    NamedCommand *command = MenuDefinition_FindOrAddNamedCommand(self, nameBuff);
    command->quitAfter = quitAfter;
    command->reloadAfter = reloadAfter;
    command->action = action;
    command->hasTextRange = TRUE;
    return command;
}

NamedCommand* MenuDefinition_AddAction2NamedCommand(MenuDefinition *self, CHAR *nameBuff, void *state, void (*action)(void *state), BOOL reloadAfter, BOOL quitAfter)
{
    NamedCommand *command = MenuDefinition_FindOrAddNamedCommand(self, nameBuff);
    command->quitAfter = quitAfter;
    command->reloadAfter = reloadAfter;
    command->action2 = action;
    command->action2State = state;
    command->hasTextRange = TRUE;
    return command;
}

MenuView *menu_create(int left, int top, int width, int height, TCHAR *title)
{
    MenuView *menuView = calloc(1, sizeof(MenuView));

    ItemsView *itemsView = calloc(1, sizeof(ItemsView));
    itemsView->maxDisplayItems = 50;
    itemsView->hasHeader = FALSE;
    itemsView->fzfSlab = fzf_make_default_slab();
    itemsView->isLoading = FALSE;

    SearchView *searchView = calloc(1, sizeof(SearchView));
    searchView->itemsView = itemsView;
    searchView->isSearching = FALSE;

    itemsView->searchView = searchView;

    menuView->itemsView = itemsView;
    menuView->searchView = searchView;
    menuView->searchView->itemsView = itemsView;

    InitializeCriticalSectionAndSpinCount(&searchView->searchCriticalSection, 0x00000400);
    InitializeCriticalSectionAndSpinCount(&searchView->cancelSearchCriticalSection, 0x00000400);
    searchView->searchEvent = CreateEvent(
            NULL,
            TRUE,
            TRUE,
            L"searchEvent");
    SetEvent(searchView->searchEvent);

    InitializeCriticalSectionAndSpinCount(&itemsView->loadCriticalSection, 0x00000400);
    itemsView->loadEvent = CreateEvent(
            NULL,
            TRUE,
            TRUE,
            L"loadEvent");
    SetEvent(itemsView->loadEvent);

    backgrounBrush = CreateSolidBrush(backgroundColor);
    highlightedBackgroundBrush = CreateSolidBrush(highlightBackgroundColor);

    HDC hdc = GetDC(NULL);
    long lfHeight;
    lfHeight = -MulDiv(14, GetDeviceCaps(hdc, LOGPIXELSY), 72);
    font = CreateFontW(lfHeight, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, TEXT("Hack Regular Nerd Font Complete"));
    DeleteDC(hdc);

    const TCHAR ClassName[] = L"SimpleMenuClass";

    HINSTANCE hInstance = GetModuleHandle(0);
    WNDCLASSEX wc;

    wc.cbSize = sizeof(WNDCLASSEX);
    wc.style = 0;
    wc.lpfnWndProc = (WNDPROC)Menu_MessageProcessor;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = hInstance;
    wc.hIcon = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_ICON));
    wc.hIconSm = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_ICON));
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.hbrBackground = highlightedBackgroundBrush;
    wc.lpszMenuName = NULL;
    wc.lpszClassName = ClassName;

    RegisterClassEx(&wc);

    menuView->hwnd = CreateWindowEx(
            WS_EX_APPWINDOW,
            ClassName,
            title,
            (DWORD) ~ (WS_CHILD | WS_CAPTION | WS_THICKFRAME | WS_MINIMIZE | WS_MAXIMIZE | WS_SYSMENU | WS_DISABLED | WS_BORDER | WS_DLGFRAME | WS_SIZEBOX | WS_VSCROLL | WS_HSCROLL),
            left,
            top,
            width,
            height,
            NULL,
            NULL,
            hInstance,
            menuView);

    return menuView;
}

MenuDefinition* menu_definition_create(MenuView *menuView)
{
    MenuDefinition *result = calloc(1, sizeof(MenuDefinition));
    MenuDefinition_AddNamedCommand(result, "copytoclipboard:cmd /c echo {} | clip", FALSE, FALSE);
    MenuDefinition_ParseAndAddKeyBinding(result, "ctl-c:copytoclipboard", FALSE);

    NamedCommand *exitCommand = MenuDefinition_AddAction2NamedCommand(result, "exit", menuView->searchView, SearchView_HandleEscape, FALSE, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, exitCommand, 0, VK_ESCAPE, FALSE);

    NamedCommand *selectCommand = MenuDefinition_AddAction2NamedCommand(result, "select", menuView->itemsView, ItemsView_HandleSelection, FALSE, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, selectCommand, 0, VK_RETURN, FALSE);

    NamedCommand *helpCommand = MenuDefinition_AddAction2NamedCommand(result, "show help", menuView->searchView, SearchView_ShowHelp, FALSE, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, helpCommand, VK_CONTROL, VK_OEM_PLUS, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, helpCommand, VK_CONTROL, VK_OEM_2, FALSE);

    NamedCommand *itemDownCommand = MenuDefinition_AddAction2NamedCommand(result, "select next item", menuView->itemsView, ItemsView_SelectNext, FALSE, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, itemDownCommand, VK_CONTROL, VK_N, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, itemDownCommand, 0, VK_DOWN, FALSE);

    NamedCommand *itemUpCommand = MenuDefinition_AddAction2NamedCommand(result, "select previous item", menuView->itemsView, ItemsView_SelectPrevious, FALSE, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, itemUpCommand, VK_CONTROL, VK_P, FALSE);
    MenuDefinition_AddKeyBindingToNamedCommand(result, itemUpCommand, 0, VK_UP, FALSE);

    return result;
}

void menu_definition_set_load_action(MenuDefinition *self, int (*loadAction)(int maxItems, CHAR** items))
{
    self->itemsAction = loadAction;
    MenuDefinition_AddLoadActionKeyBinding(self, VK_CONTROL, VkKeyScanW('r'), self->itemsAction);
}

void menu_definition_set_load_command(MenuDefinition *self, NamedCommand *loadCommand)
{
    self->loadCommand = loadCommand;
    MenuDefinition_AddKeyBindingToNamedCommand(self, self->loadCommand, VK_CONTROL, VkKeyScanW('r'), TRUE);
}

void menu_run_definition(MenuView *self, MenuDefinition *menuDefinition)
{
    self->itemsView->hasHeader = menuDefinition->hasHeader;
    self->itemsView->hasLoadCommand = menuDefinition->hasLoadCommand;
    self->itemsView->loadCommand = menuDefinition->loadCommand;
    self->itemsView->loadAction = menuDefinition->itemsAction;
    self->itemsView->namedCommands = menuDefinition->namedCommands;
    self->itemsView->hasReturnRange = menuDefinition->hasReturnRange;
    self->itemsView->returnRangeStart = menuDefinition->returnRangeStart;
    self->itemsView->returnRangeEnd = menuDefinition->returnRangeEnd;
    self->itemsView->onSelection = menuDefinition->onSelection;
    self->searchView->onEscape = menuDefinition->onEscape;
    self->searchView->keyBindings = menuDefinition->keyBindings;

    SetWindowTextA(self->searchView->hwnd, "");

    if(self->itemsView->hasLoadCommand)
    {
        ItemsView_ReloadFromCommand(self->itemsView, self->itemsView->loadCommand);
    }
    else if(menuDefinition->itemsAction)
    {
        ItemsView_LoadFromAction(self->itemsView, menuDefinition->itemsAction);
    }

    MenuView_FitChildControls(self);
}
