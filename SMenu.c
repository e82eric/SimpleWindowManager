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

#define VK_P 0x50
#define VK_R 0x52
#define VK_N 0x4E
#define VK_K 0x4B
#define VK_C 0x43

#define CHUNK_SIZE 20000

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
VOID CALLBACK ItemsView_ProcessChunkWorker2(PTP_CALLBACK_INSTANCE Instance, PVOID lpParam, PTP_WORK Work);
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
    WaitForSingleObject(self->cancelSearchEvent, 50000);
    ResetEvent(self->cancelSearchEvent);
    self->cancelSearch = FALSE;
    LeaveCriticalSection(&self->cancelSearchCriticalSection);

    self->itemsView->isSearching = TRUE;

    ProcessChunkJob *jobs[250];
    int numberOfJobs = 0;

    CHAR buff[BUF_LEN];
    GetWindowTextA(self->hwnd, buff, BUF_LEN);

    self->itemsView->fzfPattern = fzf_parse_pattern(CaseSmart, false, buff, true);
    PTP_WORK completeWorkHandles[50];
    Chunk *currentChunk = self->itemsView->chunks;
    while(currentChunk)
    {
        jobs[numberOfJobs] = calloc(1, sizeof(ProcessChunkJob));
        jobs[numberOfJobs]->fzfSlab = fzf_make_default_slab();
        jobs[numberOfJobs]->fzfPattern = fzf_parse_pattern(CaseSmart, false, buff, true);
        jobs[numberOfJobs]->chunk = currentChunk;
        jobs[numberOfJobs]->allItemsWithScores = calloc(CHUNK_SIZE, sizeof(ItemMatchResult*));
        jobs[numberOfJobs]->jobNumber = numberOfJobs;
        jobs[numberOfJobs]->searchString = _strdup(buff);

        jobs[numberOfJobs]->displayItemList = calloc(1, sizeof(DisplayItemList));
        jobs[numberOfJobs]->displayItemList->items = calloc(self->itemsView->maxDisplayItems, sizeof(ItemMatchResult*));
        jobs[numberOfJobs]->displayItemList->maxItems = self->itemsView->maxDisplayItems;
        jobs[numberOfJobs]->searchView = self;

        completeWorkHandles[numberOfJobs] = CreateThreadpoolWork(ItemsView_ProcessChunkWorker2, jobs[numberOfJobs], NULL);
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

    SendMessageA(self->itemsView->hwnd, WM_SETREDRAW, FALSE, 0);
    SendMessageA(self->itemsView->hwnd, LB_RESETCONTENT, 0, 0);
    for(int i = 0; i < mergedDisplayItemList->count; i++)
    {
        if(self->cancelSearch)
        {
            break;
        }
        SendMessageA(self->itemsView->hwnd, LB_INSERTSTRING, i, (LPARAM)mergedDisplayItemList->items[i]->item->text);
    }

    for(int i = 0; i < numberOfJobs; i++)
    {
        ProcessChunkJob *job = jobs[i];
        for(int j = 0; j < job->numberOfMatches; j++)
        {
            free(jobs[i]->allItemsWithScores[j]);
        }
        free(jobs[i]->allItemsWithScores);
    }

    LRESULT lResult = SendMessageA(self->itemsView->hwnd, LB_FINDSTRING, 0, (LPARAM)self->itemsView->selectedString);
    if(lResult == LB_ERR)
    {
        LRESULT newSelection = SendMessageA(self->itemsView->hwnd, LB_SETCURSEL, 0, 0);
        char achBuffer[BUF_LEN];
        SendMessageA(self->itemsView->hwnd, LB_GETTEXT, newSelection, (LPARAM)achBuffer); 
        self->itemsView->selectedString = _strdup(achBuffer);
    }
    else
    {
        SendMessageA(self->itemsView->hwnd, LB_SETCURSEL, lResult, 0);
    }

    if(!self->cancelSearch)
    {
        SendMessageA(self->itemsView->hwnd, WM_SETREDRAW, TRUE, 0);
    }

    for(int i = 0; i < numberOfJobs; i++)
    {
        fzf_free_slab(jobs[i]->fzfSlab);
        fzf_free_pattern(jobs[i]->fzfPattern);
        free(jobs[i]->displayItemList->items);
        free(jobs[i]);
    }

    free(mergedDisplayItemList->items);
    free(mergedDisplayItemList);

    SetEvent(self->cancelSearchEvent);
    self->itemsView->isSearching = FALSE;
    return TRUE;
}

