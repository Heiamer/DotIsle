//! Swiss International: asymmetric grids, left-aligned grotesk type and a single red accent.
use dotisle_domain::monet::Scheme;
use vello::{Scene,kurbo::{Affine,BezPath,Stroke,Rect,Cap,Join},peniko::Fill};
use crate::{draw::{self,Icon,rect},widgets::Ui,model::{Model,Page,Action},views::{self,ViewState}};
pub fn active(m:&Model)->bool{m.prefs.control_center_theme=="Swiss"}
pub fn scheme(dark:bool)->Scheme{if dark{Scheme{
 primary:0xFF695A,on_primary:0x171717,primary_container:0x181818,on_primary_container:0xF6F4EE,secondary_container:0xFF695A,on_secondary_container:0x171717,tertiary_container:0x181818,on_tertiary_container:0xF6F4EE,surface:0x121212,container:0x181818,high:0x2A2A2A,on_surface:0xF6F4EE,on_surface_variant:0xB5B3AE,outline:0x777672,outline_variant:0x383735,error:0xFF695A,
}}else{Scheme{
 primary:0xC72B23,on_primary:0xFFFFFF,primary_container:0xFAF9F5,on_primary_container:0x151515,secondary_container:0xC72B23,on_secondary_container:0xFFFFFF,tertiary_container:0xFAF9F5,on_tertiary_container:0x151515,surface:0xF3F2ED,container:0xFAF9F5,high:0xE7E6DF,on_surface:0x151515,on_surface_variant:0x66655F,outline:0x96958E,outline_variant:0xD6D5CD,error:0xC72B23,
}}}
pub fn page(p:Page)->(&'static str,&'static str){match p{Page::Home=>("OVERVIEW","01"),Page::Island=>("THE ISLAND","02"),Page::Music=>("NOW PLAYING","03"),Page::Focus=>("FOCUS / TIME","04"),Page::Todos=>("THINGS TO DO","05"),Page::Settings=>("PREFERENCES","06")}}
pub fn panel(scene:&mut Scene,r:Rect,bg:u32,edge:u32){draw::fill(scene,r,0.,bg);draw::fill(scene,rect(r.x0,r.y0,r.width(),1.),0.,edge);}
pub fn icon(scene:&mut Scene,i:Icon,x:f64,y:f64,size:f64,color:u32){
 let path=match i{
 Icon::Home=>"M4 10L12 3L20 10V21H4ZM9 21V13H15V21",
 Icon::Music=>"M8 4V17M8 7L20 4V17M3 17H8V21H3ZM15 17H20V21H15Z",
 Icon::Focus=>"M9 2H15M12 2V5M5 7H19V21H5ZM12 10V15H16",
 Icon::Todo=>"M4 4H8V8H4ZM12 6H21M4 12H8V16H4ZM12 14H21M12 21H21",
 Icon::Settings=>"M3 6H21M3 12H21M3 18H21M7 3V9M17 9V15M10 15V21",
 Icon::Island=>"M3 6H21V18H3ZM8 10V14M12 9V15M16 10V14",
 Icon::Play=>"M7 4L20 12L7 20Z",Icon::Pause=>"M6 4H9V20H6ZM15 4H18V20H15Z",
 Icon::Prev=>"M18 4L6 12L18 20ZM4 4V20",Icon::Next=>"M6 4L18 12L6 20ZM20 4V20",
 Icon::Plus=>"M12 3V21M3 12H21",Icon::Check=>"M3 12L9 18L21 5",Icon::Close=>"M4 4L20 20M20 4L4 20",
 Icon::Arrow=>"M3 12H21M13 4L21 12L13 20",Icon::Back=>"M21 12H3M11 4L3 12L11 20",
 Icon::Search=>"M4 4H15V15H4ZM15 15L22 22",Icon::Stop=>"M5 5H19V19H5Z",
 Icon::Leaf=>"M4 20V9L10 3H21V14L15 20ZM4 20L16 8",
 Icon::Folder=>"M3 6H10L13 9H21V21H3Z",Icon::Import=>"M12 2V17M6 11L12 17L18 11M3 18V22H21V18",
 Icon::Volume=>"M3 9H7L13 3V21L7 15H3ZM18 7V17M22 4V20",
 Icon::Reset=>"M4 3V10H11M4 10L9 4H20V20H6",
 Icon::Sun=>"M8 8H16V16H8ZM12 1V5M12 19V23M1 12H5M19 12H23M3 3L6 6M18 18L21 21M3 21L6 18M18 6L21 3",
 Icon::Moon=>"M21 13A9 9 0 1 1 11 3A7 7 0 0 0 21 13Z",Icon::Heart=>"M3 4H9L12 7L15 4H21V12L12 21L3 12Z",
 Icon::Minimize=>"M4 12H20",Icon::Maximize=>"M4 4H20V20H4Z",Icon::Restore=>"M8 8V3H21V16H16M3 8H16V21H3Z",
 Icon::Translate=>"M3 5H13M8 2V5M11 5L4 15M5 8L13 15M14 22L18 11L22 22M15 18H21",
 Icon::Trash=>"M3 6H21M8 6V3H16V6M5 6V21H19V6M10 10V17M14 10V17",
 Icon::Move=>"M12 2V22M2 12H22M7 7L12 2L17 7M7 17L12 22L17 17M7 7L2 12L7 17M17 7L22 12L17 17",
 };
 if let Ok(p)=BezPath::from_svg(path){let tr=Affine::translate((x,y))*Affine::scale(size/24.);if matches!(i,Icon::Play|Icon::Pause|Icon::Stop){scene.fill(Fill::NonZero,tr,draw::color(color),None,&p);}else{scene.stroke(&Stroke::new(1.8).with_caps(Cap::Butt).with_join(Join::Miter),tr,draw::color(color),None,&p);}}
}
fn section(ui:&mut Ui,title:&str,x:f64,y:f64,w:f64){let s=ui.s;draw::fill(ui.scene,rect(x,y,w,2.),0.,s.on_surface);ui.label(title,x,y+10.,12.,s.primary,true,w);}
pub fn home(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64,state:&ViewState)->f64{
 let s=ui.s;let gap=28.;let left=(w-gap)*0.67;let right=w-left-gap;let rx=x+left+gap;
 section(ui,"01 / MUSIC",x,y,left);section(ui,"02 / FOCUS",rx,y,right);
 let cover=if left>600.{208.}else{160.};let tx=left-cover-32.;
 views::record(ui,m,rect(x+left-cover,y+54.,cover,cover),false);
 ui.label(m.title(),x,y+58.,if tx>300.{38.}else{28.},s.on_surface,true,tx);ui.label(m.artist(),x,y+118.,15.,s.on_surface_variant,false,tx);
 ui.label(m.current_lyric(),x,y+166.,13.,s.on_surface_variant,false,tx);
 views::player_controls(ui,m,x,y+226.,false);views::progress(ui,m,rect(x,y+294.,left,24.),state);
 let clock=m.current_timer().map(|t|t.clock()).unwrap_or("25:00".into());ui.label(&clock,rx,y+57.,if right>270.{64.}else{48.},s.on_surface,true,right);
 ui.label(&m.current_timer().map(|t|t.subtitle()).unwrap_or("25 分钟专注 · 5 分钟休息".into()),rx,y+164.,13.,s.on_surface_variant,false,right);
 ui.button(rect(rx,y+246.,right,50.),if m.current_timer().is_some(){"回到专注"}else{"开始专注"},Some(Icon::Arrow),Action::Navigate(Page::Focus),false,true);
 let y=y+366.;section(ui,"03 / NEXT",x,y,left);section(ui,"04 / YOUR ISLAND",rx,y,right);
 let now=dotisle_domain::todos::Now::local();let todo=m.todos.items.iter().filter(|t|!t.done(&now)).take(2).collect::<Vec<_>>();
 if todo.is_empty(){ui.label("少一点牵挂。",x,y+48.,30.,s.on_surface,true,left-180.);ui.label("记下一件事，再回到此刻。",x,y+100.,14.,s.on_surface_variant,false,left-180.);ui.button(rect(x+left-148.,y+65.,148.,46.),"添加待办",Some(Icon::Plus),Action::EditTodo(None),true,true);}
 else{for(i,t)in todo.iter().enumerate(){let ty=y+43.+i as f64*57.;ui.icon_button(rect(x,ty,40.,40.),Icon::Check,"完成待办",Action::ToggleTodo(t.id.clone()),false,true);ui.label(&t.title,x+54.,ty+3.,18.,s.on_surface,true,left-60.);ui.label(&format!("{} · {}",t.date,t.time),x+54.,ty+29.,12.,s.on_surface_variant,false,left-60.);}}
 ui.label("点屿",rx,y+46.,36.,s.on_surface,true,right);ui.label("音乐、专注与小事。",rx,y+101.,14.,s.on_surface_variant,false,right);
 ui.button(rect(rx,y+140.,right,40.),if m.prefs.island{"灵动岛已开启"}else{"开启灵动岛"},Some(Icon::Island),Action::Pref("island"),true,true);y+192.
}
pub fn music_head(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64,state:&ViewState){
 let s=ui.s;section(ui,"01 / THE RECORD",x,y,w);let cover=if w>900.{288.}else{224.};let px=x+cover+32.;let avail=w-cover-32.;views::record(ui,m,rect(x,y+38.,cover,cover),false);
 ui.label(if m.track(){"NOW PLAYING"}else{"WAITING FOR MUSIC"},px,y+39.,12.,s.primary,true,avail);ui.label(m.title(),px,y+75.,if avail>500.{44.}else{32.},s.on_surface,true,avail);ui.label(m.artist(),px,y+141.,17.,s.on_surface_variant,false,avail);
 views::progress(ui,m,rect(px,y+195.,avail,24.),state);views::player_controls(ui,m,px,y+253.,true);
}
