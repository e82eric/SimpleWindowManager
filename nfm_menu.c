#define COBJMACROS
#include "nfm_menu.h"
#include <windows.h>
#include <stdio.h>
#include <strsafe.h>

#pragma comment(lib, "user32.lib")

// Globals
nfm_initialize_func                 nfm_initialize = NULL;
nfm_show_file_system_func           nfm_show_file_system = NULL;
nfm_show_programs_list_func         nfm_show_programs_list = NULL;
nfm_show_windows_list_func          nfm_show_windows_list = NULL;
nfm_show_processes_list_func        nfm_show_processes_list = NULL;
nfm_show_items_list_func            nfm_show_items_list = NULL;
nfm_show_array_columns_menu_func    nfm_show_array_columns_menu = NULL;
nfm_hide_func                       nfm_hide = NULL;
nfm_run_last_definition_func        nfm_run_last_definition = NULL;

static HMODULE LoadDllStrict(const wchar_t* fullPath, DWORD* lastErr)
{
    if (lastErr) *lastErr = 0;
    // Only DLL's own dir + System32 for dependency resolution:
    HMODULE h = LoadLibraryExW(fullPath, NULL,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!h && lastErr) *lastErr = GetLastError();
    return h;
}

HMODULE nfm_load_library(void)
{
    // (Optional but recommended) prevent loader UI popups if a dependency is missing
    UINT oldMode = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX);

    // Resolve EXE directory
    wchar_t exePath[MAX_PATH];
    DWORD len = GetModuleFileNameW(NULL, exePath, MAX_PATH);
    if (len == 0 || len >= MAX_PATH) {
        fprintf(stderr, "GetModuleFileNameW failed (err=%lu)\n", GetLastError());
        SetErrorMode(oldMode);
        return NULL;
    }

    // Trim to directory
    for (int i = (int)len - 1; i >= 0; --i) {
        if (exePath[i] == L'\\' || exePath[i] == L'/') {
            exePath[i + 1] = L'\0';
            break;
        }
        if (i == 0) {
            fprintf(stderr, "Unexpected module path format\n");
            SetErrorMode(oldMode);
            return NULL;
        }
    }

    // Build full DLL path
    const wchar_t* dllName = L"LibNfm.dll";
    wchar_t fullPath[MAX_PATH];
    HRESULT hr = StringCchPrintfW(fullPath, MAX_PATH, L"%ls%ls", exePath, dllName);
    if (FAILED(hr)) {
        fprintf(stderr, "Failed to build LibNfm.dll path (0x%08lx)\n", (unsigned long)hr);
        SetErrorMode(oldMode);
        return NULL;
    }

    // Existence check (helps with clearer messages)
    DWORD attrs = GetFileAttributesW(fullPath);
    if (attrs == INVALID_FILE_ATTRIBUTES || (attrs & FILE_ATTRIBUTE_DIRECTORY)) {
        DWORD err = GetLastError();
        wchar_t msg[1024];
        StringCchPrintfW(msg, 1024,
            L"LibNfm.dll not found next to the EXE.\nExpected: %ls\nError=%lu",
            fullPath, err);
        fprintf(stderr, "%S\n", msg);
        MessageBoxW(NULL, msg, L"SimpleWindowManager", MB_OK | MB_ICONERROR);
        SetErrorMode(oldMode);
        return NULL;
    }

    // Strict load (no unsafe fallback)
    DWORD loadErr = 0;
    HMODULE hModule = LoadDllStrict(fullPath, &loadErr);
    if (!hModule) {
        wchar_t extra[512] = L"";
        if (loadErr == ERROR_BAD_EXE_FORMAT) {
            StringCchCopyW(extra, 512, L"(Wrong architecture? Ensure 64-bit app loads 64-bit DLL.) ");
        } else if (loadErr == ERROR_MOD_NOT_FOUND) {
            StringCchCopyW(extra, 512, L"(Missing dependency? Ensure all dependent DLLs are next to LibNfm.dll or in System32.) ");
        }
        wchar_t msg[1024];
        StringCchPrintfW(msg, 1024,
            L"Failed to load LibNfm from: %ls\nLoadLibraryEx error=%lu. %ls",
            fullPath, loadErr, extra);
        fprintf(stderr, "%S\n", msg);
        MessageBoxW(NULL, msg, L"SimpleWindowManager", MB_OK | MB_ICONERROR);
        SetErrorMode(oldMode);
        return NULL;
    }

    // Resolve required exports
    nfm_initialize               = (nfm_initialize_func)              GetProcAddress(hModule, "Initialize");
    nfm_show_file_system         = (nfm_show_file_system_func)        GetProcAddress(hModule, "ShowFileSystem");
    nfm_show_programs_list       = (nfm_show_programs_list_func)      GetProcAddress(hModule, "ShowProgramsList");
    nfm_show_windows_list        = (nfm_show_windows_list_func)       GetProcAddress(hModule, "ShowWindowsList");
    nfm_show_processes_list      = (nfm_show_processes_list_func)     GetProcAddress(hModule, "ShowProcessesList");
    nfm_show_items_list          = (nfm_show_items_list_func)         GetProcAddress(hModule, "ShowItemsList");
    nfm_show_array_columns_menu  = (nfm_show_array_columns_menu_func) GetProcAddress(hModule, "ShowArrayColumns");
    nfm_hide                     = (nfm_hide_func)                    GetProcAddress(hModule, "Hide");
    nfm_run_last_definition      = (nfm_run_last_definition_func)     GetProcAddress(hModule, "RunLastDefinition");

    if (!nfm_initialize || !nfm_show_file_system || !nfm_show_programs_list || !nfm_show_windows_list ||
        !nfm_show_processes_list || !nfm_show_items_list || !nfm_show_array_columns_menu || !nfm_hide || !nfm_run_last_definition)
    {
        fprintf(stderr, "Missing exports: %s%s%s%s%s%s%s%s\n",
            nfm_initialize ? "" : "Initialize ",
            nfm_show_file_system ? "" : "ShowFileSystem ",
            nfm_show_programs_list ? "" : "ShowProgramsList ",
            nfm_show_windows_list ? "" : "ShowWindowsList ",
            nfm_show_processes_list ? "" : "ShowProcessesList ",
            nfm_show_items_list ? "" : "ShowItemsList ",
            nfm_show_array_columns_menu ? "" : "ShowArrayColumns ",
            nfm_hide ? (nfm_run_last_definition ? "" : "RunLastDefinition ") : "Hide RunLastDefinition ");
        FreeLibrary(hModule);
        SetErrorMode(oldMode);
        return NULL;
    }

    // Optional: perform an ABI/version handshake here if the DLL provides one

    nfm_initialize();
    SetErrorMode(oldMode);
    return hModule;
}

void nfm_unload_library(HMODULE hModule)
{
    if (hModule) {
        FreeLibrary(hModule);
    }
}
