//! DotIsle drawer motion with analytic damping and Hermite continuity on interruption.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Spring { pub stiffness: f64, pub damping: f64, pub mass: f64 }
impl Default for Spring { fn default() -> Self { Self { stiffness:420., damping:30., mass:0.8 } } }
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Options { pub gap:f64, pub duration:f64, pub collapse_duration:f64, pub radius:f64, pub spring:Spring,pub curve:Option<[f64;4]> }
impl Default for Options { fn default()->Self { Self { gap:8.,duration:480.,collapse_duration:440.,radius:30.,spring:Spring::default(),curve:None } } }
#[derive(Clone, Copy, Default, Debug, Deserialize, Serialize)]
pub struct Frame {
    pub capsule_scale_y:f64, pub card_reveal:f64, pub card_offset_y:f64,
    pub card_opacity:f64, pub card_scale:f64, pub card_blur:f64,
    pub content_opacity:f64, pub content_offset_y:f64,
}
impl Frame {
    pub fn hidden(o:Options)->Self { Self { capsule_scale_y:1., card_reveal:0.,card_offset_y:-o.gap-10.,card_opacity:0.,card_scale:0.98,card_blur:6.,content_opacity:0.,content_offset_y:6. } }
    pub fn expanded()->Self { Self {capsule_scale_y:1.,card_reveal:1.,card_offset_y:0.,card_opacity:1.,card_scale:1.,card_blur:0.,content_opacity:1.,content_offset_y:0.} }
    pub fn values(self)->[f64;8] { [self.capsule_scale_y,self.card_reveal,self.card_offset_y,self.card_opacity,self.card_scale,self.card_blur,self.content_opacity,self.content_offset_y] }
}

const FLUID_SCALE_FLOOR:f64=0.98;

