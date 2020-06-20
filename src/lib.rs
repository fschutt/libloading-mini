#[cfg(unix)] mod internal_impl {
    // Platform specific things
    use std::os::raw;
    use std::ffi::OsStr;
    use super::cstr_cow_from_bytes;

    extern { // syscalls
        fn dlopen(filename: *const raw::c_char, flags: raw::c_int) -> *mut raw::c_void;
        fn dlsym(handle: *mut raw::c_void, symbol: *const raw::c_char) -> *mut raw::c_void;
        fn dlclose(handle: *mut raw::c_void) -> raw::c_int;
        fn dlerror() -> *mut raw::c_char;
    }

    #[cfg(not(target_os="android"))]
    const RTLD_NOW: raw::c_int = 2;
    #[cfg(target_os="android")]
    const RTLD_NOW: raw::c_int = 0;

    /// A platform-specific equivalent of the cross-platform `Library`.
    pub struct Library(*mut raw::c_void);

    unsafe impl Send for Library {}
    unsafe impl Sync for Library {}

    impl Library {
        pub fn new<P: AsRef<OsStr>>(path: P) -> Option<Self> {
            use std::os::unix::ffi::OsStrExt;
            let file_path = cstr_cow_from_bytes(path.as_ref().as_bytes())?;
            let result = unsafe { dlopen(file_path.as_ptr(), RTLD_NOW) };
            if result.is_null() {
                #[cfg(feature = "debug_symbols")] {
                    println!("failed to dlopen library in path {}", path.as_ref().to_string_lossy());
                }
                None
            } else {
                #[cfg(feature = "debug_symbols")] {
                    println!("dlopen library in path {} @ 0x{:x}", path.as_ref().to_string_lossy(), result as usize);
                }
                Some(Library(result))
            }
        }

        pub fn get(&self, symbol: &[u8]) -> Option<*mut raw::c_void> {
            let symbol_name_new = cstr_cow_from_bytes(symbol)?;
            let symbol_new = unsafe { dlsym(self.0, symbol_name_new.as_ptr()) };
            let error = unsafe { dlerror() };
            if error.is_null() {
                #[cfg(feature = "debug_symbols")] {
                    println!("ok loaded symbol {} @ 0x{:x}", unsafe { std::str::from_utf8_unchecked(symbol) }, symbol_new as usize);
                }
                Some(symbol_new)
            } else {
                #[cfg(feature = "debug_symbols")] {
                    println!("missing symbol {} @ 0x{:x}", unsafe { std::str::from_utf8_unchecked(symbol) }, symbol_new as usize);
                }
                None
            }
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe { dlclose(self.0) };
        }
    }
}

#[cfg(windows)] mod internal_impl {

    extern crate winapi;

    use self::winapi::shared::minwindef::{HMODULE, FARPROC};
    use self::winapi::um::libloaderapi;
    use super::cstr_cow_from_bytes;
    use std::ffi::OsStr;

    pub struct Library(HMODULE);

    unsafe impl Send for Library {}
    unsafe impl Sync for Library {}

    impl Library {
        pub fn new<P: AsRef<OsStr>>(path: P) -> Option<Self> {
            use std::os::windows::ffi::OsStrExt;
            let wide_filename: Vec<u16> = path.as_ref().encode_wide().chain(Some(0)).collect();
            let handle = unsafe { libloaderapi::LoadLibraryExW(wide_filename.as_ptr(), std::ptr::null_mut(), 0) };
            if handle.is_null()  { None } else { Some(Library(handle)) }
        }
        pub fn get(&self, symbol: &[u8]) -> Option<FARPROC> {
            let symbol = cstr_cow_from_bytes(symbol)?;
            let symbol = unsafe { libloaderapi::GetProcAddress(self.0, symbol.as_ptr()) };
            if symbol.is_null() { None } else { Some(symbol) }
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe { libloaderapi::FreeLibrary(self.0); }
        }
    }
}

use std::ffi::{CStr, CString};
use std::borrow::Cow;
use std::os::raw;

pub(crate) fn cstr_cow_from_bytes<'a>(slice: &'a [u8]) -> Option<Cow<'a, CStr>> {
    static ZERO: raw::c_char = 0;
    Some(match slice.last() {
        // Slice out of 0 elements
        None => unsafe { Cow::Borrowed(CStr::from_ptr(&ZERO)) },
        // Slice with trailing 0
        Some(&0) => {
            Cow::Borrowed(CStr::from_bytes_with_nul(slice).ok()?)
        },
        // Slice with no trailing 0
        Some(_) => Cow::Owned(CString::new(slice).ok()?),
    })
}

pub use internal_impl::Library;