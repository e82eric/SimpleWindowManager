#include <assert.h>
#include <windows.h>
#include <stdio.h>
#include <strsafe.h>
#include "fzf\\fzf.h"
#include <assert.h>
#include <synchapi.h>
#include <commctrl.h>

#define BUF_LEN 4096
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

const TCHAR ClassName[] = L"SimpleMenuClass";

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

typedef struct NamedCommand NamedCommand;
struct NamedCommand
{
    char* name;
    char* expression;
    int expressionLen;
    BOOL hasReplacement;
    int indexOfReplacement;
    NamedCommand *next;
};

typedef struct KeyBinding KeyBinding;
struct KeyBinding
{
    unsigned int modifier;
    unsigned int key;
    NamedCommand* command;
    KeyBinding* next;
};

typedef struct ItemsView ItemsView;
typedef struct ProcessAsyncState ProcessAsyncState;
struct ProcessAsyncState
{
    ItemsView *itemsView;
    HANDLE waitHandle;
    HANDLE readFileHandle;
    HANDLE processId;
    HANDLE thread;
};

typedef struct Item Item;
struct Item
{
    CHAR *text;
    Item *next;
};

typedef struct ItemMatchResult
{
    Item *item;
    int score;
} ItemMatchResult;

typedef struct Chunk Chunk;
struct Chunk
{
    Item **items;
    int count;
    Chunk *next;
};

typedef struct DisplayItemList
{
    int maxItems;
    int count;
    int minScore;
    ItemMatchResult **items;
} DisplayItemList;

typedef struct ProcessChunkJob
{
    Chunk *chunk;
    DisplayItemList *displayItemList;
    ItemMatchResult **allItemsWithScores;
    int numberOfMatches;
    fzf_pattern_t *fzfPattern;
    fzf_slab_t *fzfSlab;
    int jobNumber;
    char* searchString;
} ProcessChunkJob;

typedef struct SearchView
{
    HWND hwnd;
    ItemsView *itemsView;
    KeyBinding *keyBindings;
    KeyBinding *lastKeyBinding;
} SearchView;

struct ItemsView
{
    HWND hwnd;
    HWND headerHwnd;
    HWND summaryHwnd;
    int height;
    int width;
    BOOL hasHeader;
    BOOL headerSet;
    BOOL hasLoadCommand;
    char *loadCommand;
    int maxDisplayItems;
    int numberOfItemsMatched;
    BOOL isReading;
    BOOL isSearching;
    fzf_slab_t *fzfSlab;
    fzf_pattern_t *fzfPattern;
    UINT numberOfItems;
    Chunk *chunks;
    NamedCommand *namedCommands;
    NamedCommand *lastNamedCommand;
    char* selectedString;
};

typedef struct ProcessCmdOutputJob
{
    HANDLE readHandle;
    ItemsView *itemsView;
} ProcessCmdOutputJob;

typedef struct MenuView
{
    HWND hwnd;
    int height;
    int width;
    SearchView *searchView;
    ItemsView *itemsView;
} MenuView;

void ItemsView_SetLoading(ItemsView *self);
void ItemsView_SetDoneLoading(ItemsView *self);
VOID CALLBACK ItemsView_ProcessChunkWorker2( PTP_CALLBACK_INSTANCE Instance, PVOID lpParam, PTP_WORK Work);
void DisplayItemList_MergeIntoRight(DisplayItemList *self, DisplayItemList *resultList2);
void ItemsView_SetHeader(ItemsView *self, char* headerLine);
NamedCommand* ItemsView_FindNamedCommandByName(ItemsView *self, char *name);
DWORD WINAPI ProcessCmdOutputJobWorker(LPVOID lpParam);
void ProcessWithItemStreamOutput_Start(char *cmdArgs, ProcessCmdOutputJob *job);

MenuView *menuView;
BOOL g_cancelSearch;
HANDLE cancelSearchEvent;
CRITICAL_SECTION CriticalSection; 

