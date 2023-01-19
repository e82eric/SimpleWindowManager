#include <stdio.h>
#include <windows.h>
#include <tchar.h>
#include <psapi.h>
#include <processthreadsapi.h>
#include <shlwapi.h>
#include <locale.h>
#include <tlhelp32.h>

#include "SListProcesses.h"

int main(int argc, char* argv[])
{
    int (*linesFunc)(CHAR **linesToFill) = list_processes_run_no_sort;

    for(int i = 0; i < argc; i++)
    {
        if(strcmp(argv[i], "--sort") == 0)
        {
            if(i + 1 < argc)
            {
                i++;
                if(strcmp(argv[i], "cpu") == 0)
                {
                    linesFunc = list_processes_run_sorted_by_cpu;
                }
                else if(strcmp(argv[i], "privateBytes") == 0)
                {
                    linesFunc = list_processes_run_sorted_by_private_bytes;
                }
                else if(strcmp(argv[i], "workingSet") == 0)
                {
                    linesFunc = list_processes_run_sorted_by_working_set;
                }
                else if(strcmp(argv[i], "pid") == 0)
                {
                    linesFunc = list_processes_run_sorted_by_pid;
                }
            }
        }
    }

    CHAR *lines[4096];
    int numberOfResults = linesFunc(lines);

    for(int i = 0; i < numberOfResults; i++)
    {
        printf("%s\n", lines[i]);
    }

    return 0;
}
