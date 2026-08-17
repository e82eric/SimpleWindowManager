mod windows_impl {
    use std::ffi::c_void;
    use std::sync::Mutex;
    use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows_sys::core::GUID;

    static AUDIO: Mutex<Option<usize>> = Mutex::new(None);
    static NETWORK: Mutex<Option<usize>> = Mutex::new(None);

    const CLSID_MM_DEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xbcde0395,
        data2: 0xe52f,
        data3: 0x467c,
        data4: [0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e],
    };
    const IID_MM_DEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xa95664d2,
        data2: 0x9614,
        data3: 0x4f35,
        data4: [0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6],
    };
    const IID_AUDIO_ENDPOINT_VOLUME: GUID = GUID {
        data1: 0x5cdf2c82,
        data2: 0x841e,
        data3: 0x4546,
        data4: [0x97, 0x22, 0x0c, 0xf7, 0x40, 0x78, 0x22, 0x9a],
    };
    const CLSID_NETWORK_LIST_MANAGER: GUID = GUID {
        data1: 0xdcb00c01,
        data2: 0x570f,
        data3: 0x4a9b,
        data4: [0x8d, 0x69, 0x19, 0x9f, 0xdb, 0xa5, 0x72, 0x3b],
    };
    const IID_NETWORK_LIST_MANAGER: GUID = GUID {
        data1: 0xdcb00000,
        data2: 0x570f,
        data3: 0x4a9b,
        data4: [0x8d, 0x69, 0x19, 0x9f, 0xdb, 0xa5, 0x72, 0x3b],
    };

    unsafe fn method(object: *mut c_void, index: usize) -> usize {
        unsafe { *(*(object as *mut *mut usize)).add(index) }
    }
    unsafe fn release(object: usize) {
        let call: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { std::mem::transmute(method(object as _, 2)) };
        unsafe {
            call(object as _);
        }
    }

    pub fn initialize() {
        unsafe {
            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let audio_result = CoCreateInstance(
                &CLSID_MM_DEVICE_ENUMERATOR,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_MM_DEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if crate::error_log::check_hresult(
                "CoCreateInstance(MMDeviceEnumerator) failed",
                audio_result,
            ) && !enumerator.is_null()
            {
                let get_default: unsafe extern "system" fn(
                    *mut c_void,
                    i32,
                    i32,
                    *mut *mut c_void,
                ) -> i32 = std::mem::transmute(method(enumerator, 4));
                let mut device: *mut c_void = std::ptr::null_mut();
                let default_result = get_default(enumerator, 0, 1, &mut device);
                if crate::error_log::check_hresult(
                    "IMMDeviceEnumerator::GetDefaultAudioEndpoint failed",
                    default_result,
                ) && !device.is_null()
                {
                    let activate: unsafe extern "system" fn(
                        *mut c_void,
                        *const GUID,
                        u32,
                        *mut c_void,
                        *mut *mut c_void,
                    ) -> i32 = std::mem::transmute(method(device, 3));
                    let mut endpoint: *mut c_void = std::ptr::null_mut();
                    let activate_result = activate(
                        device,
                        &IID_AUDIO_ENDPOINT_VOLUME,
                        CLSCTX_ALL,
                        std::ptr::null_mut(),
                        &mut endpoint,
                    );
                    if crate::error_log::check_hresult(
                        "IMMDevice::Activate(IAudioEndpointVolume) failed",
                        activate_result,
                    ) && !endpoint.is_null()
                    {
                        *AUDIO.lock().unwrap() = Some(endpoint as usize);
                    }
                    release(device as usize);
                }
                release(enumerator as usize);
            }
            let mut network: *mut c_void = std::ptr::null_mut();
            let network_result = CoCreateInstance(
                &CLSID_NETWORK_LIST_MANAGER,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_NETWORK_LIST_MANAGER,
                &mut network,
            );
            if crate::error_log::check_hresult(
                "CoCreateInstance(NetworkListManager) failed",
                network_result,
            ) && !network.is_null()
            {
                *NETWORK.lock().unwrap() = Some(network as usize);
            }
        }
    }

    pub fn shutdown() {
        unsafe {
            if let Some(value) = AUDIO.lock().unwrap().take() {
                release(value);
            }
            if let Some(value) = NETWORK.lock().unwrap().take() {
                release(value);
            }
        }
    }

    pub fn volume_percent() -> String {
        let Some(endpoint) = *AUDIO.lock().unwrap() else {
            return "  ?%".into();
        };
        unsafe {
            let get_scalar: unsafe extern "system" fn(*mut c_void, *mut f32) -> i32 =
                std::mem::transmute(method(endpoint as _, 9));
            let get_mute: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32 =
                std::mem::transmute(method(endpoint as _, 15));
            let mut scalar = 0.0;
            let mut muted = 0;
            if !crate::error_log::check_hresult(
                "IAudioEndpointVolume::GetMasterVolumeLevelScalar failed",
                get_scalar(endpoint as _, &mut scalar),
            ) {
                return "  ?%".into();
            }
            if !crate::error_log::check_hresult(
                "IAudioEndpointVolume::GetMute failed",
                get_mute(endpoint as _, &mut muted),
            ) {
                return "  ?%".into();
            }
            if muted != 0 {
                scalar = 0.0;
            }
            format!("{:3.0}%", scalar * 100.0)
        }
    }

    pub fn internet_icon() -> String {
        let Some(network) = *NETWORK.lock().unwrap() else {
            return "\u{f128}".into();
        };
        unsafe {
            let get_connected: unsafe extern "system" fn(*mut c_void, *mut i16) -> i32 =
                std::mem::transmute(method(network as _, 11));
            let mut connected = 0i16;
            let result = get_connected(network as _, &mut connected);
            if !crate::error_log::check_hresult(
                "INetworkListManager::get_IsConnectedToInternet failed",
                result,
            ) {
                "\u{f128}".into()
            } else if connected != 0 {
                "\u{ea7a}".into()
            } else {
                "\u{f127}".into()
            }
        }
    }
}

pub use windows_impl::*;
