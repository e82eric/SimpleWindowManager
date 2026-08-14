use std::ffi::{c_char, c_void};

pub type OnSelectWindow = unsafe extern "C" fn(isize, *mut c_void);
pub type OnSelectString = unsafe extern "C" fn(*mut c_char, *mut c_void);
pub type OnClosed = unsafe extern "C" fn();
pub type ItemsAction = unsafe extern "C" fn(*mut c_void) -> *mut *mut c_char;
pub type OnProcessSelect = unsafe extern "C" fn(u32, *mut c_void);
pub type ShowArrayColumns = unsafe extern "C" fn(
    *mut *mut *mut c_char,
    i32,
    i32,
    *mut *mut c_char,
    i32,
    *mut i32,
    i32,
    i32,
    OnSelectString,
    OnClosed,
    *mut c_void,
);

#[repr(C)]
pub struct RustNfmFileSystemOptions {
    pub struct_size: usize,
    pub max_depth: i32,
    pub preview_visible: i32,
}

#[cfg(windows)]
unsafe fn load_export<T: Copy>(module: isize, name: &'static [u8]) -> Option<T> {
    use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
    let address = unsafe { GetProcAddress(module as _, name.as_ptr()) }?;
    Some(unsafe { std::mem::transmute_copy(&address) })
}

#[cfg(windows)]
fn load_adjacent(name: &str) -> Result<isize, u32> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::LibraryLoader::{
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
    };
    let path = std::env::current_exe()
        .map_err(|_| 2u32)?
        .with_file_name(name);
    if !path.is_file() {
        return Err(2);
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let module = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        Err(unsafe { GetLastError() })
    } else {
        Ok(module as isize)
    }
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub struct RustNfm {
    pub initialize: unsafe extern "C" fn(),
    pub show_items_list:
        unsafe extern "C" fn(*mut c_char, ItemsAction, OnSelectString, OnClosed, *mut c_void),
    pub show_programs_list:
        unsafe extern "C" fn(*mut *mut c_char, i32, OnSelectString, OnClosed, *mut c_void),
    pub show_file_system: unsafe extern "C" fn(
        *const RustNfmFileSystemOptions,
        OnSelectString,
        OnClosed,
        *mut c_void,
    ) -> i32,
    pub show_windows: unsafe extern "C" fn(OnSelectWindow, OnClosed, *mut c_void) -> i32,
    pub show_processes: Option<unsafe extern "C" fn(OnProcessSelect, OnClosed, *mut c_void) -> i32>,
    pub hide: unsafe extern "C" fn(),
    pub focus: unsafe extern "C" fn(),
    pub set_menu_location: unsafe extern "C" fn(i32, i32),
}

#[cfg(windows)]
impl RustNfm {
    pub fn linked() -> Self {
        let result = Self {
            initialize: linked_initialize,
            show_items_list: linked_show_items_list,
            show_programs_list: linked_show_programs_list,
            show_file_system: linked_show_file_system,
            show_windows: linked_show_windows,
            show_processes: Some(linked_show_processes),
            hide: linked_hide,
            focus: linked_focus,
            set_menu_location: linked_set_menu_location,
        };
        unsafe {
            (result.initialize)();
        }
        result
    }
    pub fn set_monitor_center(&self, monitor: &crate::Monitor) {
        unsafe {
            (self.set_menu_location)(
                monitor.x_offset + monitor.width / 2,
                (monitor.top + monitor.bottom) / 2,
            );
        }
    }
}

#[cfg(windows)]
unsafe extern "C" fn linked_initialize() {
    rust_nfm::ffi::RustNfmInitialize();
}

#[cfg(windows)]
unsafe extern "C" fn linked_show_items_list(
    header: *mut c_char,
    items: ItemsAction,
    select: OnSelectString,
    closed: OnClosed,
    state: *mut c_void,
) {
    rust_nfm::ffi::RustNfmShowItemsList(header, Some(items), Some(select), Some(closed), state);
}

#[cfg(windows)]
unsafe extern "C" fn linked_show_programs_list(
    directories: *mut *mut c_char,
    count: i32,
    select: OnSelectString,
    closed: OnClosed,
    state: *mut c_void,
) {
    rust_nfm::ffi::RustNfmShowProgramsList(
        directories as *const *const c_char,
        count,
        Some(select),
        Some(closed),
        state,
    );
}

