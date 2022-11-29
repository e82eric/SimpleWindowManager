#include <windows.h>
#include <stdio.h>
#include <tchar.h>
#include <psapi.h>
#include <processthreadsapi.h>
#include <shlwapi.h>
#include <locale.h>

#pragma comment(lib, "Advapi32.lib")
#pragma comment(lib, "Shlwapi.lib")

NUMBERFMT numFmt = { 0 };

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

void PrintProcessNameAndID( DWORD processID )
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

int main( void )
{
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

    if ( !EnumProcesses( aProcesses, sizeof(aProcesses), &cbNeeded ) )
    {
        return 1;
    }
    cProcesses = cbNeeded / sizeof(DWORD);

    _tprintf( TEXT("%-45s %8ls %20ls %20ls %10ls\n"), L"Name", L"PID", L"WorkingSet(kb)", L"PrivateBytes(kb)", L"CPU(s)");
    for ( i = 0; i < cProcesses; i++ )
    {
        if( aProcesses[i] != 0 )
        {
            PrintProcessNameAndID( aProcesses[i] );
        }
    }

    return 0;
}
