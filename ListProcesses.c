#include <stdio.h>
#include <windows.h>
#include <tchar.h>
#include <psapi.h>
#include <processthreadsapi.h>
#include <shlwapi.h>
#include <locale.h>

#pragma comment(lib, "Advapi32.lib")
#pragma comment(lib, "Shlwapi.lib")

NUMBERFMT numFmt = { 0 };
BOOL sort = FALSE;
int sortColumn = -1;
char *unknownProcessShortName = "<UNKNOWN>";

BOOL sm_EnableTokenPrivilege(LPCTSTR pszPrivilege)
{
    HANDLE hToken = 0;
    TOKEN_PRIVILEGES tkp = {0}; 

    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &hToken))
    {
        return FALSE;
    }

    if(LookupPrivilegeValue(NULL, pszPrivilege, &tkp.Privileges[0].Luid)) 
    {
        tkp.PrivilegeCount = 1;
        tkp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        AdjustTokenPrivileges(hToken, FALSE, &tkp, 0, (PTOKEN_PRIVILEGES)NULL, 0); 

        if (GetLastError() != ERROR_SUCCESS)
        {
           return FALSE;
        }

        return TRUE;
    }

    return FALSE;
}

void FormatNumber(WCHAR *buf, size_t toFormat, NUMBERFMT* fmt)
{
    WCHAR numStr[MAX_PATH];
    _snwprintf_s(numStr, MAX_PATH, 15, L"%zu", toFormat);
    GetNumberFormat(LOCALE_USER_DEFAULT,
            0,
            numStr,
            fmt,
            buf,
            MAX_PATH);
}

typedef struct Process
{
    DWORD pid;
    size_t workingSet;
    size_t privateBytes;
    __int64 cpu;
} Process;

void FillProcessStats(Process *process)
{
    PROCESS_MEMORY_COUNTERS_EX pmc;

    HANDLE hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process->pid);

    GetProcessMemoryInfo(hProcess, (PROCESS_MEMORY_COUNTERS*)&pmc, sizeof(pmc));

    FILETIME ftCreation, ftExit, ftKernel, ftUser;
    GetProcessTimes(hProcess, &ftCreation, &ftExit, &ftKernel, &ftUser);
    __int64 userTime = (__int64) ftUser.dwHighDateTime << 32 | ftUser.dwLowDateTime;
    __int64 userTimeSec = userTime / 10000000;
    __int64 kernalTime = (__int64) ftKernel.dwHighDateTime << 32 | ftKernel.dwLowDateTime;
    __int64 kernalTimeSec = kernalTime / 10000000;
    __int64 totalSeconds = userTimeSec + kernalTimeSec;

    process->workingSet = pmc.WorkingSetSize / 1024;
    process->privateBytes = pmc.PrivateUsage / 1024;
    process->cpu = totalSeconds;
}

void PrintProcessNameAndID(DWORD processID)
{
    TCHAR lpImageFileName[MAX_PATH] = TEXT("<unknown>");
    LPCWSTR processShortFileName = TEXT("<unknown>");
    PROCESS_MEMORY_COUNTERS_EX pmc;

    HANDLE hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, processID );

    if(NULL != hProcess)
    {
        GetProcessImageFileName(
            hProcess,
            lpImageFileName,
            sizeof(lpImageFileName)/sizeof(TCHAR));
        processShortFileName = PathFindFileName(lpImageFileName);
    }

    GetProcessMemoryInfo(hProcess, (PROCESS_MEMORY_COUNTERS*)&pmc, sizeof(pmc));

    FILETIME ftCreation, ftExit, ftKernel, ftUser;
    GetProcessTimes(hProcess, &ftCreation, &ftExit, &ftKernel, &ftUser);
    __int64 userTime = (__int64) ftUser.dwHighDateTime << 32 | ftUser.dwLowDateTime;
    __int64 userTimeSec = userTime / 10000000;
    __int64 kernalTime = (__int64) ftKernel.dwHighDateTime << 32 | ftKernel.dwLowDateTime;
    __int64 kernalTimeSec = kernalTime / 10000000;
    __int64 totalSeconds = userTimeSec + kernalTimeSec;

    WCHAR privateBytesFormatedStr[MAX_PATH];
    FormatNumber(privateBytesFormatedStr, pmc.PrivateUsage, &numFmt);

    WCHAR workingSetFormatedStr[MAX_PATH];
    FormatNumber(workingSetFormatedStr, pmc.WorkingSetSize, &numFmt);

    WCHAR cpuSecondsStrFormated[MAX_PATH];
    FormatNumber(cpuSecondsStrFormated, totalSeconds, &numFmt);

    _tprintf( TEXT("%-45s %08d %20ls %20ls %10ls\n"), processShortFileName, processID, workingSetFormatedStr, privateBytesFormatedStr, cpuSecondsStrFormated);

    CloseHandle( hProcess );
}