#[derive(Clone,Copy)]
pub struct Transition{origin:Frame,tangent:Frame,options:Options,reduced:bool,frequency:f64,ratio:f64,pub end:Frame,pub duration_ms:f64}
impl Transition{
    pub fn new(expanded:bool,origin:Frame,mut options:Options,tangent:Frame,reduced:bool)->Self{
        let clean=|n:f64,default:f64,lo:f64,hi:f64|if n.is_finite(){n.clamp(lo,hi)}else{default};
        options.spring.mass=clean(options.spring.mass,0.8,0.05,100.);
        options.spring.stiffness=clean(options.spring.stiffness,420.,1.,100000.);
        options.spring.damping=clean(options.spring.damping,30.,0.,10000.);
        let requested=clean(if expanded{options.duration}else{options.collapse_duration},480.,40.,5000.);
        let frequency=(options.spring.stiffness/options.spring.mass).sqrt()*360./requested;
        let ratio=(options.spring.damping/(2.*(options.spring.stiffness*options.spring.mass).sqrt())).max(1.);
        let slow=frequency/(ratio+(ratio*ratio-1.).sqrt());
        let duration_ms=if reduced{120.}else if options.curve.is_some(){requested}else{12000./slow};
        Self{origin,tangent,options,reduced,frequency,ratio,end:if expanded{Frame::expanded()}else{Frame::hidden(options)},duration_ms}
    }
    pub fn sample(&self,ms:f64)->Frame{self.at(ms).0}
    pub fn velocity(&self,ms:f64)->Frame{self.at(ms).1}
    fn at(&self,ms:f64)->(Frame,Frame){
        if ms>=self.duration_ms{return (self.end,Frame::default())}
        if ms<=0.||!ms.is_finite(){return (self.origin,self.tangent)}
        let seconds=ms/1000.;let total=self.duration_ms/1000.;let u=(seconds/total).clamp(0.,1.);
        if self.reduced{
            let mut frame=if self.end.card_reveal>0.{self.end}else{Frame::expanded()};
            frame.card_opacity=self.origin.card_opacity+(self.end.card_opacity-self.origin.card_opacity)*u;
            frame.content_opacity=self.origin.content_opacity+(self.end.content_opacity-self.origin.content_opacity)*u;
            return (frame,Frame::default())
        }
        let delta=self.end.card_reveal-self.origin.card_reveal;
        let (position,speed)=if let Some(curve)=self.options.curve{
            let (ease,slope)=lyric_core::motion_curve::sample(curve,u);
            let initial_slope=lyric_core::motion_curve::sample(curve,0.).1;
            let interrupted=(0.0001..0.9999).contains(&self.origin.card_reveal)||self.tangent.card_reveal.abs()>0.001;
            let extra=if interrupted{self.tangent.card_reveal-delta*initial_slope/total}else{0.};
            (self.origin.card_reveal+delta*ease+extra*total*u*(1.-u).powi(2),delta*slope/total+extra*(1.-u)*(1.-3.*u))
        }else{
            let (x,v)=damped(self.origin.card_reveal-self.end.card_reveal,self.tangent.card_reveal,self.frequency,self.ratio,seconds);
            (self.end.card_reveal+x,v)
        };
        let (mut values,mut rates)=self.channels(position,speed);
        let (start,start_rates)=self.channels(self.origin.card_reveal,self.tangent.card_reveal);
        let original=self.origin.values();let incoming=self.tangent.values();
        // A cubic Hermite residual joins every presentation channel continuously to the master.
        let h0=1.-3.*u*u+2.*u*u*u;let h1=u*(1.-u).powi(2);
        for i in 0..8{if i==1{continue}let offset=original[i]-start[i];let velocity=incoming[i]-start_rates[i];
            values[i]+=offset*h0+velocity*total*h1;
            rates[i]+=offset*6.*u*(u-1.)/total+velocity*(1.-u)*(1.-3.*u);
            if matches!(i,3|5|6){let maximum=if i==5{6.}else{1.};if values[i]<0.||values[i]>maximum{rates[i]=0.}values[i]=values[i].clamp(0.,maximum);}
        }
        (frame(values),frame(rates))
    }
    fn channels(&self,p:f64,v:f64)->([f64;8],[f64;8]){
        let q=p.clamp(0.,1.);let (opacity,d_opacity)=step(q,0.16,0.72);let (text,d_text)=step(q,0.24,0.90);
        let (size,d_size)=step(q,0.,0.76);let (focus,d_focus)=step(q,0.10,0.82);let lift=self.options.gap+10.;
        ([1.,p.clamp(0.,1.035),lift*(q-1.),opacity,FLUID_SCALE_FLOOR+(1.-FLUID_SCALE_FLOOR)*size,6.*(1.-focus),text,6.*(1.-text)],
         [0.,v,lift*v,d_opacity*v,(1.-FLUID_SCALE_FLOOR)*d_size*v,-6.*d_focus*v,d_text*v,-6.*d_text*v])
    }
}
fn frame(v:[f64;8])->Frame{Frame{capsule_scale_y:v[0],card_reveal:v[1],card_offset_y:v[2],card_opacity:v[3],card_scale:v[4],card_blur:v[5],content_opacity:v[6],content_offset_y:v[7]}}
fn damped(displacement:f64,velocity:f64,frequency:f64,ratio:f64,time:f64)->(f64,f64){
    if (ratio-1.).abs()<1e-6{let linear=velocity+frequency*displacement;let decay=(-frequency*time).exp();return ((displacement+linear*time)*decay,(linear-frequency*(displacement+linear*time))*decay)}
    let separation=(ratio*ratio-1.).sqrt();let slow=-frequency/(ratio+separation);let fast=-frequency*(ratio+separation);
    let slow_weight=(velocity-fast*displacement)/(slow-fast);let fast_weight=displacement-slow_weight;
    let a=slow_weight*(slow*time).exp();let b=fast_weight*(fast*time).exp();(a+b,slow*a+fast*b)
}
/// A smoothstep remap of `p` onto 0..1 across `[a,b]`, with its derivative in `p`.
///
/// The derivative is what lets one channel's velocity become the whole frame's velocity:
/// on a re-target every derived channel contributes `d/dp × dp/dt`, so nothing jumps.
fn step(p:f64,a:f64,b:f64)->(f64,f64){
    let span=(b-a).abs().max(1e-6);let t=((p-a)/span).clamp(0.,1.);
    (t*t*(3.-2.*t),6.*t*(1.-t)/span)
}
/// Spatially coupled softness stays continuous on reversal and costs nothing at rest.
/// The quartic envelope has zero slope at both endpoints; it never blurs settled text.
pub fn content_softness(reveal:f64,animate:bool)->(f64,f64){
    if !animate||!reveal.is_finite(){return (0.,0.)}
    let p=reveal.clamp(0.,1.);let envelope=16.*p*p*(1.-p)*(1.-p);
    let visible=|sigma:f64|if sigma<0.015{0.}else{sigma};
    (visible(1.35*envelope),visible(0.9*envelope))
}
/// Complementary weights keep one presentation readable throughout a reversible morph.
pub fn content_weights(frame:Frame)->(f64,f64){let expanded=frame.content_opacity.clamp(0.,1.);(expanded,1.-expanded)}
pub fn blend(p:f64,start:f64,end:f64)->f64 {let t=((p-start)/(end-start)).clamp(0.,1.);t*t*(3.-2.*t)}
pub fn sine(t:f64)->f64{(1.-(std::f64::consts::PI*t.clamp(0.,1.)).cos())/2.}
pub fn cubic_out(t:f64)->f64{1.-(1.-t.clamp(0.,1.)).powi(3)}
/// Rubber-banding for a drag taken past its own width.
///
/// A hard clamp reads as frozen — the surface stops dead under a finger that is still
/// moving. Real things slow down before they stop, so the excess is damped instead of
/// discarded: the further past the edge, the less of it you get.
pub fn rubber(dx:f64,width:f64)->f64{
    let limit=width.abs().max(1.);let over=dx.abs()-limit;
    if over<=0.{return dx.clamp(-limit,limit)}
    let constant=0.55;let resisted=(over*limit*constant)/(limit+constant*over);
    dx.signum()*(limit+resisted)
}
#[derive(Clone,Copy)]
pub struct Activity {pub playing:bool,started:f64,from:[f64;3],opacity_from:f64}
impl Activity {
    pub fn new()->Self{Self{playing:false,started:0.,from:[0.35;3],opacity_from:1.}}
    pub fn sample(&self,now:f64,animate:bool)->([f64;3],f64){
        let elapsed=(now-self.started).max(0.);let mut bars=[0.35;3];
        for i in 0..3{bars[i]=if self.playing&&animate{let period=320.+i as f64*95.;let phase=elapsed%(period*2.)/period;0.3+0.7*sine(if phase<=1.{phase}else{2.-phase})}else if self.playing{1.}else if animate{self.from[i]+(0.35-self.from[i])*cubic_out(elapsed/160.)}else{0.35};}
        let target=if self.playing{0.9}else{0.45};let opacity=if animate{self.opacity_from+(target-self.opacity_from)*sine(elapsed/180.)}else{target};(bars,opacity)
    }
    pub fn retarget(&mut self,playing:bool,now:f64,animate:bool){let (from,opacity)=self.sample(now,animate);self.from=from;self.opacity_from=opacity;self.started=now;self.playing=playing;}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn reversal_preserves_sampled_position_and_velocity() {
        let o=Options::default();let a=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        for t in [20.,90.,180.,320.] { let p=a.sample(t);let v=a.velocity(t);let b=Transition::new(false,p,o,v,false);
            assert_eq!(b.sample(0.).values(),p.values());assert_eq!(b.velocity(0.).values(),v.values());
            for (x,y) in b.sample(0.00001).values().iter().zip(p.values()) {assert!((x-y).abs()<0.0001);}
        }
    }
    #[test] fn damping_branches_settle_without_nan() {
        for damping in [10.,2.*(420_f64*0.8).sqrt(),80.] {let mut o=Options::default();o.spring.damping=damping;
            let a=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
            for i in 0..1000{assert!(a.sample(i as f64*2.).values().iter().all(|x|x.is_finite()));}
            assert_eq!(a.sample(a.duration_ms).values(),Frame::expanded().values());
        }
    }
    /// Endpoints must match the geometry used for drawing and pointer hit testing.
    #[test] fn fluid_lands_on_the_same_endpoints() {
        let o=Options::default();
        let open=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        assert_eq!(open.sample(0.).values(),Frame::hidden(o).values());
        assert_eq!(open.sample(open.duration_ms).values(),Frame::expanded().values());
        let close=Transition::new(false,Frame::expanded(),o,Frame::default(),false);
        assert_eq!(close.sample(close.duration_ms).values(),Frame::hidden(o).values());
    }
    #[test] fn fluid_is_monotonic_and_never_opens_from_zero() {
        let o=Options::default();let t=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        let mut previous=-1.;
        for i in 0..=400{let f=t.sample(i as f64);assert!(f.card_reveal>=previous-1e-9);previous=f.card_reveal;
            assert!(f.card_scale>=FLUID_SCALE_FLOOR-1e-9,"scale dipped to {}",f.card_scale);
            assert!((0.0..=1.0).contains(&f.card_opacity));}
    }
    /// Contents must not be readable before the container has room for them.
    #[test] fn fluid_contents_trail_the_outline() {
        let o=Options::default();let t=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        for i in 0..200{let f=t.sample(i as f64*3.);if f.content_opacity>0.05{assert!(f.card_reveal>0.3,"contents visible at reveal {}",f.card_reveal);}}
    }
    #[test] fn fluid_reverses_without_a_jump() {
        let o=Options::default();let a=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        for t in [30.,120.,260.] {let p=a.sample(t);let v=a.velocity(t);let b=Transition::new(false,p,o,v,false);
            for (x,y) in b.sample(0.).values().iter().zip(p.values()){assert!((x-y).abs()<1e-6,"jumped at {t}ms");}}
    }
    /// Reversal preserves every channel even with the longer throw timing.
    #[test]fn fluid_reversal_preserves_the_first_real_frame_and_all_channel_velocities(){
        let o=Options::default();let opening=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        for at in [10.,40.,90.,140.,230.]{let from=opening.sample(at);let speed=opening.velocity(at);let closing=Transition::new(false,from,o,speed,false);
            let dt=0.0001;let frame=closing.sample(dt);for ((start,next),expected) in from.values().into_iter().zip(frame.values()).zip(speed.values()){
                assert!((start-next).abs()<0.0001,"position discontinuity at {at}ms");
                assert!(((next-start)/(dt/1000.)-expected).abs()<0.04,"velocity discontinuity at {at}ms");
            }
        }
    }
    #[test] fn fluid_settles_within_the_drawer_budget() {
        let o=Options::default();
        let fluid=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
        assert!(fluid.duration_ms<760.,"drawer budget exceeded: {}",fluid.duration_ms);
    }
    #[test] fn rubber_band_resists_instead_of_stopping() {
        assert_eq!(rubber(10.,100.),10.);
        assert_eq!(rubber(100.,100.),100.);
        let past=rubber(200.,100.);
        assert!(past>100.&&past<200.);
        assert!(rubber(400.,100.)>past);
        assert_eq!(rubber(-200.,100.),-past);
    }
}

