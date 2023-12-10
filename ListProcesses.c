#include <stdio.h>
#include <windows.h>
#include <psapi.h>
#include <locale.h>
#include <tlhelp32.h>

void format_number(WCHAR *buf, size_t toFormat)
{
    static NUMBERFMT fmt = {
        .NumDigits = 0,
        .LeadingZero = 0,
        .NumDigits = 0,
        .LeadingZero = 0,
        .Grouping = 3,
        .lpDecimalSep = L".",
        .lpThousandSep = L",",
        .NegativeOrder = 1,
    };

    WCHAR numStr[MAX_PATH];
    _snwprintf_s(numStr, MAX_PATH, 15, L"%zu", toFormat);
    GetNumberFormat(LOCALE_USER_DEFAULT,
            0,
            numStr,
            &fmt,
            buf,
            MAX_PATH);
}

typedef struct Process
{
    DWORD pid;
    size_t workingSet;
    size_t privateBytes;
    __int64 cpu;
    TCHAR *fileName;
} Process;

void fill_process_stats(Process *process)
{
    PROCESS_MEMORY_COUNTERS_EX pmc;

    HANDLE hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process->pid);

    if(GetProcessMemoryInfo(hProcess, (PROCESS_MEMORY_COUNTERS*)&pmc, sizeof(pmc)))
    {

        FILETIME ftCreation, ftExit, ftKernel, ftUser;
        GetProcessTimes(hProcess, &ftCreation, &ftExit, &ftKernel, &ftUser);

        CloseHandle(hProcess);
        __int64 userTime = (__int64) ftUser.dwHighDateTime << 32 | ftUser.dwLowDateTime;
        __int64 userTimeSec = userTime / 10000000;
        __int64 kernalTime = (__int64) ftKernel.dwHighDateTime << 32 | ftKernel.dwLowDateTime;
        __int64 kernalTimeSec = kernalTime / 10000000;
        __int64 totalSeconds = userTimeSec + kernalTimeSec;

        if(pmc.WorkingSetSize && pmc.PrivateUsage)
        {
            process->workingSet = pmc.WorkingSetSize / 1024;
            process->privateBytes = pmc.PrivateUsage / 1024;
            process->cpu = totalSeconds;
        }
    }
    else
    {
        process->workingSet = 0;
        process->privateBytes = 0;
        process->cpu = 0;
    }
}

int CompareProcessPrivateBytes(const void *a, const void *b)
{
    Process *processA = (Process *)a;
    Process *processB = (Process *)b;

    int result = (int)(processB->privateBytes - processA->privateBytes);
    return result;
}

int CompareProcessCpu(const void *a, const void *b)
{
    Process *processA = (Process *)a;
    Process *processB = (Process *)b;

    int result = (int)(processB->cpu - processA->cpu);
    return result;
}

int CompareProcessWorkingSet(const void *a, const void *b)
{
    Process *processA = (Process *)a;
    Process *processB = (Process *)b;

    int result = (int)(processB->workingSet - processA->workingSet);
    return result;
}

int CompareProcessPid(const void *a, const void *b)
{
    Process *processA = (Process *)a;
    Process *processB = (Process *)b;

    int result = (processB->pid - processA->pid);
    return result;
}

void fill_line_from_process(Process *process, CHAR *lineToFill)
{
    WCHAR privateBytesFormatedStr[MAX_PATH];
    format_number(privateBytesFormatedStr, process->privateBytes);

    WCHAR workingSetFormatedStr[MAX_PATH];
    format_number(workingSetFormatedStr, process->workingSet);

    WCHAR cpuSecondsStrFormated[MAX_PATH];
    format_number(cpuSecondsStrFormated, process->cpu);

    sprintf_s(
            lineToFill,
            1024,
            "%-75.75ls %08lu %20ls %20ls %10ls",
            process->fileName, process->pid, workingSetFormatedStr, privateBytesFormatedStr, cpuSecondsStrFormated);
}

int list_processes_run(int maxItems, CHAR** linesToFill, BOOL sort, int (*sortFunc)(const void *, const void*))
{
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);

    PROCESSENTRY32 pEntry = {0};
    pEntry.dwSize = sizeof(PROCESSENTRY32);

    int numberOfResults = 1;
    CHAR headerBuf[1024];
    sprintf_s(
            headerBuf,
            1024,
            "%-75.75ls %8ls %20ls %20ls %10ls",
            L"Name", L"PID", L"WorkingSet(kb)", L"PrivateBytes(kb)", L"CPU(s)");

    linesToFill[0] = _strdup(headerBuf);
    unsigned int count = 0;
    static Process processes[1024];

    do
    {
        if(pEntry.szExeFile[0] != '\0')
        {
            processes[count].fileName = _wcsdup(pEntry.szExeFile);
            processes[count].pid = pEntry.th32ProcessID;
            fill_process_stats(&processes[count]);
            count++;
        }
    } while(Process32Next(hSnapshot, &pEntry));

    CloseHandle(hSnapshot);

    if(sort)
    {
        qsort(processes, count, sizeof(Process), sortFunc);
    }

    for(unsigned int j = 0; j < count && numberOfResults <= maxItems; j++)
    {
        CHAR lineBuf[1024];
        fill_line_from_process(&processes[j], lineBuf);
        linesToFill[numberOfResults] = _strdup(lineBuf);
        free(processes[j].fileName);
        numberOfResults++;
    }

    return numberOfResults;
}

int list_processes_run_no_sort(int maxItems, CHAR** linesToFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    int numberOfResults = list_processes_run(maxItems, linesToFill, FALSE, NULL);
    return numberOfResults;
}

int list_processes_run_sorted_by_cpu(int maxItems, CHAR **linesToFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    int numberOfResults = list_processes_run(maxItems, linesToFill, TRUE, CompareProcessCpu);
    return numberOfResults;
}

int list_processes_run_sorted_by_private_bytes(int maxItems, CHAR **linesToFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    int numberOfResults = list_processes_run(maxItems, linesToFill, TRUE, CompareProcessPrivateBytes);
    return numberOfResults;
}

int list_processes_run_sorted_by_working_set(int maxItems, CHAR **linesToFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    int numberOfResults = list_processes_run(maxItems, linesToFill, TRUE, CompareProcessWorkingSet);
    return numberOfResults;
}

int list_processes_run_sorted_by_pid(int maxItems, CHAR **linesToFill, void *state)
{
    UNREFERENCED_PARAMETER(state);
    int numberOfResults = list_processes_run(maxItems, linesToFill, TRUE, CompareProcessPid);
    return numberOfResults;
}