DWORD WINAPI SearchView_Worker(LPVOID lpParam)
{
    UNREFERENCED_PARAMETER(lpParam);

    SYSTEMTIME st;
    GetSystemTime(&st);

    EnterCriticalSection(&CriticalSection); 
    g_cancelSearch = TRUE;
    WaitForSingleObject(cancelSearchEvent, 50000);
    ResetEvent(cancelSearchEvent);
    g_cancelSearch = FALSE;
    LeaveCriticalSection(&CriticalSection);

    SearchView *self = menuView->searchView;
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
        DisplayItemList_MergeIntoRight(jobs[i]->displayItemList, mergedDisplayItemList);
    }
    self->itemsView->numberOfItemsMatched = totalMatches;

    SendMessageA(self->itemsView->hwnd, WM_SETREDRAW, FALSE, 0);
    SendMessageA(self->itemsView->hwnd, LB_RESETCONTENT, 0, 0);
    for(int i = 0; i < mergedDisplayItemList->count; i++)
    {
        if(g_cancelSearch)
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

    if(!g_cancelSearch)
    {
        SendMessageA(self->itemsView->hwnd, WM_SETREDRAW, TRUE, 0);
    }

    for(int i = 0; i < numberOfJobs; i++)
    {
        fzf_free_slab(jobs[i]->fzfSlab);
        fzf_free_pattern(jobs[i]->fzfPattern);
        /* for(int j = 0; j = jobs[i]->displayItemList->count; j++) */
        /* { */
        /*     free(jobs[i]->displayItemList->items[j]); */
        /* } */
        free(jobs[i]->displayItemList->items);
        free(jobs[i]);
    }

    /* for(int i = 0; i < mergedDisplayItemList->count; i++) */
    /* { */
    /*     free(mergedDisplayItemList->items[i]); */
    /* } */

    free(mergedDisplayItemList->items);
    free(mergedDisplayItemList);

    SetEvent(cancelSearchEvent);
    self->itemsView->isSearching = FALSE;
    return TRUE;
}

void TriggerSearch(void)
{
    DWORD dwThreadIdArray[1];
    CreateThread( 
            NULL,
            0,
            SearchView_Worker,
            0,
            0,
            &dwThreadIdArray[0]);
}