#[cfg(test)]mod curve_tests{use super::*;
 #[test]fn custom_curve_drives_the_real_master_channel_and_preserves_reversal(){let mut o=Options::default();o.curve=Some([0.,0.,1.,1.]);let t=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);assert!((t.sample(o.duration/2.).card_reveal-0.5).abs()<1e-6);let f=t.sample(180.);let v=t.velocity(180.);let r=Transition::new(false,f,o,v,false);assert_eq!(r.sample(0.).values(),f.values());assert_eq!(r.velocity(0.).values(),v.values());let near=(r.sample(0.001).card_reveal-f.card_reveal)/0.000001;assert!((near-v.card_reveal).abs()<0.01);assert_eq!(r.sample(r.duration_ms).card_reveal,0.);}
}

#[cfg(test)]mod softness_tests{
 use super::*;
 #[test]fn motion_softness_is_bounded_symmetric_and_crisp_at_rest(){
  for p in [0.,1.,-1.,2.,f64::NAN]{assert_eq!(content_softness(p,true),(0.,0.));}
  for i in 0..=100{let p=i as f64/100.;let(a,b)=content_softness(p,true);let(c,d)=content_softness(1.-p,true);assert!((0.0..=1.35).contains(&a));assert!((0.0..=0.9).contains(&b));assert!((a-c).abs()<1e-9&&(b-d).abs()<1e-9);assert_eq!(content_softness(p,false),(0.,0.));}
 }
 #[test]fn interrupted_shape_uses_the_same_softness_in_both_directions(){
  let o=Options::default();let opening=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
  for at in [40.,120.,240.]{let from=opening.sample(at);let v=opening.velocity(at);let closing=Transition::new(false,from,o,v,false);let p=closing.sample(0.);assert_eq!(content_softness(from.card_reveal,true),content_softness(p.card_reveal,true));}
 }
}

#[cfg(test)]mod content_handoff_tests{
 use super::*;
 #[test]fn every_forward_reverse_and_interrupted_frame_has_visible_content(){
  let o=Options::default();let open=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);
  for transition in [open,Transition::new(false,Frame::expanded(),o,Frame::default(),false),Transition::new(false,open.sample(140.),o,open.velocity(140.),false)]{
   for i in 0..=1000{let f=transition.sample(transition.duration_ms*i as f64/1000.);let(a,b)=content_weights(f);assert!((a+b-1.).abs()<1e-9);assert!(a.max(b)>=0.5);}
  }
 }
}
