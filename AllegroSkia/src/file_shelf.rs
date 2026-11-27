use serde::{Deserialize, Serialize};
use std::{
    ffi::c_void,
    mem::ManuallyDrop,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelfItem {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub is_dir: bool,
}

/// 安全非破坏性的「文件中转站」暂存清单：仅记录文件路径与元信息，绝不修改用户源文件。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileShelf {
    #[serde(default)]
    pub items: Vec<ShelfItem>,
}

impl FileShelf {
    pub fn demo() -> Self {
        Self {
            items: vec![
                ShelfItem {
                    path: PathBuf::from("C:\\Users\\Public\\产品方案_v2.pdf"),
                    name: "产品方案_v2.pdf".into(),
                    size_bytes: 2_480_120,
                    is_dir: false,
                },
                ShelfItem {
                    path: PathBuf::from("C:\\Users\\Public\\宣传片导出.mp4"),
                    name: "宣传片导出.mp4".into(),
                    size_bytes: 48_620_000,
                    is_dir: false,
                },
                ShelfItem {
                    path: PathBuf::from("C:\\Users\\Public\\配色规范.png"),
                    name: "配色规范.png".into(),
                    size_bytes: 842_100,
                    is_dir: false,
                },
            ],
        }
    }

    pub fn load(data_dir: &Path) -> Self {
        let p = data_dir.join("file-shelf.json");
        std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let p = data_dir.join("file-shelf.json");
        let tmp = p.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }

    /// 拖入文件或文件夹加入暂存站（自动提取名字和字节数并按规范路径去重）。
    pub fn add_path(&mut self, path: PathBuf) -> bool {
        let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.items.iter().any(|i| {
            std::fs::canonicalize(&i.path).unwrap_or_else(|_| i.path.clone()) == canon
        }) {
            return false;
        }
        let meta = std::fs::metadata(&path).ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        let size_bytes = if is_dir {
            0
        } else {
            meta.as_ref().map_or(0, |m| m.len())
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        self.items.push(ShelfItem {
            path,
            name,
            size_bytes,
            is_dir,
        });
        if self.items.len() > 64 {
            self.items.remove(0);
        }
        true
    }

    pub fn remove_index(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.items.remove(idx);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn existing_paths(&self) -> Vec<PathBuf> {
        self.items
            .iter()
            .map(|i| i.path.clone())
            .filter(|p| p.exists())
            .collect()
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "文件夹".into();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}

pub fn ext_badge_color(name: &str, is_dir: bool) -> (u8, u8, u8) {
    if is_dir {
        return (0, 122, 255);
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => (52, 199, 89),
        "mp4" | "mov" | "mkv" | "avi" | "mp3" | "wav" => (175, 82, 222),
        "pdf" | "doc" | "docx" | "md" | "txt" => (255, 59, 48),
        "zip" | "7z" | "rar" | "tar" | "gz" => (255, 149, 0),
        "rs" | "ts" | "js" | "py" | "json" | "toml" => (48, 176, 199),
        _ => (0, 122, 255),
    }
}

/// 构造 Windows 标准 20 字节 `DROPFILES` + UTF-16LE 双空终结 `CF_HDROP` 字节流。
pub fn build_hdrop_bytes(paths: &[PathBuf]) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    let mut out = Vec::with_capacity(256);
    // DROPFILES struct: pFiles(u32=20), pt.x(i32=0), pt.y(i32=0), fNC(i32=0), fWide(u32=1)
    out.extend_from_slice(&20u32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    for p in paths {
        for u in p.as_os_str().encode_wide() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

use windows::{
    Win32::{
        Foundation::{
            DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC,
            E_NOTIMPL, HGLOBAL, S_FALSE, S_OK,
        },
        System::{
            Com::{
                DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject,
                IDataObject_Impl, IEnumFORMATETC, IEnumFORMATETC_Impl, IEnumSTATDATA, STGMEDIUM,
                STGMEDIUM_0, TYMED_HGLOBAL,
            },
            Ole::{
                DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DoDragDrop, IDropSource,
                IDropSource_Impl, OleInitialize, OleUninitialize,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
    },
    core::{HRESULT, Ref, implement},
};

const CF_HDROP_ID: u16 = 15;

fn hdrop_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP_ID,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

#[implement(IEnumFORMATETC)]
struct HdropEnum {
    done: std::sync::atomic::AtomicBool,
}

impl IEnumFORMATETC_Impl for HdropEnum_Impl {
    fn Next(
        &self,
        celt: u32,
        rgelt: *mut FORMATETC,
        pceltfetched: *mut u32,
    ) -> HRESULT {
        if celt == 0 || rgelt.is_null() {
            return S_FALSE;
        }
        if !self.done.swap(true, std::sync::atomic::Ordering::AcqRel) {
            unsafe {
                *rgelt = hdrop_format();
                if !pceltfetched.is_null() {
                    *pceltfetched = 1;
                }
            }
            if celt == 1 { S_OK } else { S_FALSE }
        } else {
            if !pceltfetched.is_null() {
                unsafe {
                    *pceltfetched = 0;
                }
            }
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> windows::core::Result<()> {
        if celt > 0 {
            self.done.store(true, std::sync::atomic::Ordering::Release);
        }
        Ok(())
    }

    fn Reset(&self) -> windows::core::Result<()> {
        self.done.store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IEnumFORMATETC> {
        Ok(HdropEnum {
            done: std::sync::atomic::AtomicBool::new(
                self.done.load(std::sync::atomic::Ordering::Acquire),
            ),
        }
        .into())
    }
}

#[implement(IDataObject)]
struct HdropDataObject {
    bytes: Vec<u8>,
}

impl HdropDataObject {
    fn matches(fmt: *const FORMATETC) -> bool {
        if fmt.is_null() {
            return false;
        }
        let f = unsafe { &*fmt };
        f.cfFormat == CF_HDROP_ID && (f.tymed & (TYMED_HGLOBAL.0 as u32)) != 0
    }
}

impl IDataObject_Impl for HdropDataObject_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if !HdropDataObject::matches(pformatetcin) {
            return Err(windows::core::Error::from(DV_E_FORMATETC));
        }
        unsafe {
            let mem = crate::platform::GlobalAlloc(0x0002, self.bytes.len());
            if mem.is_null() {
                return Err(windows::core::Error::from_win32());
            }
            let ptr = crate::platform::GlobalLock(mem);
            if ptr.is_null() {
                crate::platform::GlobalFree(mem);
                return Err(windows::core::Error::from_win32());
            }
            std::ptr::copy_nonoverlapping(self.bytes.as_ptr(), ptr.cast::<u8>(), self.bytes.len());
            crate::platform::GlobalUnlock(mem);
            Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: STGMEDIUM_0 {
                    hGlobal: HGLOBAL(mem),
                },
                pUnkForRelease: ManuallyDrop::new(None),
            })
        }
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if HdropDataObject::matches(pformatetc) {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        if !pformatetcout.is_null() {
            unsafe {
                (*pformatetcout).ptd = std::ptr::null_mut();
            }
        }
        E_NOTIMPL
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: windows::core::BOOL,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        if dwdirection == DATADIR_GET.0 as u32 {
            Ok(HdropEnum {
                done: std::sync::atomic::AtomicBool::new(false),
            }
            .into())
        } else {
            Err(windows::core::Error::from(E_NOTIMPL))
        }
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Ref<'_, IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }
}

#[implement(IDropSource)]
struct FileDropSource;

impl IDropSource_Impl for FileDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        fescapepressed: windows::core::BOOL,
        grfkeystate: MODIFIERKEYS_FLAGS,
    ) -> HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        // MK_LBUTTON == 0x0001
        if (grfkeystate.0 & 1) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// 启动 Windows 原生跨进程文件拖出（可拖往桌面、资源管理器、微信/QQ、浏览器等）。
pub fn start_ole_file_drag(paths: &[PathBuf]) -> Result<(), String> {
    let valid: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
    if valid.is_empty() {
        return Err("无可拖出的有效文件".into());
    }
    let bytes = build_hdrop_bytes(&valid);
    unsafe {
        let init_res = OleInitialize(None);
        let data_obj: IDataObject = HdropDataObject { bytes }.into();
        let drop_src: IDropSource = FileDropSource.into();
        let mut effect = DROPEFFECT(0);
        let _hr = DoDragDrop(
            &data_obj,
            &drop_src,
            DROPEFFECT_COPY | DROPEFFECT_LINK,
            &mut effect,
        );
        if init_res.is_ok() {
            OleUninitialize();
        }
    }
    let _unused: Option<*mut c_void> = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdrop_buffer_layout() {
        let paths = [
            PathBuf::from("C:\\tmp\\a.txt"),
            PathBuf::from("C:\\tmp\\中文.png"),
        ];
        let buf = build_hdrop_bytes(&paths);
        assert!(buf.len() > 24);
        assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(buf[16..20].try_into().unwrap()), 1);
        assert_eq!(&buf[buf.len() - 4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn shelf_add_dedup_remove_and_persist() {
        let dir = std::env::temp_dir().join(format!("allegro-shelf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f1 = dir.join("note.txt");
        let f2 = dir.join("shot.png");
        std::fs::write(&f1, b"hello").unwrap();
        std::fs::write(&f2, b"1234567890").unwrap();

        let mut shelf = FileShelf::default();
        assert!(shelf.add_path(f1.clone()));
        assert!(!shelf.add_path(f1.clone()));
        assert!(shelf.add_path(f2.clone()));
        assert_eq!(shelf.items.len(), 2);
        assert_eq!(shelf.existing_paths().len(), 2);

        shelf.save(&dir).unwrap();
        let loaded = FileShelf::load(&dir);
        assert_eq!(loaded, shelf);

        shelf.remove_index(0);
        assert_eq!(shelf.items.len(), 1);
        shelf.clear();
        assert!(shelf.items.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn format_file_size() {
        assert_eq!(format_size(0), "文件夹");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(2 * 1024 * 1024), "2.0 MB");
    }
}