LRESULT CALLBACK Summary_MessageProcessor(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(dwRefData);
    UNREFERENCED_PARAMETER(uIdSubclass);

    switch (uMsg)
    {
         case WM_PAINT:
             static int spinnerCtr = 0;
             wint_t spinner[10] = { 0x280B, 0x2819, 0x2839, 0x2838,0x283C,0x2834,0x2827,0x2807,0x280F };
             wchar_t countBuffer[BUF_LEN];
             wchar_t spinnerBuffer[10];

             swprintf(spinnerBuffer, 10, L"%lc ", spinner[spinnerCtr]);
             swprintf(countBuffer, 256, L"%d/%d", menuView->itemsView->numberOfItemsMatched, menuView->itemsView->numberOfItems);
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
             if(menuView->itemsView->isReading || menuView->itemsView->isSearching)
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
    HANDLE stdOut = GetStdHandle(STD_OUTPUT_HANDLE);
    if(stdOut != NULL && stdOut != INVALID_HANDLE_VALUE)
    {
        LRESULT dwSel = SendMessageA(self->hwnd, LB_GETCURSEL, 0, 0);
        if(dwSel != LB_ERR)
        {
            CHAR str[BUF_LEN] = "";
            SendMessageA(self->hwnd, LB_GETTEXT, dwSel, (LPARAM)str);
            printf("%s", str);
        }
    }
    PostQuitMessage(0);
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

void ItemsView_StartBindingProcess(ItemsView *itemsView, char *cmdArgs, WAITORTIMERCALLBACK callback)
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
        0,
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

void ItemsView_StartLoadItems(ItemsView *self)
{
    ProcessCmdOutputJob *job = calloc(1, sizeof(ProcessCmdOutputJob));
    job->itemsView = menuView->itemsView;

    NamedCommand *command = ItemsView_FindNamedCommandByName(self, self->loadCommand);
    if(command)
    {
        ProcessWithItemStreamOutput_Start(command->expression, job);
    }
}

void ItemsView_HandleReload(ItemsView *self)
{
    if(self->hasLoadCommand)
    {
        SendMessageA(menuView->itemsView->hwnd, LB_SETCURSEL, 0, -1);
        SendMessage(menuView->itemsView->hwnd, LB_RESETCONTENT, 0, 0);;

        Chunk *currentChunk = self->chunks;
        while(currentChunk)
        {
            for(int i = 0; i < currentChunk->count; i++)
            {
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

    ItemsView_HandleReload(asyncState->itemsView);
    UnregisterWait(asyncState->waitHandle);
    CloseHandle(asyncState->thread);
    CloseHandle(asyncState->processId);
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
            NamedCommand_Parse(newBindCommand, curSelStr, command, BUF_LEN);
            ItemsView_StartBindingProcess(self, newBindCommand, BindProcessFinishCallback);
        }
    }
}

LRESULT CALLBACK SearchView_MessageProcessor(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam, UINT_PTR uIdSubclass, DWORD_PTR dwRefData)
{
    UNREFERENCED_PARAMETER(uIdSubclass);
    UNREFERENCED_PARAMETER(dwRefData);

    SearchView *self = (SearchView*)dwRefData;

    switch (uMsg)
    {
        case WM_CHAR:
            {
                if(wParam == VK_ESCAPE)
                {
                    PostQuitMessage(0);
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
        KeyBinding *current = self->keyBindings;
        while(current)
        {
            if(wParam == current->key)
            {
                if(GetAsyncKeyState(current->modifier) & 0x8000)
                {
                    ItemsView_HandleBinding(self->itemsView, current->command);
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
        if(g_cancelSearch)
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

void DisplayItemList_MergeIntoRight(DisplayItemList *self, DisplayItemList *resultList2)
{
    for(int i = 0; i < self->count; i++)
    {
        ItemMatchResult *matchResult = self->items[i];
        if(g_cancelSearch)
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

    if(current->count + 1 == CHUNK_SIZE)
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

    self->itemsView->numberOfItems = 0;
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
                    TriggerSearch();
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

    TriggerSearch();
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
        0,
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
        int headerTop = top;
        int headerHeight = 30;
        MoveWindow(self->headerHwnd, left, headerTop, width, headerHeight, TRUE);

        listHeight = height - headerHeight;
        listTop = headerHeight + headerTop;
    }
    else
    {
        listTop = top;
        listHeight = height;
    }

    MoveWindow(self->hwnd, left, listTop, width, listHeight, TRUE);
    self->height = listHeight;
    /* printf("listHeight %d\n", listHeight); */
    self->maxDisplayItems = (listHeight - 3) / listBoxItemHeight;
    /* printf("maxDisplayItems %d\n", self->maxDisplayItems); */
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
    SetFocus(menuView->searchView->hwnd);
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

    if(menuView->itemsView->hasHeader)
    {
        menuView->itemsView->headerHwnd = CreateWindowA(
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
    }

    menuView->itemsView->summaryHwnd = CreateWindow(
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
    SetWindowSubclass(self->itemsView->summaryHwnd, Summary_MessageProcessor, 0, (DWORD_PTR)0);

    menuView->itemsView->hwnd = CreateWindowA(
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

    SetTimer(menuView->hwnd,
            IDT_LOADINGTIMER,
            75,
            (TIMERPROC) NULL);
    SetFocus(self->searchView->hwnd);
}

LRESULT CALLBACK Menu_MessageProcessor(
        HWND hWnd,
        UINT uMsg,
        WPARAM wParam,
        LPARAM lParam )
{
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
                if((HWND)lParam == menuView->itemsView->headerHwnd)
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
                menuView->height = HIWORD(lParam);
                menuView->width = LOWORD(lParam);
                MenuView_FitChildControls(menuView);
            }
            break;
        case WM_CREATE:
            {
                if(menuView->hwnd == 0)
                {
                    menuView->hwnd = hWnd;
                }
                MenuView_CreateChildControls(menuView);
                break;
            }
        case WM_MOVE:
            {
                /* SetFocus(menuView->searchView->hwnd); */
                /* MenuView_FitChildControls(menuView); */
            }
            break;
        case WM_DESTROY:
            {
                PostQuitMessage(0);
                return 0;
            }
        case WM_TIMER:
            switch (wParam)
            {
                case IDT_LOADINGTIMER:
                    SetFocus(menuView->searchView->hwnd);
                    InvalidateRect(menuView->itemsView->summaryHwnd, NULL, FALSE);
            }
        case WM_COMMAND:
            {
                switch(LOWORD(wParam))
                {
                    case IDC_EDIT_TEXT:
                        {
                            if(HIWORD(wParam) == EN_CHANGE)
                            {
                                TriggerSearch();
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

                    ItemsView *self = menuView->itemsView;
                    if(self->fzfPattern)
                    {
                        fzf_position_t *pos = fzf_get_positions(achBuffer, self->fzfPattern, self->fzfSlab);

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

void SearchView_ParseAndAddKeyBinding(SearchView *self, char *argText)
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

    KeyBinding *binding = calloc(1, sizeof(KeyBinding));
    if(argText[0] == 'c' && argText[1] == 't' && argText[2] == 'l')
    {
        binding->modifier = VK_LCONTROL;
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

    NamedCommand *command = ItemsView_FindNamedCommandByName(self->itemsView, nameBuff);
    binding->command = command;
    binding->key = virtualKey;

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

int main(int argc, char* argv[])
{
    UNREFERENCED_PARAMETER(argc);
    UNREFERENCED_PARAMETER(argv);

    int top = 250;
    int left = 250;
    int height = 940;
    int width = 2050;
    TCHAR title[BUF_LEN];

    menuView = calloc(1, sizeof(MenuView));

    NamedCommand *copyCommand = calloc(1, sizeof(NamedCommand));
    copyCommand->name = "copytoclipboard";
    copyCommand->expression = "cmd /c echo {} | clip"; 
    copyCommand->expressionLen = 21;
    copyCommand->hasReplacement = TRUE;
    copyCommand->indexOfReplacement = 12;
    copyCommand->next = NULL;

    KeyBinding *copyKeyBinding = calloc(1, sizeof(KeyBinding));
    copyKeyBinding->modifier = VK_LCONTROL;
    copyKeyBinding->key = VK_C;
    copyKeyBinding->command = copyCommand;
    copyKeyBinding->next = NULL;

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

    menuView->itemsView = itemsView;
    menuView->searchView = searchView;
    menuView->searchView->itemsView = itemsView;

    for(int i = 0; i < argc; i++)
    {
        if(strcmp(argv[i], "-h") == 0)
            if(i + 1 < argc)
        {
            {
                i++;
                height = atoi(argv[i]);
            }
        }
        if(strcmp(argv[i], "-w") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                width = atoi(argv[i]);
            }
        }
        if(strcmp(argv[i], "-l") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                left = atoi(argv[i]);
            }
        }
        if(strcmp(argv[i], "-t") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                top = atoi(argv[i]);
            }
        }
        if(strcmp(argv[i], "--title") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                int wchars_num = MultiByteToWideChar(CP_UTF8 , 0 , argv[i], -1, NULL , 0 );
                MultiByteToWideChar(CP_UTF8 , 0 , argv[i], -1, title, wchars_num );
            }
        }
        if(strcmp(argv[i], "--loadCommand") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                itemsView->hasLoadCommand = TRUE;
                itemsView->loadCommand = argv[i];
            }
        }
        if(strcmp(argv[i], "--bind") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                SearchView_ParseAndAddKeyBinding(searchView, argv[i]);
            }
        }
        if(strcmp(argv[i], "--namedCommand") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                ItemsView_AddNamedCommand(itemsView, argv[i]);
            }
        }
        if(strcmp(argv[i], "--hasHeader") == 0)
        {
            itemsView->hasHeader = TRUE;
        }
    }

    InitializeCriticalSectionAndSpinCount(&CriticalSection, 0x00000400);
    cancelSearchEvent = CreateEvent(
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

    if (!RegisterClassEx(&wc))
    {
        MessageBox(NULL, L"Failed To Register The Window Class.", L"Error", MB_OK | MB_ICONERROR);
        return 0;
    }

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
            NULL);

    ShowWindow(menuView->hwnd, SW_SHOW);

    if(itemsView->hasLoadCommand)
    {
        ItemsView_StartLoadItems(itemsView);
    }
    else
    {
        ItemsView_StartLoadItemsFromStdIn(itemsView);
    }

    MSG Msg;
    while (GetMessage(&Msg, NULL, 0, 0))
    {
        TranslateMessage(&Msg);
        DispatchMessage(&Msg);
    }

    return 0;
}
