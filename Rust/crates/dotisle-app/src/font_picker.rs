//! System font chooser. Selecting a row only previews it; nothing is applied until 确认更换.
use vello::{kurbo::Affine,peniko::Fill};
use crate::{draw::{self,rect,Icon},model::{Action,Model},widgets::Ui};

pub const SAMPLE:&str="海边来信 · 让好音乐陪你";

pub struct FontPicker{
    pub families:Vec<String>,
    pub query:String,
    pub selected:Option<String>,
    pub scroll:f64,
    pub current:String,
}

impl FontPicker{
    pub fn new(current:&str)->Self{
        let families=dotisle_render::draw::Text::system_families();
        let selected=if current=="Noto Sans SC"{None}else{Some(current.to_owned())};
        Self{families,query:String::new(),selected,scroll:0.,current:current.to_owned()}
    }
    pub fn filtered(&self)->Vec<&String>{
        let q=self.query.trim().to_lowercase();
        if q.is_empty(){self.families.iter().collect()}else{self.families.iter().filter(|f|f.to_lowercase().contains(&q)).collect()}
    }
    pub fn preview_family(&self)->&str{self.selected.as_deref().filter(|f|*f!="Noto Sans SC").unwrap_or_else(||if self.current=="Noto Sans SC"{"Google Sans Flex"}else{&self.current})}
    pub fn input(&mut self,text:&str){
        let clean=text.chars().filter(|c|!c.is_control()).take(80).collect::<String>();
        if clean.is_empty(){return}
        self.query.push_str(&clean);self.scroll=0.;
    }
    pub fn backspace(&mut self){self.query.pop();self.scroll=0.;}
    pub fn scroll_by(&mut self,dy:f64,visible:usize){
        let max=(self.filtered().len().saturating_sub(visible))as f64;
        self.scroll=(self.scroll+dy).clamp(0.,max);
    }
    pub fn move_selection(&mut self,delta:i32,visible:usize){
        let rows=self.filtered().into_iter().cloned().collect::<Vec<_>>();
        if rows.is_empty(){return}
        let current=self.selected.as_ref().and_then(|s|rows.iter().position(|f|f==s)).map(|i|i as i32).unwrap_or(-delta.min(1));
        let next=(current+delta).clamp(0,rows.len()as i32-1)as usize;
        self.selected=Some(rows[next].clone());
        let max=(rows.len().saturating_sub(visible))as f64;
        let row=next as f64;
        if row<self.scroll{self.scroll=row;}
        if row>=self.scroll+visible as f64{self.scroll=(row-visible as f64+1.).min(max);}
    }
}

impl Model{
    pub fn open_font_picker(&mut self){
        let current=self.setting("IslandFontFamily").as_str().unwrap_or("Noto Sans SC").to_owned();
        self.dialog=None;self.palette=false;self.font_picker=Some(FontPicker::new(&current));
    }
    pub fn font_input(&mut self,text:&str){if let Some(p)=&mut self.font_picker{p.input(text);}}
    pub fn font_backspace(&mut self){if let Some(p)=&mut self.font_picker{p.backspace();}}
    pub fn font_scroll(&mut self,dy:f64){if let Some(p)=&mut self.font_picker{let visible=panel_rows();p.scroll_by(dy,visible);}}
    pub fn font_move(&mut self,delta:i32){if let Some(p)=&mut self.font_picker{p.move_selection(delta,panel_rows());}}
    pub fn select_font(&mut self,family:String){if let Some(p)=&mut self.font_picker{p.selected=Some(family);}}
    pub fn confirm_font(&mut self){
        let Some(p)=self.font_picker.take()else{return};
        let Some(family)=p.selected.as_ref().filter(|f|!f.is_empty()).cloned()else{self.font_picker=Some(p);return};
        self.native_state["settings"]["IslandFontFamily"]=serde_json::json!(family.clone());
        self.native_update("IslandFontFamily",serde_json::json!(family.clone()));
        self.message(format!("灵动岛字体已切换为 {family}"));
    }
    pub fn reset_font(&mut self){
        self.font_picker=None;
        self.native_state["settings"]["IslandFontFamily"]=serde_json::json!("Noto Sans SC");
        self.native_update("IslandFontFamily",serde_json::json!("Noto Sans SC"));
        self.message("已恢复默认字体");
    }
}

fn panel_rows()->usize{9}
const PANEL_W:f64=780.;
const PANEL_H:f64=588.;
const ROW_H:f64=34.;
const LIST_TOP:f64=128.;

