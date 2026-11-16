use windows::{core::PCWSTR,Win32::{Foundation::{HWND,LPARAM,WPARAM},UI::{Input::KeyboardAndMouse::{SetFocus,EnableWindow,IsWindowEnabled},WindowsAndMessaging::*}}};

pub fn show(text:&str)->bool{unsafe{
 fn word(b:&mut Vec<u8>,v:u16){b.extend(v.to_le_bytes())}
 fn dword(b:&mut Vec<u8>,v:u32){b.extend(v.to_le_bytes())}
 fn wide(b:&mut Vec<u8>,s:&str){for c in s.encode_utf16().chain(Some(0)){word(b,c)}}
 fn control(b:&mut Vec<u8>,style:u32,rect:[u16;4],id:u16,class:u16,title:&str){while b.len()%4!=0{b.push(0)}dword(b,style);dword(b,0);for v in rect{word(b,v)}word(b,id);word(b,0xffff);word(b,class);wide(b,title);word(b,0)}
 let mut b=Vec::new();dword(&mut b,0x80c808c0);dword(&mut b,0);word(&mut b,4);for v in [0,0,450,280,0,0]{word(&mut b,v)}wide(&mut b,"点屿 · 使用声明与免责条款");word(&mut b,10);wide(&mut b,"Microsoft YaHei UI");
 control(&mut b,0x50a10844,[10,10,430,218],1001,0x81,"");control(&mut b,0x50000000,[10,234,430,12],1002,0x82,"请阅读全文，滚动至文末后才能同意。");control(&mut b,0x58010000,[210,252,110,18],6,0x80,"同意并继续");control(&mut b,0x50010001,[330,252,110,18],7,0x80,"不同意并退出");
 unsafe extern "system" fn callback(hwnd:HWND,msg:u32,wp:WPARAM,lp:LPARAM)->isize{match msg{
  WM_INITDIALOG=>{let _=SetDlgItemTextW(hwnd,1001,PCWSTR(lp.0 as *const u16));let _=SetFocus(GetDlgItem(Some(hwnd),7).ok());let _=SetTimer(Some(hwnd),1,120,None);0},
  WM_TIMER=>{if let Ok(edit)=GetDlgItem(Some(hwnd),1001){let mut info=SCROLLINFO{cbSize:std::mem::size_of::<SCROLLINFO>() as u32,fMask:SIF_ALL,..Default::default()};if GetScrollInfo(edit,SB_VERT,&mut info).is_ok()&&info.nPos as i64+info.nPage as i64>info.nMax as i64{if let Ok(button)=GetDlgItem(Some(hwnd),6){let _=EnableWindow(button,true);}let message="已到文末，请决定是否同意。".encode_utf16().chain(Some(0)).collect::<Vec<_>>();let _=SetDlgItemTextW(hwnd,1002,PCWSTR(message.as_ptr()));let _=KillTimer(Some(hwnd),1);}}1},
  WM_COMMAND=>{let id=wp.0&0xffff;if id==6&&!GetDlgItem(Some(hwnd),6).is_ok_and(|h|IsWindowEnabled(h).as_bool()){return 1}if id==6||id==7||id==2{let _=EndDialog(hwnd,id as isize);1}else{0}},
  WM_CLOSE=>{let _=EndDialog(hwnd,2);1},_=>0
 }}
 let text=text.replace("\r\n","\n").replace("\n","\r\n").encode_utf16().chain(Some(0)).collect::<Vec<_>>();let result=DialogBoxIndirectParamW(None,b.as_ptr().cast(),None,Some(callback),LPARAM(text.as_ptr() as isize));result==6
}}
