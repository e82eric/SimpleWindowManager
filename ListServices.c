#include <stdio.h>
#include <windows.h>
#include <psapi.h>
#include <locale.h>
#include <tlhelp32.h>
#include <shlwapi.h>
#include <strsafe.h>

int EnumerateAllServices(SC_HANDLE hSCM, CHAR **toFill, int maxItems)
{
    void* buf = NULL;
    DWORD bufSize = 0;
    DWORD moreBytesNeeded, serviceCount;
    int numberOfResults = 0;
    CHAR textBuff[1024];

    for (;;)
    {
        if (EnumServicesStatusEx(
                    hSCM,
                    SC_ENUM_PROCESS_INFO,
                    SERVICE_WIN32,
                    SERVICE_STATE_ALL,
                    (LPBYTE)buf,
                    bufSize,
                    &moreBytesNeeded,
                    &serviceCount,
                    NULL,
                    NULL))
        {
            ENUM_SERVICE_STATUS_PROCESS* services = (ENUM_SERVICE_STATUS_PROCESS*)buf;
            for(DWORD i = 0; i < serviceCount && i < maxItems; i++)
            {
                CHAR serviceStateBuff[1024];
                switch(services[i].ServiceStatusProcess.dwCurrentState)
                {
                    case SERVICE_CONTINUE_PENDING:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_CONTINUE_PENDING");
                        break;
                    case SERVICE_PAUSE_PENDING:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_PAUSE_PENDING");
                        break;
                    case SERVICE_RUNNING:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_RUNNING");
                        break;
                    case SERVICE_START_PENDING:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_START_PENDING");
                        break;
                    case SERVICE_STOP_PENDING:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_STOP_PENDING");
                        break;
                    case SERVICE_STOPPED:
                        StringCchCopyA(
                                serviceStateBuff,
                                1024,
                                "SERVICE_STOPPED");
                        break;
                }

                CHAR serviceTypeBuff[1024];
                switch(services[i].ServiceStatusProcess.dwServiceType)
                {
                    case SERVICE_FILE_SYSTEM_DRIVER:
                        StringCchCopyA(
                                serviceTypeBuff,
                                1024,
                                "SERVICE_FILE_SYSTEM_DRIVER");
                        break;
                    case SERVICE_KERNEL_DRIVER:
                        StringCchCopyA(
                                serviceTypeBuff,
                                1024,
                                "SERVICE_KERNEL_DRIVER");
                        break;
                    case SERVICE_WIN32_OWN_PROCESS:
                        StringCchCopyA(
                                serviceTypeBuff,
                                1024,
                                "SERVICE_WIN32_OWN_PROCESS");
                        break;
                    case SERVICE_WIN32_SHARE_PROCESS:
                        StringCchCopyA(
                                serviceTypeBuff,
                                1024,
                                "SERVICE_WIN32_SHARE_PROCESS");
                        break;
                }

                CHAR pidBuff[32];
                if(services[i].ServiceStatusProcess.dwProcessId != 0)
                {
                    sprintf_s(
                            pidBuff,
                            32,
                            "%d",
                            services[i].ServiceStatusProcess.dwProcessId);
                }
                else
                {
                    StringCchCopyA(
                            pidBuff,
                            32,
                            "");
                }

                sprintf_s(
                        textBuff,
                        1024,
                        "%-45.45ls %-50.50ls %-8s %-35s %-35s",
                        services[i].lpServiceName,
                        services[i].lpDisplayName,
                        pidBuff,
                        serviceTypeBuff,
                        serviceStateBuff);

                toFill[numberOfResults] = StrDupA(textBuff);
                numberOfResults++;
            }
            free(buf);
            return numberOfResults;
        }

        int err = GetLastError();
        if (ERROR_MORE_DATA != err)
        {
            free(buf);
            return err;
        }

        bufSize += moreBytesNeeded;
        free(buf);
        buf = malloc(bufSize);
    }
}

int list_services_run_no_sort(int maxItems, CHAR** linesToFill)
{
    SC_HANDLE scHandle = OpenSCManagerA(
            NULL,
            NULL,
            SC_MANAGER_ALL_ACCESS);

    CHAR headerBuff[1024];
    sprintf_s(
            headerBuff,
            1024,
            "%-45.45s %-50.50s %-8s %-35s %-35s",
            "Name",
            "Display",
            "PID",
            "Type",
            "State");

    linesToFill[0] = StrDupA(headerBuff);

    int numberOfServices = EnumerateAllServices(scHandle, &linesToFill[1], maxItems - 1);

    CloseServiceHandle(scHandle);

    return numberOfServices + 1;
}

void start_service(CHAR *szSvcName)
{
    SERVICE_STATUS_PROCESS ssStatus; 
    DWORD dwBytesNeeded;
 
    SC_HANDLE schSCManager = OpenSCManager( 
        NULL,
        NULL,
        SC_MANAGER_ALL_ACCESS);
 
    if (NULL == schSCManager) 
    {
        printf("OpenSCManager failed (%d)\n", GetLastError());
        return;
    }

    SC_HANDLE schService = OpenServiceA( 
        schSCManager,
        szSvcName,
        SERVICE_ALL_ACCESS);
 
    if (schService == NULL)
    { 
        printf("OpenService failed (%d)\n", GetLastError()); 
        CloseServiceHandle(schSCManager);
        return;
    }    

    if (!QueryServiceStatusEx( 
            schService,
            SC_STATUS_PROCESS_INFO,
            (LPBYTE) &ssStatus,
            sizeof(SERVICE_STATUS_PROCESS),
            &dwBytesNeeded))
    {
        printf("QueryServiceStatusEx failed (%d)\n", GetLastError());
        CloseServiceHandle(schService); 
        CloseServiceHandle(schSCManager);
        return; 
    }

    if(ssStatus.dwCurrentState != SERVICE_STOPPED && ssStatus.dwCurrentState != SERVICE_STOP_PENDING)
    {
        printf("Cannot start the service because it is already running\n");
        CloseServiceHandle(schService); 
        CloseServiceHandle(schSCManager);
        return; 
    }

    if (!StartService(
            schService,
            0,
            NULL))
    {
        printf("StartService failed (%d)\n", GetLastError());
        CloseServiceHandle(schService); 
        CloseServiceHandle(schSCManager);
        return; 
    }

    CloseServiceHandle(schService); 
    CloseServiceHandle(schSCManager);
}
