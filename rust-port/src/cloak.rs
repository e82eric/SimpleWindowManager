pub fn set_cloak_for_window(hwnd: isize, cloak_type: i32, cloak_flag: i32) -> i32 {
    use std::ffi::c_void;
    use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows_sys::core::GUID;
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
    unsafe fn method(object: *mut c_void, index: usize) -> usize {
        unsafe { *(*(object as *mut *mut usize)).add(index) }
    }
    unsafe fn release(object: *mut c_void) {
        let call: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { std::mem::transmute(method(object, 2)) };
        unsafe {
            call(object);
        }
    }
    unsafe {
        let mut provider: *mut c_void = std::ptr::null_mut();
        let mut result = CoCreateInstance(
            &CLSID_IMMERSIVE_SHELL,
            std::ptr::null_mut(),
            CLSCTX_ALL,
            &IID_SERVICE_PROVIDER,
            &mut provider,
        );
        if result < 0 || provider.is_null() {
            if result < 0 {
                crate::error_log::report_hresult("CoCreateInstance(ImmersiveShell) failed", result);
            }
            return result;
        }
        let query: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            *const GUID,
            *mut *mut c_void,
        ) -> i32 = std::mem::transmute(method(provider, 3));
        let mut collection: *mut c_void = std::ptr::null_mut();
        result = query(
            provider,
            &IID_APP_VIEW_COLLECTION,
            &IID_APP_VIEW_COLLECTION,
            &mut collection,
        );
        if result < 0 {
            crate::error_log::report_hresult(
                "IServiceProvider::QueryService(ApplicationViewCollection) failed",
                result,
            );
        }
        if result >= 0 && !collection.is_null() {
            let get_view: unsafe extern "system" fn(
                *mut c_void,
                *mut c_void,
                *mut *mut c_void,
            ) -> i32 = std::mem::transmute(method(collection, 6));
            let mut view: *mut c_void = std::ptr::null_mut();
            result = get_view(collection, hwnd as *mut c_void, &mut view);
            if result < 0 {
                crate::error_log::report_hresult(
                    "IApplicationViewCollection::GetViewForHwnd failed",
                    result,
                );
            }
            if result >= 0 && !view.is_null() {
                let set_cloak: unsafe extern "system" fn(*mut c_void, i32, i32) -> i32 =
                    std::mem::transmute(method(view, 12));
                result = set_cloak(view, cloak_type, cloak_flag);
                if result < 0 {
                    crate::error_log::report_hresult("IApplicationView::SetCloak failed", result);
                }
                release(view);
            }
            release(collection);
        }
        release(provider);
        result
    }
}
