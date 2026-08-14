use std::ffi::{c_char, c_void};

pub type OnSelectWindow = unsafe extern "C" fn(isize, *mut c_void);
pub type OnSelectString = unsafe extern "C" fn(*mut c_char, *mut c_void);
pub type OnClosed = unsafe extern "C" fn();
pub type ItemsAction = unsafe extern "C" fn(*mut c_void) -> *mut *mut c_char;
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
