//! Read-only, one-second telemetry; no hooks or input capture.
use std::time::{Instant,Duration};
use dotisle_domain::desktop_widgets::{Metrics,cpu_usage};
use windows::Win32::{Foundation::FILETIME,System::{Threading::GetSystemTimes,SystemInformation::{GlobalMemoryStatusEx,MEMORYSTATUSEX},Power::{GetSystemPowerStatus,SYSTEM_POWER_STATUS}}};
pub struct Hub{pub value:Metrics,last:Instant,previous:Option<(u64,u64,u64)>}
impl Default for Hub{fn default()->Self{Self{value:Metrics::default(),last:Instant::now()-Duration::from_secs(2),previous:None}}}
impl Hub{
 pub fn poll(&mut self){if self.last.elapsed()<Duration::from_secs(1){return}self.last=Instant::now();unsafe{
  let mut idle=FILETIME::default();let mut kernel=FILETIME::default();let mut user=FILETIME::default();
  if GetSystemTimes(Some(&mut idle),Some(&mut kernel),Some(&mut user)).is_ok(){let n=|v:FILETIME|(v.dwHighDateTime as u64)<<32|v.dwLowDateTime as u64;let sample=(n(idle),n(kernel),n(user));self.value.cpu_percent=self.previous.and_then(|p|cpu_usage(p,sample));self.previous=Some(sample);}else{self.value.cpu_percent=None;}
  let mut memory=MEMORYSTATUSEX{dwLength:std::mem::size_of::<MEMORYSTATUSEX>()as u32,..Default::default()};
  if GlobalMemoryStatusEx(&mut memory).is_ok(){self.value.memory_percent=Some(memory.dwMemoryLoad.min(100)as f64);self.value.memory_total_gib=Some(memory.ullTotalPhys as f64/1073741824.);self.value.memory_used_gib=Some(memory.ullTotalPhys.saturating_sub(memory.ullAvailPhys)as f64/1073741824.);}else{self.value.memory_percent=None;self.value.memory_total_gib=None;self.value.memory_used_gib=None;}
  let mut power=SYSTEM_POWER_STATUS::default();if GetSystemPowerStatus(&mut power).is_ok(){self.value.battery_percent=(power.BatteryFlag&128==0&&power.BatteryLifePercent<=100).then_some(power.BatteryLifePercent);self.value.external_power=match power.ACLineStatus{0=>Some(false),1=>Some(true),_=>None};}else{self.value.battery_percent=None;self.value.external_power=None;}
 }}
}

pub fn host_running(data:&std::path::Path)->bool{use windows::{core::PCWSTR,Win32::{System::Threading::{OpenMutexW,SYNCHRONIZATION_ACCESS_RIGHTS},Foundation::CloseHandle}};let name=dotisle_domain::desktop_widgets::host_key(data).encode_utf16().chain(Some(0)).collect::<Vec<_>>();unsafe{if let Ok(handle)=OpenMutexW(SYNCHRONIZATION_ACCESS_RIGHTS(0x00100000),false,PCWSTR(name.as_ptr())){let _=CloseHandle(handle);true}else{false}}}
