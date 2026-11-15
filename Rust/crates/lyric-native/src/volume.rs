//! Default playback endpoint only. Resolve per interaction so device switches
//! never leave the music controls attached to an unplugged device.
use std::time::{Duration,Instant};
use windows::{core::Result,Win32::{Media::Audio::{eRender,eMultimedia,IMMDeviceEnumerator,MMDeviceEnumerator,Endpoints::IAudioEndpointVolume},System::Com::{CoCreateInstance,CLSCTX_ALL}}};

pub const BUTTON:i32=16;
pub struct Volume{pub level:Option<u8>,pub muted:bool,remainder:i32,last_read:Instant}
impl Default for Volume{fn default()->Self{Self{level:None,muted:false,remainder:0,last_read:Instant::now()-Duration::from_secs(1)}}}
unsafe fn endpoint()->Result<IAudioEndpointVolume>{
 let enumerator:IMMDeviceEnumerator=CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL)?;
 enumerator.GetDefaultAudioEndpoint(eRender,eMultimedia)?.Activate(CLSCTX_ALL,None)
}
fn wheel_steps(remainder:&mut i32,delta:i16)->i32{*remainder+=delta as i32;let steps=*remainder/120;*remainder%=120;steps}
fn adjusted(level:f32,steps:i32)->f32{(level+steps as f32*0.02).clamp(0.,1.)}
impl Volume{
 pub fn refresh(&mut self,force:bool){
  if !force&&self.last_read.elapsed()<Duration::from_millis(500){return} self.last_read=Instant::now();
  let value=unsafe{endpoint().and_then(|e|Ok((e.GetMasterVolumeLevelScalar()?,e.GetMute()?.as_bool())))};
  match value{Ok((level,muted))=>{self.level=Some((level.clamp(0.,1.)*100.).round() as u8);self.muted=muted},Err(_)=>{self.level=None;self.muted=false}}
 }
 pub fn wheel(&mut self,delta:i16){
  let steps=wheel_steps(&mut self.remainder,delta);if steps==0{return}
  let result=unsafe{endpoint().and_then(|e|{let old=e.GetMasterVolumeLevelScalar()?;let next=adjusted(old,steps);e.SetMasterVolumeLevelScalar(next,std::ptr::null())?;if steps>0&&e.GetMute()?.as_bool(){e.SetMute(false,std::ptr::null())?;}Ok(())})};
  if result.is_err(){self.level=None}else{self.refresh(true)}
 }
 pub fn leave(&mut self){self.remainder=0;}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn precision_wheel_accumulates_and_reverses(){let mut r=0;assert_eq!(wheel_steps(&mut r,40),0);assert_eq!(wheel_steps(&mut r,40),0);assert_eq!(wheel_steps(&mut r,40),1);assert_eq!(r,0);assert_eq!(wheel_steps(&mut r,-60),0);assert_eq!(wheel_steps(&mut r,60),0);assert_eq!(wheel_steps(&mut r,-240),-2);}
 #[test]fn endpoints_are_bounded(){assert_eq!(adjusted(0.99,1),1.);assert_eq!(adjusted(0.01,-1),0.);assert!((adjusted(0.5,2)-0.54).abs()<0.0001);}
}
