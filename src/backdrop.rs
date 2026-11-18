use std::{ffi::c_void, mem::size_of, sync::OnceLock};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*};

#[repr(C)]
struct AccentPolicy {
    state: i32,
    flags: u32,
    color: u32,
    animation: u32,
}
#[repr(C)]
struct CompositionData {
    attribute: i32,
    data: *const c_void,
    size: usize,
}
type SetComposition = unsafe extern "system" fn(HWND, *const CompositionData) -> i32;

/// Windows 10's accent blur is resolved dynamically; unsupported systems retain
/// the readable solid background. No capture loop or software blur is needed.
pub unsafe fn apply(hwnd: HWND, enabled: bool) -> bool {
    if hwnd.is_null() || !GetParent(hwnd).is_null() {
        return false;
    }
    static API: OnceLock<Option<SetComposition>> = OnceLock::new();
    let api = API.get_or_init(|| unsafe {
        let module = GetModuleHandleW(crate::wide("user32.dll").as_ptr());
        GetProcAddress(module, c"SetWindowCompositionAttribute".as_ptr().cast()).map(|address| {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, SetComposition>(address)
        })
    });
    let Some(api) = api else { return false };
    let accent = AccentPolicy {
        state: if enabled { 3 } else { 0 }, // ACCENT_ENABLE_BLURBEHIND / DISABLED
        flags: 0,
        color: 0,
        animation: 0,
    };
    let data = CompositionData {
        attribute: 19, // WCA_ACCENT_POLICY
        data: (&accent as *const AccentPolicy).cast(),
        size: size_of::<AccentPolicy>(),
    };
    api(hwnd, &data) != 0 && enabled
}
