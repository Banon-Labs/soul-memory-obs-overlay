use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn obs_module_load() -> bool {
    true
}

#[no_mangle]
pub extern "C" fn obs_module_description() -> *const c_char {
    static DESCRIPTION: &[u8] = b"Soul Memory Overlay plugin\0";
    DESCRIPTION.as_ptr() as *const c_char
}