#[cfg(windows)]
unsafe extern "C" fn linked_show_file_system(
    options: *const RustNfmFileSystemOptions,
    select: OnSelectString,
    closed: OnClosed,
    state: *mut c_void,
) -> i32 {
    unsafe {
        rust_nfm::ffi::RustNfmShowFileSystem(options.cast(), Some(select), Some(closed), state)
    }
}

#[cfg(windows)]
unsafe extern "C" fn linked_show_windows(
    select: OnSelectWindow,
    closed: OnClosed,
    state: *mut c_void,
) -> i32 {
    rust_nfm::ffi::RustNfmShowWindows(Some(select), Some(closed), state)
}

#[cfg(windows)]
unsafe extern "C" fn linked_show_processes(
    select: OnProcessSelect,
    closed: OnClosed,
    state: *mut c_void,
) -> i32 {
    rust_nfm::ffi::RustNfmShowProcesses(Some(select), Some(closed), state)
}

#[cfg(windows)]
unsafe extern "C" fn linked_hide() {
    rust_nfm::ffi::RustNfmHide();
}

#[cfg(windows)]
unsafe extern "C" fn linked_focus() {
    rust_nfm::ffi::RustNfmFocus();
}

#[cfg(windows)]
unsafe extern "C" fn linked_set_menu_location(x: i32, y: i32) {
    rust_nfm::ffi::RustNfmSetMenuLocation(x, y);
}

pub struct LegacyNfm {
    #[cfg(windows)]
    module: isize,
    pub initialize: unsafe extern "C" fn(),
    pub show_windows_list: unsafe extern "C" fn(OnSelectWindow, OnClosed, *mut c_void),
    pub show_items_list:
        unsafe extern "C" fn(*mut c_char, ItemsAction, OnSelectString, OnClosed, *mut c_void),
    pub show_array_columns: ShowArrayColumns,
    pub hide: unsafe extern "C" fn(),
    pub show_programs_list:
        unsafe extern "C" fn(*mut *mut c_char, i32, OnSelectString, OnClosed, *mut c_void),
    pub run_last_definition: unsafe extern "C" fn(),
    pub show_file_system: unsafe extern "C" fn(OnSelectString, OnClosed, *mut c_void),
    pub show_processes_list: unsafe extern "C" fn(OnSelectString, OnClosed, *mut c_void),
    pub set_menu_location: Option<unsafe extern "C" fn(i32, i32)>,
}

#[cfg(windows)]
impl LegacyNfm {
    pub fn load() -> Result<Self, u32> {
        let module = load_adjacent("LibNfm.dll")?;
        let result = unsafe {
            Self {
                module,
                initialize: load_export(module, b"Initialize\0").ok_or(127u32)?,
                show_windows_list: load_export(module, b"ShowWindowsList\0").ok_or(127u32)?,
                show_items_list: load_export(module, b"ShowItemsList\0").ok_or(127u32)?,
                show_array_columns: load_export(module, b"ShowArrayColumns\0").ok_or(127u32)?,
                hide: load_export(module, b"Hide\0").ok_or(127u32)?,
                show_programs_list: load_export(module, b"ShowProgramsList\0").ok_or(127u32)?,
                run_last_definition: load_export(module, b"RunLastDefinition\0").ok_or(127u32)?,
                show_file_system: load_export(module, b"ShowFileSystem\0").ok_or(127u32)?,
                show_processes_list: load_export(module, b"ShowProcessesList\0").ok_or(127u32)?,
                set_menu_location: load_export(module, b"SetMenuLocation\0"),
            }
        };
        unsafe {
            (result.initialize)();
        }
        Ok(result)
    }
    pub fn set_monitor_center(&self, monitor: &crate::Monitor) {
        if let Some(set) = self.set_menu_location {
            unsafe {
                set(
                    monitor.x_offset + monitor.width / 2,
                    (monitor.top + monitor.bottom) / 2,
                );
            }
        }
    }
}

#[cfg(windows)]
impl Drop for LegacyNfm {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::FreeLibrary(self.module as _);
        }
    }
}
