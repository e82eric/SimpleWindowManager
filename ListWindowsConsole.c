#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <shlwapi.h>
#include <strsafe.h>

#include "ListWindows.h"

int main (void)
{
    CHAR *results[4096];
    int numberOfLines = list_windows_run(4096, results);

    for(int i = 0; i < numberOfLines; i++)
    {
        printf("%s", results[i]);
        free(results[i]);
    }

    return 0;
}
