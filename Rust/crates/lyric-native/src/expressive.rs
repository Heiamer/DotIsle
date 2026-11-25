// 非音乐类活动卡片（番茄钟 / 计时器 / 秒表 / 待办清单），统一采用纯净通透曜光极简视觉语言。
impl Renderer{
    fn device_glyph(&mut self,r:Rect,kind:&str,ink:u32){
        let name=match kind{"Mouse"=>"mouse","Keyboard"=>"keyboard","Headphones"=>"headphones","Speaker"=>"speaker","Phone"=>"smartphone","Gamepad"=>"sports_esports","Volume"=>"hard_drive","Caps"=>"keyboard_capslock","Num"=>"dialpad","Mic"=>"mic","MicOff"=>"mic_off","Touchpad"=>"touchpad_mouse",_=>"usb"};
        self.clip(r,0.,(ink>>24)as f32/255.);dotisle_render::material_symbols::paint(&mut self.canvas,name,r.x,r.y,r.w,ink);self.canvas.pop_layer();
    }
    fn m3(&mut self,s:&Scene<'_>)->dotisle_domain::monet::Scheme{
        let mut c=self.dynamic_resolved.map(|r|r.roles.ui()).unwrap_or_else(||dotisle_domain::dynamic_color::roles(s.dynamic_custom_seed,true,false,s.dynamic_high_contrast).ui());
        if !s.expanded_cover_background{
            c.primary=s.component_color&0xffffff;
            c.on_primary=if crate::cover_color::contrast(c.primary,0xffffff)>=crate::cover_color::contrast(c.primary,0){0xffffff}else{0};
        }
        c
    }
    fn m3_button(&mut self,id:i32,r:Rect,label:&str,s:&Scene<'_>,primary:bool){
        let radius=(r.h/2.).min(18.);
        if primary{
            // 克制通透曜光玻璃胶囊主钮：高通透垂直浅霜渐变，绝不使用刺眼实心色块
            let top_a:u32=if s.pressed==id{0x44}else if s.hover==id{0x36}else{0x28};
            let bot_a:u32=if s.pressed==id{0x26}else if s.hover==id{0x1c}else{0x12};
            let grad=Gradient::new_linear((r.x,r.y),(r.x,r.y+r.h)).with_stops([
                (0.0,color((top_a<<24)|0xf6f9ff)),
                (1.0,color((bot_a<<24)|0xdbe6f6)),
            ]);
            self.canvas.fill(Fill::NonZero,Affine::IDENTITY,&grad,None,&RoundedRect::from_rect(vr(r),radius));
            self.outline(r,radius,if s.hover==id{0x44ffffff}else{0x2affffff},0.7);
            let feedback=s.feedback.get(id as usize).copied().unwrap_or(0.);
            if feedback>0.001{self.fill(r,radius,((feedback*180.)as u32)<<24|0xffffff);}
            self.text_at(label,Rect::new(r.x+8.,r.y+(r.h-20.)/2.,r.w-16.,22.),13.,600,0xfff8fbff,0.5);
        }else{
            // 无框轻盈幽灵次级钮：常态无白边底框，仅在悬停/按下时浮现柔和薄雾
            let fill_a:u32=if s.pressed==id{0x24}else if s.hover==id{0x16}else{0x0a};
            if fill_a>0{
                self.fill(r,radius,(fill_a<<24)|0xffffff);
            }
            let feedback=s.feedback.get(id as usize).copied().unwrap_or(0.);
            if feedback>0.001{self.fill(r,radius,((feedback*180.)as u32)<<24|0xffffff);}
            self.text_at(label,Rect::new(r.x+8.,r.y+(r.h-20.)/2.,r.w-16.,22.),12.5,500,if s.hover==id{0xfff8fbff}else{0xcce4ecf6},0.5);
        }
    }
    fn activity_close_button(&mut self,s:&Scene<'_>,id:i32,r:Rect){
        // 轻量关闭圆钮：去掉常态生硬灰圈与白框，悬停时再温和亮起
        let alpha:u32=if s.pressed==id{0x26}else if s.hover==id{0x18}else{0x00};
        if alpha>0{
            self.fill(r,r.w/2.,(alpha<<24)|0xffffff);
        }
        self.icon(Icon::Close,Rect::new(r.x+(r.w-14.)/2.,r.y+(r.h-14.)/2.,14.,14.),if s.hover==id{0xffeef4fc}else{0x96e2eaf4});
    }
    fn expressive_tools(&mut self,s:&Scene<'_>,p:&crate::activities::Panel){
    let w=s.layout.card.w;let h=s.layout.card.h;let c=self.m3(s);
    if p.kind==3{
        let style=dotisle_render::update_card::Style{primary:c.primary,on_primary:c.on_primary,on_surface:c.on_surface,muted:c.on_surface_variant,outline:c.outline};
        let view=dotisle_render::update_card::Presentation{title:&p.title,value:&p.value,subtitle:&p.subtitle,progress:p.progress,time_ms:s.time_ms,animate:s.animate,close:false,action:p.tasks.first().map(|t|t.title.as_str()),feedback:s.feedback[50],skip:s.update_phase=="available",skip_feedback:s.feedback.get(52).copied().unwrap_or(0.)};
        dotisle_render::update_card::paint(&mut self.canvas,&mut self.text,w,h,style,&view);
        self.activity_close_button(s,51,crate::layout::update_close(w));
        return;
    }

    // 统一冰白曜光文字色阶（彻底不随全局调试色偏染肉粉）
    let ink:u32=0xfff6f9ff;
    let muted:u32=0x98dde6f2;
    let soft_ice:u32=0xff8be0ff;

    // 极简无框顶栏：纯图标 + 纯白标题（完全去掉左上角方框）
    let header_icon=if p.kind==2{Icon::Todo}else{Icon::Focus};
    self.icon(header_icon,Rect::new(22.,20.,16.,16.),0xdce4f2ff);
    self.text_at(&p.title,Rect::new(46.,17.,(w-180.).max(60.),22.),13.5,600,ink,0.);
    self.activity_close_button(s,crate::layout::ACTIVITY_CLOSE,crate::layout::activity_close(w));

    if p.kind==2{
        // 重制待办清单：去盒化无边框极简行 + 极细分隔线 + 彩色圆环勾选 + 悬浮滑块
        let total=p.tasks.len();
        self.text_at(&format!("{total} 项"),Rect::new(w-116.,18.,60.,20.),11.5,500,0x8ae2ebf6,1.0);

        let vp=crate::layout::todo_viewport(w,h);
        if p.tasks.is_empty(){
            self.text_at("暂无待办事项",Rect::new(24.,82.,w-48.,22.),14.,600,ink,0.5);
            self.text_at("前往控制中心或点击下方管理待办",Rect::new(24.,108.,w-48.,18.),11.5,400,muted,0.5);
        }else{
            let max_scroll=crate::layout::todo_max_scroll(h,total);
            let scroll=s.todo_scroll.clamp(0.,max_scroll);
            self.clip(vp,8.,1.);
            for (i,t) in p.tasks.iter().enumerate(){
                let r=crate::layout::todo_row_rect(w,i,scroll);
                if r.y+r.h<vp.y-2. || r.y>vp.y+vp.h+2.{continue;}
                let id=1000+i as i32;
                let hovered=s.hover==id;
                let pressed=s.pressed==id;
                if hovered || pressed{
                    let row_a:u32=if pressed{0x22}else{0x14};
                    self.fill(r,10.,(row_a<<24)|0xffffff);
                }
                if i+1<total{
                    self.fill(Rect::new(r.x+10.,r.y+r.h+2.5,r.w-20.,0.6),0.3,0x14ffffff);
                }
                // 优先级色标融入圆形勾选环本身，干净优雅不杂乱
                let ring_col=if (t.color&0xffffff)!=0{0xff000000|(t.color&0xffffff)}else{soft_ice};
                let cx=r.x+16.;let cy=r.y+r.h/2.;
                if hovered{
                    self.fill(Rect::new(cx-7.5,cy-7.5,15.,15.),7.5,0x24000000|(ring_col&0xffffff));
                }
                self.ring(cx,cy,6.8,ring_col,1.65);
                self.text_line(&t.title,Rect::new(r.x+34.,r.y+4.5,r.w-42.,19.),12.5,500,ink);
                self.text_line(&t.detail,Rect::new(r.x+34.,r.y+22.5,r.w-42.,15.),10.,400,muted);
            }
            self.canvas.pop_layer();

            // 右侧无底框极细悬浮滚动条滑块
            if max_scroll>0.5{
                let bar_x=vp.x+vp.w-4.;
                let track_h=(vp.h-10.).max(14.);
                let visible_ratio=(vp.h/(vp.h+max_scroll)).clamp(0.18,0.85);
                let thumb_h=(track_h*visible_ratio).max(20.);
                let thumb_y=vp.y+5.+(track_h-thumb_h)*(scroll/max_scroll);
                self.fill(Rect::new(bar_x,thumb_y,2.5,thumb_h),1.25,0x5cffffff);
            }
        }
        let footer=crate::layout::todo_footer(w,h);
        self.m3_button(7,footer,"管理待办清单",s,false);
        return;
    }

    // 重制番茄钟 / 计时器 / 秒表：彻底去盒子去粗描边，直接呼吸在曜光主表面
    let detail=p.timer.as_ref();
    let clock=detail.map(|d|d.expanded_value.as_str()).unwrap_or(&p.value);
    let primary=detail.map(|d|d.primary.as_str()).unwrap_or(if p.running{"暂停"}else{"继续"});
    let state_label=if p.running{"进行中"}else{"已暂停"};
    // 顶部右侧极简无框状态呼吸点 + 纯文本
    let dot_col=if p.running{soft_ice}else{0x70ffffff};
    self.fill(Rect::new(w-108.,25.5,5.,5.),2.5,dot_col);
    self.text_at(state_label,Rect::new(w-98.,18.,44.,20.),11.,500,if p.running{0xdcf0f7ff}else{muted},0.);

    // 中央纯净高对比等宽倒计时大数字 + 副标题（直接悬浮于主玻璃面）
    let clock_sz=if clock.len()>8{31.}else if clock.len()>5{38.}else{46.};
    if s.timer_mono {
        self.text_mono(clock,Rect::new(24.,52.,w-48.,54.),clock_sz,600,ink,0.5);
    } else {
        self.text_at(clock,Rect::new(24.,52.,w-48.,54.),clock_sz,600,ink,0.5);
    }
    self.text_at(&p.subtitle,Rect::new(28.,109.,w-56.,18.),11.5,400,muted,0.5);

    // 与音乐布局一致的极细 3.5px 曜光流光进度条
    let track=Rect::new(26.,139.,w-52.,3.5);
    self.fill(track,1.75,0x1effffff);
    let frac=if detail.is_some_and(|d|d.indefinite){
        if p.running && s.animate{0.28+0.44*(0.5+0.5*(s.time_ms/1600.).sin())}else{0.28}
    }else{
        p.progress.clamp(0.,1.)
    };
    let fill_w=(track.w*frac).max(if frac>0.005{6.}else{0.});
    if fill_w>0.{
        let bar_rect=Rect::new(track.x,track.y,fill_w,track.h);
        self.canvas.draw_blurred_rounded_rect(Affine::IDENTITY,vr(bar_rect),color(0x469be4ff),1.75,3.5);
        let grad=Gradient::new_linear((bar_rect.x,bar_rect.y),(bar_rect.x+bar_rect.w,bar_rect.y)).with_stops([
            (0.0,color(0x9ec0ecff)),
            (1.0,color(0xffffffff)),
        ]);
        self.canvas.fill(Fill::NonZero,Affine::IDENTITY,&grad,None,&RoundedRect::from_rect(vr(bar_rect),1.75));
    }

    // 底部精致操控按钮（保持与 layout::timer_buttons 完全相同的命中区，但视觉收敛为轻盈居中主胶囊 + 幽灵辅钮）
    let buttons=crate::layout::timer_buttons(w,h);
    for(id,r)in buttons{
        if id==30{
            let pill_w=128_f64.min(r.w);
            let pill_h=34_f64.min(r.h);
            let visual=Rect::new(r.x+(r.w-pill_w)/2.,r.y+(r.h-pill_h)/2.,pill_w,pill_h);
            self.m3_button(id,visual,primary,s,true);
        }else{
            let ghost=Rect::new(r.x+2.,r.y+(r.h-32.)/2.,r.w-4.,32.);
            let active=s.hover==id || s.pressed==id;
            if active{
                let a:u32=if s.pressed==id{0x24}else{0x14};
                self.fill(ghost,16.,(a<<24)|0xffffff);
            }
            let feedback=s.feedback.get(id as usize).copied().unwrap_or(0.);
            if feedback>0.001{self.fill(ghost,16.,((feedback*180.)as u32)<<24|0xffffff);}
            self.text_at(if id==31{"重置"}else{"设置"},Rect::new(ghost.x,ghost.y+6.,ghost.w,20.),12.5,500,if s.hover==id{ink}else{0xbce4edf8},0.5);
        }
    }
}

}
