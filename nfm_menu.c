#include "nfm_menu.h"
#include <stdio.h>

nfm_initialize_func nfm_initialize = NULL;
nfm_show_file_system_func nfm_show_file_system = NULL;
nfm_show_programs_list_func nfm_show_programs_list = NULL;
nfm_show_windows_list_func nfm_show_windows_list = NULL;
nfm_show_processes_list_func nfm_show_processes_list = NULL;
nfm_show_items_list_func nfm_show_items_list = NULL;
nfm_hide_func nfm_hide = NULL;
nfm_run_last_definition_func nfm_run_last_definition = NULL;

HMODULE nfm_load_library(const char* dllPath) {
    HMODULE hModule = LoadLibrary(dllPath);
    if (!hModule) {
        fprintf(stderr, "Failed to load DLL: %s\n", dllPath);
        return NULL;
    }

    nfm_initialize = (nfm_initialize_func)GetProcAddress(hModule, "Initialize");
    nfm_show_file_system = (nfm_show_file_system_func)GetProcAddress(hModule, "ShowFileSystem");
    nfm_show_programs_list = (nfm_show_programs_list_func)GetProcAddress(hModule, "ShowProgramsList");
    nfm_show_windows_list = (nfm_show_windows_list_func)GetProcAddress(hModule, "ShowWindowsList");
    nfm_show_processes_list = (nfm_show_processes_list_func)GetProcAddress(hModule, "ShowProcessesList");
    nfm_show_items_list = (nfm_show_items_list_func)GetProcAddress(hModule, "ShowItemsList");
    nfm_hide = (nfm_hide_func)GetProcAddress(hModule, "Hide");
    nfm_run_last_definition = (nfm_run_last_definition_func)GetProcAddress(hModule, "RunLastDefinition");

    if (!nfm_initialize || !nfm_show_file_system || !nfm_show_programs_list || !nfm_show_windows_list ||
        !nfm_show_processes_list || !nfm_show_items_list || !nfm_hide || !nfm_run_last_definition) {
        fprintf(stderr, "Failed to resolve one or more functions in DLL.\n");
        FreeLibrary(hModule);
        return NULL;
    }

    nfm_initialize();

    return hModule;
}

void nfm_unload_library(HMODULE hModule) {
    if (hModule) {
        FreeLibrary(hModule);
    }
}