void TriggerSearch(SearchView *self)
{
    DWORD dwThreadIdArray[1];
    CreateThread( 
            NULL,
            0,
            SearchView_Worker,
            self,
            0,
            &dwThreadIdArray[0]);
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

    self->searchView->onEscape();
    if(self->onSelection)
    {
        self->onSelection(result);
    }

    /* ItemsView_Clear(self->searchView->itemsView); */
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
        self->selectedString = _strdup(achBuffer);
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
        self->selectedString = _strdup(achBuffer);
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
}

void ItemsView_StartLoadItems_FromCommand(ItemsView *self, NamedCommand *namedCommand)
{
    UNREFERENCED_PARAMETER(self);
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = self;

    ProcessWithItemStreamOutput_Start(namedCommand->expression, job);
}

void ItemsView_StartLoadItems(ItemsView *self)
{
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = self;

    if(self->loadCommand)
    {
        ProcessWithItemStreamOutput_Start(self->loadCommand->expression, job);
    }
}

void ItemsView_Clear(ItemsView *self)
{
    SendMessageA(self->hwnd, LB_SETCURSEL, 0, -1);
    SendMessage(self->hwnd, LB_RESETCONTENT, 0, 0);;

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
}

void ItemsView_LoadFromAction(ItemsView *self, int (itemsAction)(CHAR **items))
{
    ItemsView_Clear(self);

    ItemsView_SetLoading(self);
    self->chunks = calloc(1, sizeof(Chunk));
    self->chunks->items = calloc(CHUNK_SIZE, sizeof(Item*));

    CHAR *itemsBuf[CHUNK_SIZE];
    int numberOfItems = itemsAction(itemsBuf);

    for(int i = 0; i < numberOfItems; i++)
    {
        ItemsView_AddToEnd(self, itemsBuf[i]);
    }

    TriggerSearch(self->searchView);
    ItemsView_SetDoneLoading(self);
}

void ItemsView_ReloadFromCommand(ItemsView *self, NamedCommand *command)
{
    ItemsView_Clear(self);

    ItemsView_StartLoadItems_FromCommand(self, command);
}

void ItemsView_HandleReload(ItemsView *self)
{
    if(self->hasLoadCommand)
    {
        SendMessageA(self->hwnd, LB_SETCURSEL, 0, -1);
        SendMessage(self->hwnd, LB_RESETCONTENT, 0, 0);;

        Chunk *currentChunk = self->chunks;
        while(currentChunk)
        {
            for(int i = 0; i < currentChunk->count; i++)
            {
                free(currentChunk->items[i]->text);
                free(currentChunk->items[i]);
            }

            Chunk *temp = currentChunk;
            currentChunk = currentChunk->next;
            free(temp);
        }

        self->chunks = calloc(1, sizeof(Chunk));
        self->chunks->items = calloc(CHUNK_SIZE, sizeof(Item*));

        ItemsView_StartLoadItems(self);
    }
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
        ItemsView_Clear(asyncState->itemsView);
        asyncState->itemsView->searchView->onEscape();
    }
}

