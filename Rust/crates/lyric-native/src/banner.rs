//! 解析式二阶阻尼弹簧驱动的灵动岛通知/提醒表面，兼具 Apple 弹出果冻回吸与任意时刻打断连续性。
use crate::{layout::Rect,notifications::Notice};
#[derive(Clone,Copy,Debug)]
struct Spring{from:f64,to:f64,velocity:f64,time:f64,omega:f64,zeta:f64}
impl Default for Spring{
    fn default()->Self{Self{from:0.,to:0.,velocity:0.,time:0.,omega:20.,zeta:1.}}
}
impl Spring {
    fn sample(self,now:f64)->(f64,f64){
        let t=(now-self.time).max(0.);
        if t==0.||!self.omega.is_finite()||self.omega<=0.{return (self.from,self.velocity)}
        let d=self.from-self.to;
        if d==0.&&self.velocity==0.{return (self.to,0.)}
        let w=self.omega;let z=self.zeta.max(0.05);
        if (z-1.).abs()<=1e-5{
            let b=self.velocity+w*d;let e=(-w*t).exp();
            (self.to+(d+b*t)*e,(self.velocity-w*b*t)*e)
        }else if z<1.{
            let wd=w*(1.-z*z).sqrt();let a=d;let b=(self.velocity+z*w*d)/wd;
            let e=(-z*w*t).exp();let (s,c)=(wd*t).sin_cos();let env=a*c+b*s;
            (self.to+e*env,e*(-z*w*env+wd*(b*c-a*s)))
        }else{
            let s=(z*z-1.).sqrt();let r1=-w*(z-s);let r2=-w*(z+s);
            let c1=(self.velocity-r2*d)/(r1-r2);let c2=d-c1;
            let e1=c1*(r1*t).exp();let e2=c2*(r2*t).exp();
            (self.to+e1+e2,r1*e1+r2*e2)
        }
    }
    fn retarget_with(&mut self,to:f64,now:f64,omega:f64,zeta:f64){
        let (from,velocity)=self.sample(now);
        *self=Self{from,to,velocity,time:now,omega,zeta};
    }
    #[cfg(test)]
    fn retarget(&mut self,to:f64,now:f64){self.retarget_with(to,now,22.,1.);}
    fn snap(&mut self,to:f64,now:f64){*self=Self{from:to,to,time:now,velocity:0.,omega:20.,zeta:1.};}
}
pub struct Banner {pub notice:Option<Notice>,motion:[Spring;6],pub open:bool,deadline:f64,changed:f64,compact:bool,retiring:bool}
impl Default for Banner {fn default()->Self{Self{notice:None,motion:[Spring::default();6],open:false,deadline:0.,changed:0.,compact:false,retiring:false}}}
impl Banner {
    const OPEN_SPRING:[(f64,f64);6]=[(19.0,0.78),(21.5,0.80),(19.0,0.78),(21.5,0.80),(20.0,0.92),(22.0,1.0)];
    const CLOSE_SPRING:[(f64,f64);6]=[(22.5,0.92),(22.5,0.92),(22.5,0.92),(22.5,0.92),(22.5,0.96),(24.0,1.0)];
    /// An island-body activity with independently sized, unscaled typography.
    pub fn show_capsule(&mut self,notice:Notice,base:Rect,radius:f64,host:f64,y:f64,width:f64,height:f64,now:f64,animate:bool){
        if self.notice.is_none(){for(s,v)in self.motion.iter_mut().zip([base.x,base.y,base.w,base.h,radius,0.]){s.snap(v,now);}}
        self.notice=Some(notice);self.open=true;self.retiring=false;self.changed=now;self.deadline=now+5.;self.compact=true;
        let width=width.min(host-48.).max(140.);let height=height.clamp(40.,112.);
        for(i,(s,v))in self.motion.iter_mut().zip([(host-width)/2.,y,width,height,(height/2.).min(28.),1.]).enumerate(){
            if animate||i==5{let(w,z)=Self::OPEN_SPRING[i];s.retarget_with(v,now,w,z)}else{s.snap(v,now)}
        }
    }
    pub fn hold_until(&mut self,deadline:f64){self.deadline=deadline;}
    pub fn visible(&self)->bool{self.notice.is_some()}
    /// Whether the hold has run out. The shell asks this so a queued message can take
    /// over the open surface instead of closing it and opening it again.
    pub fn due(&self,now:f64)->bool{self.open&&now>=self.deadline-0.08}
    pub fn presentation(&self,now:f64)->(Rect,f64,f64){
        let a=self.motion.map(|s|s.sample(now).0);
        let w=a[2].max(4.);let h=a[3].max(4.);
        (Rect::new(a[0],a[1],w,h),a[4].clamp(0.,w.min(h)/2.),a[5].clamp(0.,1.))
    }
    pub fn show(&mut self,notice:Notice,base:Rect,radius:f64,host_width:f64,height:f64,now:f64,animate:bool){
        if self.notice.is_none(){for (s,v) in self.motion.iter_mut().zip([base.x,base.y,base.w,base.h,radius,0.]){s.snap(v,now);}}
        self.notice=Some(notice);self.open=true;self.retiring=false;self.changed=now;self.deadline=now+7.;self.compact=false;
        let width=440_f64.min(host_width-48.);
        let h=height.clamp(64.,98.);
        let target=[(host_width-width)/2.,base.y,width,h,24.,1.];
        for (i,(s,v)) in self.motion.iter_mut().zip(target).enumerate(){if animate{let(w,z)=Self::OPEN_SPRING[i];s.retarget_with(v,now,w,z)}else{s.snap(v,now)}}
    }
    /// A compact, non-interactive hint: same spring and interruption rules, shorter hold.
    pub fn show_compact(&mut self,notice:Notice,base:Rect,radius:f64,host_width:f64,now:f64,animate:bool){
        if self.notice.is_none(){for (s,v) in self.motion.iter_mut().zip([base.x,base.y,base.w,base.h,radius,0.]){s.snap(v,now);}}
        self.notice=Some(notice);self.open=true;self.retiring=false;self.changed=now;self.deadline=now+1.6;self.compact=true;
        let width=220_f64.min(host_width-48.);
        let target=[(host_width-width)/2.,base.y,width,44.,22.,1.];
        // Reduced motion still gets a short opacity fade; the shell never scales.
        for (i,(s,v)) in self.motion.iter_mut().zip(target).enumerate(){if animate||i==5{let(w,z)=Self::OPEN_SPRING[i];s.retarget_with(v,now,w,z)}else{s.snap(v,now)}}
    }
    fn close_target(&mut self,base:Rect,radius:f64,now:f64,restore:bool)->[f64;6]{
        if restore{self.retiring=false;return [base.x,base.y,base.w,base.h,radius,0.]}
        if !self.retiring{
            self.retiring=true;let(r,rad,_)=self.presentation(now);
            // There is no resting capsule: keep the notification's content-sized shell.
            return [r.x+4.,r.y-5.,(r.w-8.).max(1.),r.h,rad,0.]
        }
        self.motion.map(|s|s.to)
    }
    pub fn dismiss(&mut self,base:Rect,radius:f64,now:f64,animate:bool,restore:bool){
        if !self.visible()||!self.open{return}self.open=false;self.changed=now;
        let target=self.close_target(base,radius,now,restore);
        for (i,(s,v)) in self.motion.iter_mut().zip(target).enumerate(){if animate||(self.compact&&i==5){let(w,z)=Self::CLOSE_SPRING[i];s.retarget_with(v,now,w,z)}else{s.snap(v,now)}}
    }
    pub fn tick(&mut self,base:Rect,radius:f64,now:f64,hover:bool,animate:bool,restore:bool){
        if self.open {if hover{self.deadline=now+1.;}if now>=self.deadline{self.dismiss(base,radius,now,animate,restore);}}
        if !self.open&&self.visible(){
            let target=self.close_target(base,radius,now,restore);
            for (i,(s,v)) in self.motion.iter_mut().zip(target).enumerate(){if (s.to-v).abs()>0.001{if animate||(self.compact&&i==5){let(w,z)=Self::CLOSE_SPRING[i];s.retarget_with(v,now,w,z)}else{s.snap(v,now)}}}
            let settled=self.motion.iter().zip(target).all(|(s,v)|{let(p,vel)=s.sample(now);(p-v).abs()<0.1&&vel.abs()<0.5});
            let reduced=if self.compact{0.16}else{0.};
            if if animate{now-self.changed>if self.compact{0.32}else{0.42}&&settled}else{now-self.changed>reduced&&settled}{self.notice=None;}
        }
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn retarget_preserves_position_and_velocity(){let mut s=Spring::default();s.snap(260.,0.);s.retarget(440.,0.);let before=s.sample(0.08);s.retarget(260.,0.08);assert_eq!(s.sample(0.08),before);assert!(s.sample(0.09).0>before.0);}
    #[test]fn close_returns_to_original_surface(){let base=Rect::new(114.,24.,260.,40.);let mut b=Banner::default();b.show(Notice::default(),base,20.,488.,80.,0.,true);b.dismiss(base,20.,0.12,true,true);b.tick(base,20.,1.,false,true,true);assert!(!b.visible());assert!((b.presentation(2.).0.w-260.).abs()<0.001);}
}

#[cfg(test)]mod retirement_tests{
 use super::*;
 #[test]fn idle_notification_retires_without_visiting_a_blank_capsule(){
  let base=Rect::new(114.,24.,260.,40.);let mut b=Banner::default();b.show(Notice::default(),base,20.,488.,80.,0.,false);b.dismiss(base,20.,0.,true,false);
  for i in 0..=45{let now=i as f64/100.;b.tick(base,20.,now,false,true,false);let(r,_,_)=b.presentation(now);assert!(r.w>=431.);assert_eq!(r.h,80.);}
  b.tick(base,20.,0.8,false,true,false);assert!(!b.visible());
 }
 #[test]fn playback_arriving_mid_exit_preserves_the_live_surface(){
  let base=Rect::new(114.,24.,260.,40.);let mut b=Banner::default();b.show(Notice::default(),base,20.,488.,80.,0.,false);b.dismiss(base,20.,0.,true,false);let before=b.presentation(0.08);b.tick(base,20.,0.08,false,true,true);assert_eq!(before,b.presentation(0.08));b.tick(base,20.,1.,false,true,true);assert!(!b.visible());assert!((b.presentation(1.).0.w-base.w).abs()<0.01);
 }
}
