//! This module provides a minimal, platform-specific implementation for
//! loading dynamic libraries (`.so` files) at runtime.
//! This is the foundation for fixing unnecessary recompiles and for hot-reloading.

// This entire module will only be compiled on Linux systems.
#[cfg(target_os = "linux")]
pub(crate) mod linux {
    use std::ffi::{CStr, OsStr};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    /// A handle to a dynamically loaded library.
    /// The library will be automatically unloaded when this struct is dropped.
    pub struct DynamicLibrary {
        handle: *mut libc::c_void,
    }

    impl DynamicLibrary {
        /// Loads a dynamic library from the given path.
        ///
        /// # Safety
        /// Loading a dynamic library is inherently unsafe as it can execute
        /// arbitrary code from the library's static initializers.
        #[allow(dead_code)]
        pub unsafe fn new(path: &Path) -> Result<Self, String> {
            // Convert the path to a C-compatible string.
            // Note: This requires the path to not contain interior null bytes.
            let path_os: &OsStr = path.as_ref();
            let c_path_bytes = path_os.as_bytes();
            let c_path = CStr::from_bytes_with_nul(c_path_bytes)
                .map_err(|_| format!("Path contains null bytes: {:?}", path_os))?;

            // Use dlopen to load the library. RTLD_NOW resolves all symbols immediately.
            let handle = libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW);
            if handle.is_null() {
                // If loading fails, get the error message from dlerror.
                let error_msg = CStr::from_ptr(libc::dlerror()).to_string_lossy().into_owned();
                Err(error_msg)
            } else {
                Ok(Self { handle })
            }
        }

        /// Retrieves a function pointer from the loaded library by its symbol name.
        ///
        /// # Safety
        /// The caller must ensure that the symbol exists in the library and that
        /// the function signature `T` is correct. Calling a symbol with the
        /// wrong signature is undefined behavior.
        #[allow(dead_code)]
        pub unsafe fn get_symbol<T>(&self, symbol: &[u8]) -> Result<T, String> {
            // Symbol names in C are null-terminated.
            let c_symbol = CStr::from_bytes_with_nul(symbol)
                .map_err(|_| format!("Symbol name contains null bytes: {:?}", symbol))?;

            let func_ptr = libc::dlsym(self.handle, c_symbol.as_ptr());
            if func_ptr.is_null() {
                let error_msg = CStr::from_ptr(libc::dlerror()).to_string_lossy().into_owned();
                Err(error_msg)
            } else {
                // Transmute the void pointer to the correct function signature.
                // This is the most unsafe part, relying entirely on the caller providing the correct T.
                Ok(std::mem::transmute_copy(&func_ptr))
            }
        }
    }

    impl Drop for DynamicLibrary {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    libc::dlclose(self.handle);
                }
                self.handle = ptr::null_mut();
            }
        }
    }
}
