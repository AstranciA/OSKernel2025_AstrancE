use core::ffi::CStr;

pub(crate) unsafe fn cstr_to_str(ptr: usize) -> Result<&str, core::str::Utf8Error> {
    unsafe { CStr::from_ptr(ptr as *const _).to_str() }
}
