//! Internal module replacement, not a third-party plugin catalogue.
use crate::{draw::rect,model::{Action,Model,Page},widgets::Ui};use serde_json::{json,Value};
pub fn render(ui:&mut Ui,m:&Model,x:f64,mut y:f64,w:f64)->f64{
 let s=ui.s;let state=&m.native_state["moduleManagement"];let phase=state["phase"].as_str().unwrap_or("idle");let busy=matches!(phase,"checking"|"applying");
 ui.label("每个功能独立更新，CORE 也可以替换",x,y,19.,s.on_surface,true,w);y+=34.;
 ui.label("只选择可信的内部 DLL。校验通过后重启安装，启动失败会恢复原模块。",x,y,12.,s.on_surface_variant,false,w);y+=32.;
 ui.panel(rect(x,y,w,132.),s.container);ui.label(state["message"].as_str().unwrap_or("连接主程序后可以查看模块版本"),x+20.,y+16.,13.,s.on_surface,true,w-40.);
 ui.button(rect(x+20.,y+66.,142.,42.),"刷新模块",None,Action::NativeCommand("moduleRefresh",Value::Null),!busy,true);
 ui.button(rect(x+178.,y+66.,154.,42.),"安装并重启",None,Action::NativeCommand("moduleApply",json!({"callerSettingsPid":std::process::id()})),phase=="staged",true);
 ui.button(rect(x+348.,y+66.,142.,42.),"取消暂存",None,Action::NativeCommand("moduleCancel",Value::Null),phase=="staged",true);y+=152.;
 for(id,label)in dotisle_module_api::MODULES{
  let info=state["items"].as_array().and_then(|a|a.iter().find(|v|v["id"]==*id));let version=info.and_then(|v|v["version"].as_str()).unwrap_or("尚未读取");
  ui.panel(rect(x,y,w,112.),s.container);ui.label(label,x+20.,y+15.,17.,s.on_surface,true,w-365.);
  ui.label(&format!("dotisle_{id}.dll · {version} · API 1"),x+20.,y+55.,12.,s.on_surface_variant,false,w-365.);
  ui.button(rect(x+w-324.,y+34.,142.,42.),"选择替换 DLL",None,Action::ReplaceModule(id),!busy,true);
  ui.button(rect(x+w-166.,y+34.,146.,42.),"恢复上一版本",None,Action::NativeCommand("moduleRollback",json!({"id":id})),!busy,true);y+=128.;
 }y
}
pub fn health(out:&std::path::Path)->Result<(),String>{
 std::fs::create_dir_all(out).map_err(|e|e.to_string())?;let mut m=Model::new(out.join("data"),true);m.tutorial_checked=true;m.page=Page::Settings;m.settings_tab=9;m.prefs.reduced_motion=true;
 let mut text=crate::draw::Text::new();let view=crate::views::render(&m,&mut text,1200.,800.,&crate::views::ViewState::default());
 if !view.hits.iter().any(|h|matches!(h.action,Action::ReplaceModule("core"))){return Err("模块管理页面缺少 CORE 替换入口".into());}
 crate::gpu::Gpu::new().capture(&view.scene,1200,800,&out.join("modules.png"))?;
 std::fs::write(out.join("report.json"),json!({"success":true,"module_count":dotisle_module_api::MODULES.len(),"bootstrap_abi":1}).to_string()).map_err(|e|e.to_string())
}
