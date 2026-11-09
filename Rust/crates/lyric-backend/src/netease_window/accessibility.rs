//! Resolve the actual timeline row from the player's semantic play control.
//! CEF may omit the slider label; its raw accessibility containers still expose
//! the row geometry. Only inspect that play control's footer ancestry.
use std::cell::RefCell;
use windows::{core::{BSTR,Interface},Win32::{Foundation::{HWND,RECT},System::{Variant::VARIANT,Com::{CoInitializeEx,CoUninitialize,CoCreateInstance,COINIT_MULTITHREADED,CLSCTX_INPROC_SERVER}},UI::Accessibility::*}};
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize)]pub(super)struct Slider{pub left:i32,pub top:i32,pub right:i32,pub bottom:i32}
struct Com(bool);impl Drop for Com{fn drop(&mut self){if self.0{unsafe{CoUninitialize()}}}}
struct Context{automation:IUIAutomation,_apartment:Com}
thread_local!{static CONTEXT:RefCell<Option<Context>>=const{RefCell::new(None)};}
impl Context{unsafe fn new()->Option<Self>{
 let result=CoInitializeEx(None,COINIT_MULTITHREADED);let apartment=Com(result.is_ok());
 if result.is_err()&&result.0!=0x80010106u32 as i32{return None}
 let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER).ok()?;
 let timeouts=automation.cast::<IUIAutomation2>().ok()?;timeouts.SetConnectionTimeout(500).ok()?;timeouts.SetTransactionTimeout(500).ok()?;
 Some(Self{automation,_apartment:apartment})
}}
fn row(r:RECT,play:RECT,client:RECT,scale:f64)->bool{
 let(w,h)=(r.right-r.left,r.bottom-r.top);let center=(play.left+play.right)/2;
 w as f64>=160.*scale&&h>0&&h as f64<=24.*scale&&r.left<=center&&r.right>=center&&r.left>=client.left-2&&r.right<=client.right+2&&r.top>=client.bottom-(180.*scale)as i32&&r.bottom<=client.bottom+2
}
// The hidden native root may omit CEF descendants; connect to its provider first.
pub(super)fn slider(window:HWND,renderer:HWND,dpi:u32)->Option<Slider>{CONTEXT.with(|slot|{let mut slot=slot.borrow_mut();if slot.is_none(){*slot=unsafe{Context::new()}}let c=slot.as_ref()?;unsafe{resolve(c,renderer,renderer,dpi).or_else(||resolve(c,window,renderer,dpi))}})}
unsafe fn resolve(c:&Context,window:HWND,renderer:HWND,dpi:u32)->Option<Slider>{
 let root=c.automation.ElementFromHandle(window).ok()?;
 let condition=c.automation.CreatePropertyCondition(UIA_AutomationIdPropertyId,&VARIANT::from(BSTR::from("btn_pc_minibar_play"))).ok()?;
 let found=root.FindAll(TreeScope_Descendants,&condition).ok()?;if found.Length().ok()?!=1{return None}
 let mut current=found.GetElement(0).ok()?;let play=current.CurrentBoundingRectangle().ok()?;
 if play.right<=play.left||play.bottom<=play.top{return None}
 let renderer_element=c.automation.ElementFromHandle(renderer).ok()?;let client=renderer_element.CurrentBoundingRectangle().ok()?;
 let walker=c.automation.RawViewWalker().ok()?;let scale=dpi as f64/96.;let mut rows=Vec::new();
 for _ in 0..6{
  let parent=walker.GetParentElement(&current).ok()?;let role=parent.CurrentAriaRole().ok()?.to_string();
  let mut child=walker.GetFirstChildElement(&parent).ok();let mut count=0;
  while let Some(e)=child{if count>=32{break}count+=1;
   let rect=e.CurrentBoundingRectangle().ok()?;
   if row(rect,play,client,scale){
    // A classic progress row also contains the two time labels. Descend
    // through its sole wide group child to exclude both labels.
    let mut actual=rect;let mut inner=walker.GetFirstChildElement(&e).ok();let mut groups=Vec::new();let mut inner_count=0;
    while let Some(next)=inner{if inner_count>=16{break}inner_count+=1;let r=next.CurrentBoundingRectangle().ok()?;
     if next.CurrentControlType().ok()?==UIA_GroupControlTypeId&&row(r,play,client,scale){groups.push(r)}inner=walker.GetNextSiblingElement(&next).ok();
    }
    if groups.len()==1{actual=groups[0]}
    if !rows.iter().any(|r:&RECT|r.left==actual.left&&r.right==actual.right&&r.top==actual.top&&r.bottom==actual.bottom){rows.push(actual)}
   }
   child=walker.GetNextSiblingElement(&e).ok();
  }
  current=parent;if role=="contentinfo"{break}
  let rect=current.CurrentBoundingRectangle().ok()?;if rect.bottom-rect.top>(220.*scale)as i32{break}
 }
 if rows.len()!=1||client.right<=client.left||client.bottom<=client.top{return None}let r=rows[0];
 // Hidden native hosts can move while CEF retains its accessibility origin.
 // Use the renderer and row from the same provider coordinate space.
 Some(Slider{left:r.left-client.left,top:r.top-client.top,right:r.right-client.left,bottom:r.bottom-client.top})
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn only_a_wide_timeline_in_the_play_controls_footer_is_accepted(){let client=RECT{left:100,top:100,right:1156,bottom:852};let play=RECT{left:608,top:790,right:648,bottom:830};
  assert!(row(RECT{left:100,top:769,right:1156,bottom:772},play,client,1.));
  assert!(row(RECT{left:470,top:825,right:786,bottom:845},play,client,1.));
  for r in [RECT{left:100,top:100,right:1156,bottom:103},RECT{left:130,top:800,right:400,bottom:818},RECT{left:100,top:770,right:1156,bottom:852},RECT{left:600,top:800,right:650,bottom:810}]{assert!(!row(r,play,client,1.));}
 }
}
