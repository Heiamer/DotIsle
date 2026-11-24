//! Cached resolved lyrics and read-only legacy SQLite song mapping.
use std::{ffi::{c_char,c_void,CStr,CString},path::PathBuf,sync::Arc};
use lyric_core::{cache::Cache,models::*,identity::normalized};
pub struct FinalCache(pub Arc<Cache>);
impl FinalCache{
 pub fn get(&self,key:&str)->Option<Selection>{self.0.get::<Selection>(key).filter(|r|r.content.usable())}
 pub fn put(&self,key:&str,value:&Selection){if value.content.usable(){self.0.put(key,value)}}
}

// Dynamically use the already distributed SQLite library; no managed EF runtime.
type Db=*mut c_void;
struct Sqlite { _library:windows::Win32::Foundation::HMODULE, open:unsafe extern "C" fn(*const c_char,*mut Db,i32,*const c_char)->i32,
    close:unsafe extern "C" fn(Db)->i32,prepare:unsafe extern "C" fn(Db,*const c_char,i32,*mut Db,*mut *const c_char)->i32,
    step:unsafe extern "C" fn(Db)->i32,text:unsafe extern "C" fn(Db,i32)->*const u8,int:unsafe extern "C" fn(Db,i32)->i32,finalize:unsafe extern "C" fn(Db)->i32 }
impl Sqlite{
    unsafe fn load()->Result<Self,String>{use windows::{Win32::System::LibraryLoader::{LoadLibraryW,GetProcAddress}};
        let path=std::env::current_exe().map_err(|e|e.to_string())?.parent().unwrap().join("e_sqlite3.dll");
        let wide=path.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let lib=LoadLibraryW(windows::core::PCWSTR(wide.as_ptr())).map_err(|e|e.to_string())?;
        macro_rules! symbol{($name:literal)=>{{let name=windows::core::PCSTR(concat!($name,"\0").as_ptr());std::mem::transmute(GetProcAddress(lib,name).ok_or(concat!("missing ",$name))?)}}}
        Ok(Self{_library:lib,open:symbol!("sqlite3_open_v2"),close:symbol!("sqlite3_close"),prepare:symbol!("sqlite3_prepare_v2"),step:symbol!("sqlite3_step"),text:symbol!("sqlite3_column_text"),int:symbol!("sqlite3_column_int"),finalize:symbol!("sqlite3_finalize")})
    }
}
impl Drop for Sqlite{fn drop(&mut self){unsafe{let _=windows::Win32::Foundation::FreeLibrary(self._library);}}}
pub struct MappingDb(pub PathBuf);
pub struct Mapping{pub title:String,pub artist:String,pub album:Option<String>,pub preferred_provider:Option<String>,pub is_pure_music:bool}
struct SongSearchMap{original_title:String,original_artist:String,original_album:String,mapped_title:String,mapped_artist:String,mapped_album:String,preferred_provider:Option<String>,is_marked_as_pure_music:bool}

impl MappingDb{
    pub fn resolve(&self,track:&Track)->Mapping{
        let mut result=Mapping{title:track.title.clone(),artist:track.artist.clone(),album:None,preferred_provider:None,is_pure_music:false};
        for row in unsafe{self.rows()}.unwrap_or_default(){
            if normalized(&row.original_title)!=normalized(&track.title)||normalized(&row.original_artist)!=normalized(&track.artist){continue}
            if !row.original_album.is_empty()&&normalized(&row.original_album)!=normalized(&track.album){continue}
            if !row.mapped_title.is_empty(){result.title=row.mapped_title}if !row.mapped_artist.is_empty(){result.artist=row.mapped_artist}if !row.mapped_album.is_empty(){result.album=Some(row.mapped_album)}result.preferred_provider=row.preferred_provider;result.is_pure_music=row.is_marked_as_pure_music;break
        }result
    }
}
impl MappingDb{
    unsafe fn rows(&self)->Result<Vec<SongSearchMap>,String>{
        if !self.0.exists(){return Ok(vec![])}let api=Sqlite::load()?;let mut db=null_mut();
        let path=CString::new(self.0.to_string_lossy().as_bytes()).map_err(|e|e.to_string())?;
        if (api.open)(path.as_ptr(),&mut db,1,std::ptr::null())!=0{if !db.is_null(){(api.close)(db);}return Err("mapping database open failed".into())}
        let sql=CString::new("SELECT OriginalTitle,OriginalArtist,OriginalAlbum,MappedTitle,MappedArtist,MappedAlbum,PreferredProvider,IsMarkedAsPureMusic FROM SongSearchMaps ORDER BY Id").unwrap();
        let mut statement=null_mut();let mut rows=vec![];
        if (api.prepare)(db,sql.as_ptr(),-1,&mut statement,null_mut())==0{
            while (api.step)(statement)==100{
                let col=|i|{let s=(api.text)(statement,i);if s.is_null(){String::new()}else{CStr::from_ptr(s as _).to_string_lossy().into_owned()}};
                let p=col(6);rows.push(SongSearchMap{original_title:col(0),original_artist:col(1),original_album:col(2),mapped_title:col(3),mapped_artist:col(4),mapped_album:col(5),preferred_provider:if p.is_empty(){None}else{Some(p)},is_marked_as_pure_music:(api.int)(statement,7)!=0});
            }
            (api.finalize)(statement);
        }
        (api.close)(db);Ok(rows)
    }
}
use std::ptr::null_mut;
