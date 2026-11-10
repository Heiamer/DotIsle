//! One settings hierarchy. Legacy page bodies remain reusable behind named destinations.
use crate::{widgets::Ui,model::{Model,Action},draw::{rect,Icon},views::ViewState};
const GROUPS:[(&str,&str);6]=[("外观与灵动岛","主题、排版、材质、字体与动效"),("音乐与歌词","播放器、歌词、进度与播放按钮"),("通知与提醒","应用通知、设备、音效与应用规则"),("计时与待办","专注习惯、到期提醒与默认颜色"),("通用与窗口","开机启动、窗口行为与快捷键"),("关于与更新","版本信息与自动更新")];
pub fn parent(tab:u8)->u8{match tab{0|128..=131|135=>240,1|132..=134=>241,2|5..=7=>242,8=>243,4=>244,3=>245,_=>255}}
fn name(tab:u8)->&'static str{match tab{135=>"灵动岛布局编辑器",0=>"控制中心外观",1=>"歌词与本地文件",2=>"应用通知与历史",3=>"关于与更新",4=>"启动、窗口与字体",5=>"设备连接提醒",6=>"提醒音效",7=>"按应用提醒",8=>"计时与待办偏好",128=>"灵动岛尺寸与排版",129=>"材质与取色",130=>"交互与系统提示",131=>"动效与圆角",132=>"播放器与同步",133=>"播放进度条",134=>"播放按钮主题",_=>"设置"}}
fn link(ui:&mut Ui,x:f64,y:f64,w:f64,title:&str,detail:&str,tab:u8){let s=ui.s;let r=rect(x,y,w,84.);ui.panel(r,s.container);let hit=ui.hit(r,title,Action::SettingsTab(tab),true);ui.state(r,24.,hit,s.on_surface);ui.label(title,x+22.,y+14.,17.,s.on_surface,true,w-80.);ui.label(detail,x+22.,y+47.,12.,s.on_surface_variant,false,w-80.);ui.icon(Icon::Arrow,x+w-46.,y+30.,22.,s.on_surface_variant);}
pub fn render(ui:&mut Ui,m:&Model,x:f64,mut y:f64,w:f64,state:&ViewState)->f64{
 let tab=m.settings_tab;let s=ui.s;if tab==135{return crate::layout_editor::render(ui,m,x,y,w,state)}
 if tab==255{ui.panel(rect(x,y,w,88.),s.high);ui.toggle(rect(x+22.,y+7.,(w-60.)/2.,70.),"灵动岛","音乐与提醒，轻盈相伴。",m.prefs.island,Action::Pref("island"));ui.toggle(rect(x+w/2.+12.,y+7.,w/2.-36.,70.),"减少动态效果","使用清晰、平静的切换。",m.prefs.reduced_motion,Action::Pref("motion"));y+=98.;let cw=(w-18.)/2.;for(i,(title,detail))in GROUPS.iter().enumerate(){link(ui,x+(i%2)as f64*(cw+18.),y+(i/2)as f64*94.,cw,title,detail,240+i as u8);}return y+274.;}
 let is_group=(240..246).contains(&tab);ui.button(rect(x,y,126.,40.),if is_group{"全部设置"}else{"返回分类"},Some(Icon::Back),Action::SettingsTab(if is_group{255}else{parent(tab)}),true,true);ui.label(if is_group{GROUPS[(tab-240)as usize].0}else{name(tab)},x+150.,y+7.,21.,s.on_surface,true,w-166.);y+=66.;
 if is_group{let entries:&[(u8,&str)]=match tab{
 240=>&[(135,"组合歌曲信息、歌词、播放按钮与进度条，预览并应用"),(0,"七套控制中心主题与背景"),(128,"模式、大小、字体与音乐排版"),(129,"胶囊、卡片与 Mini 的材质和配色"),(130,"悬停、手势、位置与系统状态提示"),(131,"形变曲线、圆角与高级弹簧参数")],
 241=>&[(135,"10 套播放按钮主题、8 种进度条与 16 种滑块"),(1,"翻译、高亮与本地歌词文件"),(132,"播放器响应范围、浏览器媒体与同步偏移"),],
 242=>&[(2,"接收、暂停、允许与屏蔽列表、历史保留"),(5,"耳机、音响、手柄与输入设备"),(6,"通知、待办计时与设备提醒音"),(7,"进入应用与使用时长规则")],
 243=>&[(8,"到期提醒、锁屏暂停与默认颜色")],244=>&[(4,"启动页面、开机启动、快捷键与字体范围")],_=>&[(3,"手写字标、版本信息与双源更新")],};for(leaf,detail)in entries{link(ui,x,y,w,name(*leaf),detail,*leaf);y+=100.;}return y;}
 if (128..135).contains(&tab){return crate::island_settings::render(ui,m,x,y,w,state)}crate::views::settings(ui,m,x,y,w)
}