void ItemsView_HandleBinding(ItemsView *self, NamedCommand *command, BOOL reloadAfter, BOOL quitAfter)
{
    if(self->namedCommands)
    {
        if(command)
        {
            LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
            char newBindCommand[BUF_LEN];
            CHAR curSelStr[BUF_LEN] = "";
            SendMessageA(self->hwnd, LB_GETTEXT, dwSel, (LPARAM)curSelStr);

            if(self->hasReturnRange)
            {
                CHAR selSubStr[BUF_LEN];

                sprintf_s(selSubStr, BUF_LEN, "%.*s\n", self->returnRangeEnd, curSelStr + self->returnRangeStart);

                NamedCommand_Parse(newBindCommand, selSubStr, command, BUF_LEN);
                ItemsView_StartBindingProcess(self, newBindCommand, BindProcessFinishCallback, reloadAfter, quitAfter);
            }
            else
            {
                NamedCommand_Parse(newBindCommand, curSelStr, command, BUF_LEN);
                ItemsView_StartBindingProcess(self, newBindCommand, BindProcessFinishCallback, reloadAfter, quitAfter);
            }
        }
    }
}

LRESULT CALLBACK SearchView_MessageProcessor(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);

    SearchView *self = (SearchView*)dwRefData;

    switch (uMsg)
    {
        case WM_CHAR:
            {
                if(wParam == VK_ESCAPE)
                {
                    self->onEscape();
                    /* PostQuitMessage(0); */
                    return 0;
                }
                if(wParam == VK_RETURN)
                {
                    ItemsView_HandleSelection(self->itemsView);
                    return 0;
                }
            }
            break;
    }

    if((uMsg == WM_KEYDOWN) && (wParam == VK_N))
    {
        if(GetAsyncKeyState(VK_CONTROL))
        {
            ItemsView_SelectNext(self->itemsView);
            return 1;
        }
    }
    if((uMsg == WM_KEYDOWN) && (wParam == VK_P))
    {
        if(GetAsyncKeyState(VK_CONTROL))
        {
            ItemsView_SelectPrevious(self->itemsView);
            return 1;
        }
    }
    if((uMsg == WM_KEYDOWN) && (wParam == VK_R))
    {
        if(GetAsyncKeyState(VK_LCONTROL))
        {
            ItemsView_HandleReload(self->itemsView);
            return 1;
        }
    }

    if((uMsg == WM_KEYDOWN))
    {
        MenuKeyBinding *current = self->keyBindings;
        while(current)
        {
            if(wParam == current->key)
            {
                if(GetAsyncKeyState(current->modifier) & 0x8000)
                {
                    if(current->isLoadCommand)
                    {
                        ItemsView_ReloadFromCommand(self->itemsView, current->command);
                    }
                    else
                    {
                        ItemsView_HandleBinding(self->itemsView, current->command, current->reloadAfter, current->quitAfter);
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
            return 1;
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

VOID CALLBACK ItemsView_ProcessChunkWorker2(PTP_CALLBACK_INSTANCE Instance, PVOID lpParam, PTP_WORK Work)
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

BOOL ProcessCmdOutJob_ProcessStream(ProcessCmdOutputJob *self)
{
    ItemsView_SetLoading(self->itemsView);
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
    ItemsView_SetDoneLoading(self->itemsView);
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
}

void MenuView_CreateChildControls(MenuView *self)
{
    HINSTANCE hinstance = (HINSTANCE)GetWindowLongPtr(self->hwnd, GWLP_HINSTANCE);
    self->searchView->hwnd = CreateWindowA(
            "Edit", 
            NULL, 
            WS_VISIBLE | WS_CHILD,
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

    /* if(self->itemsView->hasHeader) */
    /* { */
        self->itemsView->headerHwnd = CreateWindowA(
                "STATIC", 
                NULL, 
                WS_VISIBLE | WS_CHILD | SS_LEFT | WS_BORDER,
                10, 
                10, 
                600, 
                10,
                self->hwnd, 
                (HMENU)IDC_STATIC_TEXT,
                hinstance,
                NULL);
        SendMessageA(self->itemsView->headerHwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);
    /* } */

    self->itemsView->summaryHwnd = CreateWindow(
            L"STATIC", 
            NULL, 
            WS_VISIBLE | WS_CHILD | SS_RIGHT | SS_OWNERDRAW,
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
            WS_VISIBLE | WS_CHILD | LBS_STANDARD | LBS_HASSTRINGS | LBS_OWNERDRAWFIXED,
            260, 
            260, 
            900, 
            600,
            self->hwnd, 
            (HMENU)IDC_LISTBOX_TEXT, 
            (HINSTANCE)GetWindowLongPtr(self->hwnd, GWLP_HINSTANCE),
            NULL);
    SendMessageA(self->itemsView->hwnd, WM_SETFONT, (WPARAM)font, (LPARAM)TRUE);

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

                    if(self->itemsView->fzfPattern)
                    {
                        fzf_position_t *pos = fzf_get_positions(achBuffer, self->itemsView->fzfPattern, self->itemsView->fzfSlab);

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

    MenuKeyBinding *binding = calloc(1, sizeof(MenuKeyBinding));
    if(argText[0] == 'c' && argText[1] == 't' && argText[2] == 'l')
    {
        binding->modifier = VK_CONTROL;
    }
    if(argText[0] == 'a' && argText[1] == 't' && argText[2] == 'l')
    {
        /* binding->modifier = VM_LALT; */
    }

    char nameBuff[BUF_LEN];
    BOOL quitAfter = FALSE;
    BOOL reloadAfter = TRUE;
    for(int i = 6; i < len; i++)
    {
        if(argText[i] == ':')
        {
            nameBuff[i - startOfName] = '\0';
            if(len - i == 5)
            {
                if(argText[len - 4] == 'q' && argText[len - 3] == 'u' && argText[len - 2] == 'i' && argText[len - 1] == 't')
                {
                    quitAfter = TRUE;
                    break;
                }
            }
            else if(len - i == 7)
            {
                if(     argText[len - 6] == 'r' &&
                        argText[len - 5] == 'e' &&
                        argText[len - 4] == 'l' &&
                        argText[len - 3] == 'o' &&
                        argText[len - 2] == 'a' &&
                        argText[len - 1] == 'd')
                {
                    reloadAfter = TRUE;
                    break;
                }
            }
        }
        nameBuff[i - startOfName] = argText[i];
    }

    nameBuff[len - startOfName] = '\0';

    NamedCommand *command = MenuDefinition_FindNamedCommandByName(self, nameBuff);
    binding->command = command;
    binding->key = virtualKey;
    binding->reloadAfter = reloadAfter;
    binding->isLoadCommand = isLoadCommand;
    binding->quitAfter = quitAfter;

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

void SearchView_ParseAndAddKeyBinding(SearchView *self, char *argText, BOOL isLoadCommand)
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

    MenuKeyBinding *binding = calloc(1, sizeof(MenuKeyBinding));
    if(argText[0] == 'c' && argText[1] == 't' && argText[2] == 'l')
    {
        binding->modifier = VK_CONTROL;
    }
    if(argText[0] == 'a' && argText[1] == 't' && argText[2] == 'l')
    {
        /* binding->modifier = VM_LALT; */
    }

    char nameBuff[BUF_LEN];
    BOOL quitAfter = FALSE;
    BOOL reloadAfter = TRUE;
    for(int i = 6; i < len; i++)
    {
        if(argText[i] == ':')
        {
            nameBuff[i - startOfName] = '\0';
            if(len - i == 5)
            {
                if(argText[len - 4] == 'q' && argText[len - 3] == 'u' && argText[len - 2] == 'i' && argText[len - 1] == 't')
                {
                    quitAfter = TRUE;
                    break;
                }
            }
            else if(len - i == 7)
            {
                if(     argText[len - 6] == 'r' &&
                        argText[len - 5] == 'e' &&
                        argText[len - 4] == 'l' &&
                        argText[len - 3] == 'o' &&
                        argText[len - 2] == 'a' &&
                        argText[len - 1] == 'd')
                {
                    reloadAfter = TRUE;
                    break;
                }
            }
        }
        nameBuff[i - startOfName] = argText[i];
    }

    nameBuff[len - startOfName] = '\0';

    NamedCommand *command = ItemsView_FindNamedCommandByName(self->itemsView, nameBuff);
    binding->command = command;
    binding->key = virtualKey;
    binding->reloadAfter = reloadAfter;
    binding->isLoadCommand = isLoadCommand;
    binding->quitAfter = quitAfter;

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

void MenuDefinition_ParseAndAddLoadCommand(MenuDefinition *self, char *argText)
{
    NamedCommand *loadCommand = MenuDefinition_FindNamedCommandByName(self, argText);
    if(loadCommand)
    {
        self->hasLoadCommand = TRUE;
        self->loadCommand = loadCommand;
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

void MenuDefinition_AddNamedCommand(MenuDefinition *self, char *argText)
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

    NamedCommand *command = calloc(1, sizeof(NamedCommand));
    command->name = _strdup(nameBuff);
    command->expression = _strdup(expressionBuff);
    command->hasReplacement = hasReplacement;
    command->indexOfReplacement = indexOfReplacement;
    command->expressionLen = expressionCtr;

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

void ItemsView_AddNamedCommand(ItemsView *self, char *argText)
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

    NamedCommand *command = calloc(1, sizeof(NamedCommand));
    command->name = _strdup(nameBuff);
    command->expression = _strdup(expressionBuff);
    command->hasReplacement = hasReplacement;
    command->indexOfReplacement = indexOfReplacement;
    command->expressionLen = expressionCtr;

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

MenuView *menu_create(int left, int top, int width, int height, TCHAR *title)
{
    MenuView *menuView = calloc(1, sizeof(MenuView));

    NamedCommand *copyCommand = calloc(1, sizeof(NamedCommand));
    copyCommand->name = "copytoclipboard";
    copyCommand->expression = "cmd /c echo {} | clip"; 
    copyCommand->expressionLen = 21;
    copyCommand->hasReplacement = TRUE;
    copyCommand->indexOfReplacement = 12;
    copyCommand->next = NULL;

    MenuKeyBinding *copyKeyBinding = calloc(1, sizeof(MenuKeyBinding));
    copyKeyBinding->modifier = VK_LCONTROL;
    copyKeyBinding->key = VK_C;
    copyKeyBinding->command = copyCommand;
    copyKeyBinding->next = NULL;
    copyKeyBinding->reloadAfter = FALSE;

    ItemsView *itemsView = calloc(1, sizeof(ItemsView));
    itemsView->maxDisplayItems = 50;
    itemsView->hasHeader = FALSE;
    itemsView->chunks = calloc(1, sizeof(Chunk));
    itemsView->chunks->items = calloc(CHUNK_SIZE, sizeof(Item*));
    itemsView->namedCommands = copyCommand;
    itemsView->lastNamedCommand = copyCommand;

    SearchView *searchView = calloc(1, sizeof(SearchView));
    searchView->itemsView = itemsView;
    searchView->keyBindings = copyKeyBinding;
    searchView->lastKeyBinding = copyKeyBinding;

    itemsView->searchView = searchView;

    menuView->itemsView = itemsView;
    menuView->searchView = searchView;
    menuView->searchView->itemsView = itemsView;

    InitializeCriticalSectionAndSpinCount(&searchView->cancelSearchCriticalSection, 0x00000400);
    searchView->cancelSearchEvent = CreateEvent(
            NULL,
            TRUE,
            TRUE,
            L"CancelSearchEvent");

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

void menu_run_definition(MenuView *self, MenuDefinition *menuDefinition)
{
    self->itemsView->hasHeader = menuDefinition->hasHeader;
    self->itemsView->hasLoadCommand = menuDefinition->hasLoadCommand;
    self->itemsView->loadCommand = menuDefinition->loadCommand;
    self->searchView->keyBindings = menuDefinition->keyBindings;
    self->itemsView->namedCommands = menuDefinition->namedCommands;
    self->itemsView->hasReturnRange = menuDefinition->hasReturnRange;
    self->itemsView->returnRangeStart = menuDefinition->returnRangeStart;
    self->itemsView->returnRangeEnd = menuDefinition->returnRangeEnd;
    self->itemsView->onSelection = menuDefinition->onSelection;
    self->searchView->onEscape = menuDefinition->onEscape;

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