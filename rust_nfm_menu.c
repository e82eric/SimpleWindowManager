#define COBJMACROS
#include "rust_nfm_menu.h"
#include "SimpleWindowManager.h"
#include <stdio.h>
#include <strsafe.h>

rust_nfm_initialize_func rust_nfm_initialize = NULL;
rust_nfm_show_items_list_func rust_nfm_show_items_list = NULL;
rust_nfm_show_programs_list_func rust_nfm_show_programs_list = NULL;
rust_nfm_show_file_system_func rust_nfm_show_file_system = NULL;
rust_nfm_show_windows_func rust_nfm_show_windows = NULL;
rust_nfm_show_processes_func rust_nfm_show_processes = NULL;
rust_nfm_hide_func rust_nfm_hide = NULL;
rust_nfm_focus_func rust_nfm_focus = NULL;
rust_nfm_set_menu_location_func rust_nfm_set_menu_location = NULL;

static HMODULE LoadDllStrict(const wchar_t* fullPath, DWORD* lastErr)
{
    if (lastErr) *lastErr = 0;
    HMODULE h = LoadLibraryExW(fullPath, NULL,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!h && lastErr) *lastErr = GetLastError();
    return h;
}

HMODULE rust_nfm_load_library(void)
{
    UINT oldMode = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX);

    wchar_t exePath[MAX_PATH];
    DWORD len = GetModuleFileNameW(NULL, exePath, MAX_PATH);
    if (len == 0 || len >= MAX_PATH) {
        fprintf(stderr, "GetModuleFileNameW failed for rust_nfm.dll (err=%lu)\n", GetLastError());
        SetErrorMode(oldMode);
        return NULL;
    }

    for (int i = (int)len - 1; i >= 0; --i) {
        if (exePath[i] == L'\\' || exePath[i] == L'/') {
            exePath[i + 1] = L'\0';
            break;
        }
        if (i == 0) {
            fprintf(stderr, "Unexpected module path format for rust_nfm.dll\n");
            SetErrorMode(oldMode);
            return NULL;
        }
    }

    wchar_t fullPath[MAX_PATH];
    HRESULT hr = StringCchPrintfW(fullPath, MAX_PATH, L"%lsrust_nfm.dll", exePath);
    if (FAILED(hr)) {
        fprintf(stderr, "Failed to build rust_nfm.dll path (0x%08lx)\n", (unsigned long)hr);
        SetErrorMode(oldMode);
        return NULL;
    }

    DWORD attrs = GetFileAttributesW(fullPath);
    if (attrs == INVALID_FILE_ATTRIBUTES || (attrs & FILE_ATTRIBUTE_DIRECTORY)) {
        fprintf(stderr, "rust_nfm.dll not found next to the EXE: %S\n", fullPath);
        SetErrorMode(oldMode);
        return NULL;
    }

    DWORD loadErr = 0;
    HMODULE hModule = LoadDllStrict(fullPath, &loadErr);
    if (!hModule) {
        fprintf(stderr, "Failed to load rust_nfm.dll from: %S LoadLibraryEx error=%lu\n", fullPath, loadErr);
        SetErrorMode(oldMode);
        return NULL;
    }

    rust_nfm_initialize = (rust_nfm_initialize_func)GetProcAddress(hModule, "RustNfmInitialize");
    rust_nfm_show_items_list = (rust_nfm_show_items_list_func)GetProcAddress(hModule, "RustNfmShowItemsList");
    rust_nfm_show_programs_list = (rust_nfm_show_programs_list_func)GetProcAddress(hModule, "RustNfmShowProgramsList");
    rust_nfm_show_file_system = (rust_nfm_show_file_system_func)GetProcAddress(hModule, "RustNfmShowFileSystem");
    rust_nfm_show_windows = (rust_nfm_show_windows_func)GetProcAddress(hModule, "RustNfmShowWindows");
    rust_nfm_show_processes = (rust_nfm_show_processes_func)GetProcAddress(hModule, "RustNfmShowProcesses");
    rust_nfm_hide = (rust_nfm_hide_func)GetProcAddress(hModule, "RustNfmHide");
    rust_nfm_focus = (rust_nfm_focus_func)GetProcAddress(hModule, "RustNfmFocus");
    rust_nfm_set_menu_location = (rust_nfm_set_menu_location_func)GetProcAddress(hModule, "RustNfmSetMenuLocation");

    if (!rust_nfm_initialize || !rust_nfm_show_items_list || !rust_nfm_hide || !rust_nfm_focus || !rust_nfm_set_menu_location || !rust_nfm_show_programs_list || !rust_nfm_show_file_system || !rust_nfm_show_windows) {
        fprintf(stderr, "Missing rust_nfm exports: %s%s%s%s%s%s%s%s\n",
            rust_nfm_initialize ? "" : "RustNfmInitialize ",
            rust_nfm_show_items_list ? "" : "RustNfmShowItemsList ",
            rust_nfm_show_programs_list ? "" : "RustNfmShowProgramsList ",
            rust_nfm_show_file_system ? "" : "RustNfmShowFileSystem ",
            rust_nfm_show_windows ? "" : "RustNfmShowWindows ",
            rust_nfm_hide ? "" : "RustNfmHide ",
            rust_nfm_focus ? "" : "RustNfmFocus ",
            rust_nfm_set_menu_location ? "" : "RustNfmSetMenuLocation ");
        FreeLibrary(hModule);
        rust_nfm_initialize = NULL;
        rust_nfm_show_items_list = NULL;
        rust_nfm_show_programs_list = NULL;
        rust_nfm_show_file_system = NULL;
        rust_nfm_show_windows = NULL;
        rust_nfm_show_processes = NULL;
        rust_nfm_hide = NULL;
        rust_nfm_focus = NULL;
        rust_nfm_set_menu_location = NULL;
        SetErrorMode(oldMode);
        return NULL;
    }

    rust_nfm_initialize();
    SetErrorMode(oldMode);
    return hModule;
}

void rust_nfm_unload_library(HMODULE hModule)
{
    if (hModule) {
        FreeLibrary(hModule);
    }
}

void rust_nfm_set_menu_location_monitor_center(Monitor *monitor)
{
    if (!rust_nfm_set_menu_location) return;
    int cx = monitor->xOffset + monitor->w / 2;
    int cy = (monitor->top + monitor->bottom) / 2;
    rust_nfm_set_menu_location(cx, cy);
}
