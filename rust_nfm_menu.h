#ifndef RUST_NFM_MENU_H
#define RUST_NFM_MENU_H

#include <windows.h>

#include "nfm_menu.h"

typedef void (*rust_nfm_initialize_func)(void);
typedef void (*rust_nfm_show_items_list_func)(
    char* header,
    nfm_items_action_callback nativeItemsAction,
    nfm_on_select_string_callback onSelect,
    nfm_on_closed_callback onClosed,
    void* state
);
typedef void (*rust_nfm_show_programs_list_func)(
    char** directories,
    int directoryCount,
    nfm_on_select_string_callback onSelect,
    nfm_on_closed_callback onClosed,
    void* state
);
typedef struct RustNfmFileSystemOptions
{
    size_t struct_size;
    int max_depth;
    BOOL preview_visible;
} RustNfmFileSystemOptions;
typedef BOOL (*rust_nfm_show_file_system_func)(
    const RustNfmFileSystemOptions* options,
    nfm_on_select_string_callback onSelect,
    nfm_on_closed_callback onClosed,
    void* state
);
typedef BOOL (*rust_nfm_show_windows_func)(
    nfm_on_select_callback onSelect,
    nfm_on_closed_callback onClosed,
    void* state
);
typedef void (*rust_nfm_on_process_select_callback)(uint32_t pid, void* state);
typedef BOOL (*rust_nfm_show_processes_func)(
    rust_nfm_on_process_select_callback onSelect,
    nfm_on_closed_callback onClosed,
    void* state
);
typedef void (*rust_nfm_hide_func)(void);
typedef void (*rust_nfm_focus_func)(void);
typedef void (*rust_nfm_set_menu_location_func)(int x, int y);

HMODULE rust_nfm_load_library(void);
void rust_nfm_unload_library(HMODULE hModule);
void rust_nfm_set_menu_location_monitor_center(Monitor *monitor);

extern rust_nfm_initialize_func rust_nfm_initialize;
extern rust_nfm_show_items_list_func rust_nfm_show_items_list;
extern rust_nfm_show_programs_list_func rust_nfm_show_programs_list;
extern rust_nfm_show_file_system_func rust_nfm_show_file_system;
extern rust_nfm_show_windows_func rust_nfm_show_windows;
extern rust_nfm_show_processes_func rust_nfm_show_processes;
extern rust_nfm_hide_func rust_nfm_hide;
extern rust_nfm_focus_func rust_nfm_focus;
extern rust_nfm_set_menu_location_func rust_nfm_set_menu_location;

#endif
