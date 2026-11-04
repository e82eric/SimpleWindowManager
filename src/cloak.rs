use std::ffi::c_void;
use std::ptr::NonNull;

use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows_sys::core::GUID;

use crate::platform_error::{PlatformError, PlatformResult, check_hresult};

const CLSID_IMMERSIVE_SHELL: GUID = GUID {
    data1: 0xC2F03A33,
    data2: 0x21F5,
    data3: 0x47FA,
    data4: [0xB4, 0xBB, 0x15, 0x63, 0x62, 0xA2, 0xF2, 0x39],
};
const IID_SERVICE_PROVIDER: GUID = GUID {
    data1: 0x6D5140C1,
    data2: 0x7436,
    data3: 0x11CE,
    data4: [0x80, 0x34, 0x00, 0xAA, 0x00, 0x60, 0x09, 0xFA],
};
const IID_APP_VIEW_COLLECTION: GUID = GUID {
    data1: 0x1841C6D7,
    data2: 0x4F9D,
    data3: 0x42C0,
    data4: [0xAF, 0x41, 0x87, 0x47, 0x53, 0x8F, 0x10, 0xE5],
};

struct ComPtr(NonNull<c_void>);

impl ComPtr {
    fn checked_output(
        result: i32,
        pointer: *mut c_void,
        operation: &'static str,
    ) -> PlatformResult<Self> {
        if let Err(error) = check_hresult(result, operation) {
            if let Some(pointer) = NonNull::new(pointer) {
                drop(Self(pointer));
            }
            return Err(error);
        }
        NonNull::new(pointer)
            .map(Self)
            .ok_or_else(|| PlatformError::null_output(operation))
    }

    fn as_ptr(&self) -> *mut c_void {
        self.0.as_ptr()
    }

    unsafe fn method(&self, index: usize) -> usize {
        unsafe {
            let interface = self.as_ptr() as *mut *mut usize;
            let vtable = *interface;
            let slot = vtable.add(index);
            let function_address = *slot;

            function_address
        }
    }
}

impl Drop for ComPtr {
    fn drop(&mut self) {
        let release: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { std::mem::transmute(self.method(2)) };
        unsafe {
            release(self.as_ptr());
        }
    }
}

/// Owns the application-view collection for one group of cloak operations.
pub struct CloakManager {
    collection: ComPtr,
}

impl CloakManager {
    pub fn new() -> PlatformResult<Self> {
        unsafe {
            let mut provider = std::ptr::null_mut();
            let result = CoCreateInstance(
                &CLSID_IMMERSIVE_SHELL,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_SERVICE_PROVIDER,
                &mut provider,
            );
            let provider =
                ComPtr::checked_output(result, provider, "CoCreateInstance(ImmersiveShell)")?;

            let query: unsafe extern "system" fn(
                *mut c_void,
                *const GUID,
                *const GUID,
                *mut *mut c_void,
            ) -> i32 = std::mem::transmute(provider.method(3));
            let mut collection = std::ptr::null_mut();
            let result = query(
                provider.as_ptr(),
                &IID_APP_VIEW_COLLECTION,
                &IID_APP_VIEW_COLLECTION,
                &mut collection,
            );
            let collection = ComPtr::checked_output(
                result,
                collection,
                "IServiceProvider::QueryService(ApplicationViewCollection)",
            )?;

            Ok(Self { collection })
        }
    }

    pub fn set_cloak(&self, hwnd: isize, cloak_type: i32, cloak_flag: i32) -> PlatformResult<()> {
        unsafe {
            let get_view: unsafe extern "system" fn(
                *mut c_void,
                *mut c_void,
                *mut *mut c_void,
            ) -> i32 = std::mem::transmute(self.collection.method(6));
            let mut view = std::ptr::null_mut();
            let result = get_view(self.collection.as_ptr(), hwnd as *mut c_void, &mut view);
            let view =
                ComPtr::checked_output(result, view, "IApplicationViewCollection::GetViewForHwnd")?;

            let set_cloak: unsafe extern "system" fn(*mut c_void, i32, i32) -> i32 =
                std::mem::transmute(view.method(12));
            check_hresult(
                set_cloak(view.as_ptr(), cloak_type, cloak_flag),
                "IApplicationView::SetCloak",
            )
        }
    }
}
