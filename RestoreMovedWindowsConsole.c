#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <string.h>

#include "RestoreMovedWindows.h"

int main(int argc, char *argv[])
{
    BOOL noRestore = FALSE;
    if (argc >= 2 && strcmp(argv[1], "--NoRestore") == 0)
    {
        noRestore = TRUE;
    }

    int screenRight = GetSystemMetrics(SM_CXMAXTRACK);
    printf("Params: screenRight: %d, noRestore: %d\n", screenRight, noRestore);

    restore_moved_windows_to_screen(noRestore);

    return 0;
}