pub fn panel(ui:&mut Ui,m:&Model,width:f64,height:f64){
    let Some(p)=m.font_picker.as_ref()else{return};let s=ui.s;ui.hits.clear();
    ui.scene.fill(Fill::NonZero,Affine::IDENTITY,draw::alpha(0,0.48),None,&rect(0.,0.,width,height));
    let x=(width-PANEL_W)/2.;let y=(height-PANEL_H)/2.;ui.panel(rect(x,y,PANEL_W,PANEL_H),s.surface);
    ui.label("选择字体 · 灵动岛",x+28.,y+20.,24.,s.on_surface,true,480.);
    ui.icon_button(rect(x+PANEL_W-64.,y+16.,44.,44.),Icon::Close,"取消选择字体",Action::FontClose,false,true);
    ui.label("搜索本机已安装的字体；选中后先预览，确认才会应用。",x+28.,y+58.,12.,s.on_surface_variant,false,PANEL_W-56.);

    let field=rect(x+28.,y+84.,420.,46.);draw::fill(ui.scene,field,12.,s.high);draw::outline(ui.scene,field,12.,s.primary,2.);
    ui.label(if p.query.is_empty(){"搜索字体名称"}else{&p.query},field.x0+14.,field.y0+12.,16.,if p.query.is_empty(){s.on_surface_variant}else{s.on_surface},false,field.width()-28.);
    if p.query.is_empty(){draw::line(ui.scene,(field.x0+15.,field.center().y-8.),(field.x0+15.,field.center().y+8.),s.primary,1.6);}
    ui.hit(field,"字体搜索框",Action::FontField,true);

    let rows=p.filtered();let list=rect(x+28.,y+LIST_TOP,420.,PANEL_H-LIST_TOP-84.);
    ui.clip(list);
    let visible=panel_rows();let start=p.scroll.floor().max(0.)as usize;
    for(i,family)in rows.iter().enumerate().skip(start).take(visible){
        let r=rect(list.x0,y+LIST_TOP+(i-start)as f64*ROW_H,list.width(),ROW_H-4.);
        let chosen=p.selected.as_deref()==Some(family.as_str());
        if chosen{draw::fill(ui.scene,r,10.,s.secondary_container);}
        let index=ui.hit(r,family,Action::FontSelect((*family).clone()),true);ui.state(r,10.,index,s.on_surface);
        ui.label(family,r.x0+14.,r.y0+7.,15.,if chosen{s.on_secondary_container}else{s.on_surface},chosen,r.width()-40.);
        if chosen{draw::icon(ui.scene,Icon::Check,r.x1-28.,r.y0+8.,16.,s.primary);}
    }
    ui.end_clip(rect(0.,0.,width,height));
    if rows.is_empty(){ui.label("没有匹配的字体",list.x0+14.,list.y0+10.,14.,s.on_surface_variant,false,list.width()-28.);}
    if rows.len()>visible{
        let track=rect(list.x1+6.,list.y0,4.,list.height());draw::fill(ui.scene,track,2.,s.outline_variant);
        let frac=visible as f64/rows.len()as f64;let thumb=(list.height()*frac).max(32.);let max=(rows.len()-visible)as f64;let pos=if max>0.{(p.scroll/max).clamp(0.,1.)*(list.height()-thumb)}else{0.};
        draw::fill(ui.scene,rect(track.x0,list.y0+pos,4.,thumb),2.,s.outline);
    }

    let rx=x+472.;let rw=PANEL_W-500.;
    ui.label("预览",rx,y+84.,13.,s.on_surface_variant,true,rw);
    ui.panel(rect(rx,y+104.,rw,176.),s.container);
    let sample=rect(rx+14.,y+122.,rw-28.,44.);draw::fill(ui.scene,sample,10.,s.high);
    ui.text.preview_line(ui.scene,p.preview_family(),SAMPLE,rect(sample.x0+12.,sample.y0,sample.width()-24.,sample.height()),19.,0xff000000|s.on_surface,500);
    ui.label("示例文字",rx+14.,y+180.,11.,s.on_surface_variant,false,rw-28.);
    let name=p.selected.as_deref().unwrap_or(if p.current=="Noto Sans SC"{"默认字体"}else{&p.current});
    ui.text.preview_line(ui.scene,p.preview_family(),name,rect(rx+14.,y+204.,rw-28.,40.),22.,0xff000000|s.primary,600);
    ui.label("字体名称",rx+14.,y+246.,11.,s.on_surface_variant,false,rw-28.);
    ui.label("选择只会预览；点确认更换后才会应用到灵动岛。",rx,y+300.,11.,s.on_surface_variant,false,rw);
    if p.selected.is_some(){ui.label("确认后会立即重新排版展开卡片与胶囊。",rx,y+320.,11.,s.on_surface_variant,false,rw);}

    ui.button(rect(x+28.,y+PANEL_H-62.,150.,44.),"恢复默认",None,Action::FontReset,true,true);
    ui.button(rect(x+PANEL_W-256.,y+PANEL_H-62.,100.,44.),"取消",None,Action::FontClose,true,true);
    ui.button(rect(x+PANEL_W-142.,y+PANEL_H-62.,114.,44.),"确认更换",None,Action::FontConfirm,false,p.selected.is_some());
}