int CompareProcessPrivateBytes(const void *a, const void *b)
{
    Process *processA = (Process *)a;
    Process *processB = (Process *)b;

    if(sortColumn == 1)
    {
        return (processB->cpu - processA->cpu);
    }
    if(sortColumn == 2)
    {
        return (processB->privateBytes - processA->privateBytes);
    }
    if(sortColumn == 3)
    {
        return (processB->workingSet - processA->workingSet);
    }
    if(sortColumn == 4)
    {
        return (processB->pid - processA->pid);
    }
    return 0;
}

void PrintProcess(Process *process)
{
    TCHAR lpImageFileName[MAX_PATH] = TEXT("<unknown>");
    LPCWSTR processShortFileName = TEXT("<unknown>");

    HANDLE hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process->pid);

    if(NULL != hProcess)
    {
        GetProcessImageFileName(
            hProcess,
            lpImageFileName,
            MAX_PATH);
        processShortFileName = PathFindFileName(lpImageFileName);
    }

    WCHAR privateBytesFormatedStr[MAX_PATH];
    FormatNumber(privateBytesFormatedStr, process->privateBytes, &numFmt);

    WCHAR workingSetFormatedStr[MAX_PATH];
    FormatNumber(workingSetFormatedStr, process->workingSet, &numFmt);

    WCHAR cpuSecondsStrFormated[MAX_PATH];
    FormatNumber(cpuSecondsStrFormated, process->cpu, &numFmt);

    printf("%-45ls %08d %20ls %20ls %10ls\n", processShortFileName, process->pid, workingSetFormatedStr, privateBytesFormatedStr, cpuSecondsStrFormated);
}

int main(int argc, char* argv[])
{
    for(int i = 0; i < argc; i++)
    {
        if(strcmp(argv[i], "--sort") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                if(strcmp(argv[i], "cpu") == 0)
                {
                    sortColumn = 1;
                }
                else if(strcmp(argv[i], "privateBytes") == 0)
                {
                    sortColumn = 2;
                }
                else if(strcmp(argv[i], "workingSet") == 0)
                {
                    sortColumn = 3;
                }
                else if(strcmp(argv[i], "pid") == 0)
                {
                    sortColumn = 4;
                }
                else if(strcmp(argv[i], "name") == 0)
                {
                    sortColumn = 5;
                }
                if(sortColumn != -1)
                {
                    sort = TRUE;
                }
            }
        }
    }

    numFmt.NumDigits = 0;
    numFmt.LeadingZero = 0;
    numFmt.NumDigits = 0;
    numFmt.LeadingZero = 0;
    numFmt.Grouping = 3;
    numFmt.lpDecimalSep = L".";
    numFmt.lpThousandSep = L",";
    numFmt.NegativeOrder = 1;

    sm_EnableTokenPrivilege(SE_DEBUG_NAME);
    DWORD aProcesses[1024], cbNeeded, cProcesses;
    unsigned int i;

    if(!EnumProcesses(aProcesses, sizeof(aProcesses), &cbNeeded))
    {
        return 1;
    }

    cProcesses = cbNeeded / sizeof(DWORD);

    _tprintf( TEXT("%-45s %8ls %20ls %20ls %10ls\n"), L"Name", L"PID", L"WorkingSet(kb)", L"PrivateBytes(kb)", L"CPU(s)");
    if(!sort)
    {
        for(i = 0; i < cProcesses; i++)
        {
            if(aProcesses[i] != 0)
            {
                PrintProcessNameAndID(aProcesses[i]);
            }
        }
    }
    else
    {
        unsigned int count = 0;
        Process processes[1024];
        for(unsigned int j = 0; j < cProcesses; j++)
        {
            if(aProcesses[j] != 0)
            {
                processes[count].pid = aProcesses[j];
                FillProcessStats(&processes[count]);
                count++;
            }
        }

        qsort(processes, count + 1, sizeof(Process), CompareProcessPrivateBytes);

        for(unsigned int j = 0; j < count; j++)
        {
            PrintProcess(&processes[j]);
        }
    }

    return 0;
}
