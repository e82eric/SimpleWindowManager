#include <stdio.h>
#include <windows.h>
#include <tchar.h>
#include <psapi.h>
#include <processthreadsapi.h>
#include <shlwapi.h>
#include <locale.h>
#include <tlhelp32.h>

#include "ListServices.h"

int main(int argc, char* argv[])
{
    UNREFERENCED_PARAMETER(argc);
    UNREFERENCED_PARAMETER(argv);

    CHAR *lines[4096];
    int numberOfResults = list_services_run_no_sort(4096, lines);

    for(int i = 0; i < numberOfResults; i++)
    {
        printf("%s\n", lines[i]);
    }

    return 0;
}
