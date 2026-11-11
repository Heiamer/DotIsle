//! Restores update preferences using the production native IPC contract.
use crate::{widgets::Ui,model::{Model,Action},draw::{self,rect,Icon}};
use serde_json::{json,Value};
pub fn action(phase:&str)->Option<(&'static str,&'static str)>{match phase{
 "available"=>Some(("下载更新","updateDownload")),"ready"=>Some(("重启并安装","updateApply")),
 "downloading"|"switching"|"verifying"=>Some(("取消下载","updateCancel")),"skipped"=>Some(("恢复此次更新","updateUnskip")),
 "checking"|"applying"|"restarting"=>None,_=>Some(("检查更新","checkUpdate"))}}
pub fn render(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64)->f64{
 let s=ui.s;let top=y;
 ui.panel(rect(x,top,w,300.),s.high);
 dotisle_render::brand_signature::draw(ui.scene,rect(x+(w-310.)/2.,top+12.,310.,96.),1.,draw::color(s.on_surface));
 ui.center(&format!("版本 {}",env!("CARGO_PKG_VERSION")),rect(x,top+114.,w,24.),13.,s.on_surface_variant,false);
 let u=&m.native_state["softwareUpdate"];let phase=u["phase"].as_str().unwrap_or("idle");
 let status=match phase{"checking"=>"正在检查更新…","available"=>"发现新版本","downloading"=>"正在下载更新","switching"=>"正在切换更新源","verifying"=>"正在校验更新包","ready"=>"更新已就绪","applying"=>"正在安装更新","restarting"=>"正在重启点屿","completed"=>"更新已完成","failed"=>"更新未完成","cancelled"=>"已取消更新","skipped"=>"已跳过此次更新",_=>"当前已是最新版本"};
 let green=s.primary;
 let active=matches!(phase,"checking"|"downloading"|"switching"|"verifying"|"applying"|"restarting");
 let fraction=if phase=="downloading"{u["totalBytes"].as_u64().filter(|t|*t>0).map(|t|u["receivedBytes"].as_u64().unwrap_or(0)as f64/t as f64)}else if matches!(phase,"ready"|"completed"){Some(1.)}else{None};
 let version=match phase{"available"|"downloading"|"switching"|"verifying"|"ready"|"applying"|"restarting"|"completed"|"failed"|"skipped"=>u["version"].as_str().unwrap_or(""),_=>""};let title=if version.is_empty(){status.to_owned()}else{format!("{status}  {version}")};
 ui.center(&title,rect(x+24.,top+152.,w-48.,26.),16.,if phase=="failed"{s.error}else{s.on_surface},true);
 if let Some(p)=fraction{let track=rect(x+(w-240.)/2.,top+184.,240.,4.);draw::fill(ui.scene,track,2.,s.outline_variant);draw::fill(ui.scene,rect(track.x0,track.y0,track.width()*p.clamp(0.,1.),4.),2.,green);}
 let bytes=|v:u64|format!("{:.1} MB",v as f64/1048576.);
 let detail=if phase=="downloading"{let received=bytes(u["receivedBytes"].as_u64().unwrap_or(0));match u["totalBytes"].as_u64().filter(|t|*t>0){Some(t)=>format!("已下载 {received} / {}",bytes(t)),None=>format!("已下载 {received} · 总大小未知")}}else{u["message"].as_str().filter(|v|!v.is_empty()).unwrap_or(if m.demo{"设计预览；实际更新由主进程处理"}else{"双源检查 · 保留本机设置与数据"}).to_owned()};
 ui.center(&detail,rect(x+24.,top+198.,w-48.,20.),12.,s.on_surface_variant,false);
 if let Some((label,command))=action(phase){ui.button(rect(x+(w-180.)/2.,top+238.,180.,44.),label,None,Action::NativeCommand(command,Value::Null),false,!m.demo);}
 else{ui.center(if active{"请稍候，当前任务正在处理"}else{"为专注与聆听留一点空间"},rect(x+24.,top+244.,w-48.,24.),13.,s.on_surface_variant,false);}
 let on=m.setting("AutoCheckUpdatesEnabled").as_bool().unwrap_or(true);ui.panel(rect(x,top+316.,w,88.),s.container);ui.toggle(rect(x+24.,top+324.,w-48.,72.),"自动检查更新","后台定期检查；下载和安装均由你确认",on,Action::NativeEdit("AutoCheckUpdatesEnabled",json!(!on)));
 top+420.

}
#[cfg(test)]mod tests{use super::*;#[test]fn actions_are_explicit_and_phase_safe(){assert_eq!(action("available").unwrap().1,"updateDownload");assert_eq!(action("ready").unwrap().1,"updateApply");for p in ["checking","applying","restarting"]{assert!(action(p).is_none());}for p in ["downloading","switching","verifying"]{assert_eq!(action(p).unwrap().1,"updateCancel");}}}
