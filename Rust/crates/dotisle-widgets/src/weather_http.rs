// Bounded public HTTPS GET using the interactive desktop's proxy settings.
// No cookies, authentication prompts, cache writes, or certificate exceptions.
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows::{
    core::{w, PCSTR},
    Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
};
type Handle = *mut c_void;
type Open = unsafe extern "system" fn(*const u16, u32, *const u16, *const u16, u32) -> Handle;
type OpenUrl = unsafe extern "system" fn(Handle, *const u16, *const u16, u32, u32, usize) -> Handle;
type SetOption = unsafe extern "system" fn(Handle, u32, *const c_void, u32) -> i32;
type Read = unsafe extern "system" fn(Handle, *mut c_void, u32, *mut u32) -> i32;
type Query = unsafe extern "system" fn(Handle, u32, *mut c_void, *mut u32, *mut u32) -> i32;
type Close = unsafe extern "system" fn(Handle) -> i32;
struct Api {
    open: Open,
    open_url: OpenUrl,
    set_option: SetOption,
    read: Read,
    query: Query,
    close: Close,
}
impl Api {
    fn get() -> Result<&'static Self, String> {
        static API: OnceLock<Result<Api, String>> = OnceLock::new();
        API.get_or_init(|| unsafe {
            // Retain the system DLL for the cached function pointers' lifetime.
            let module = LoadLibraryExW(w!("wininet.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                .map_err(|e| e.to_string())?;
            macro_rules! symbol {
                ($name:literal,$kind:ty) => {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $kind>(
                        GetProcAddress(module, PCSTR(concat!($name, "\0").as_ptr()))
                            .ok_or(concat!("missing ", $name))?,
                    )
                };
            }
            Ok(Api {
                open: symbol!("InternetOpenW", Open),
                open_url: symbol!("InternetOpenUrlW", OpenUrl),
                set_option: symbol!("InternetSetOptionW", SetOption),
                read: symbol!("InternetReadFile", Read),
                query: symbol!("HttpQueryInfoW", Query),
                close: symbol!("InternetCloseHandle", Close),
            })
        })
        .as_ref()
        .map_err(Clone::clone)
    }
}
struct Owned {
    handle: Handle,
    close: Close,
}
impl Owned {
    fn new(handle: Handle, api: &Api) -> Result<Self, String> {
        if handle.is_null() {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(Self {
                handle,
                close: api.close,
            })
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub fn get_json(url: &str) -> Result<serde_json::Value, String> {
    if !(url.starts_with("https://api.open-meteo.com/")
        || url.starts_with("https://geocoding-api.open-meteo.com/"))
    {
        return Err("weather endpoint is not allowed".into());
    }
    let api = Api::get()?;
    unsafe {
        let session = Owned::new(
            (api.open)(wide("DotIsle Weather/3.5").as_ptr(), 0, null(), null(), 0),
            api,
        )?;
        let timeout = 8000u32;
        for option in [2, 5, 6] {
            (api.set_option)(session.handle, option, &timeout as *const _ as _, 4);
        }
        const RELOAD: u32 = 0x80000000;
        const NO_CACHE_WRITE: u32 = 0x04000000;
        const NO_COOKIES: u32 = 0x00080000;
        const NO_AUTH: u32 = 0x00040000;
        const NO_UI: u32 = 0x00000200;
        let request = Owned::new(
            (api.open_url)(
                session.handle,
                wide(url).as_ptr(),
                null(),
                0,
                RELOAD | NO_CACHE_WRITE | NO_COOKIES | NO_AUTH | NO_UI,
                0,
            ),
            api,
        )?;
        let (mut status, mut length) = (0u32, 4u32);
        if (api.query)(
            request.handle,
            19 | 0x20000000,
            &mut status as *mut _ as _,
            &mut length,
            null_mut(),
        ) == 0
            || !(200..300).contains(&status)
        {
            return Err(format!("weather HTTP {status}"));
        }
        let mut body = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let mut count = 0;
            if (api.read)(
                request.handle,
                buffer.as_mut_ptr() as _,
                buffer.len() as u32,
                &mut count,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if count == 0 {
                break;
            }
            if body.len() + count as usize > 256 * 1024 {
                return Err("weather response exceeds limit".into());
            }
            body.extend_from_slice(&buffer[..count as usize]);
        }
        serde_json::from_slice(&body).map_err(|e| e.to_string())
    }
}
