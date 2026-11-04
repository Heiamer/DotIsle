//! Progress-bar and thumb styles shared by the control centre and the expanded island card.
//! Only seekable playback bars use these; passive progress indicators keep their own drawing.
use vello::{Scene,kurbo::{Affine,BezPath,Cap,Circle,Point,Rect,RoundedRect,Shape,Size,Stroke},peniko::{Color,Fill,Gradient}};

const TAU:f64=std::f64::consts::TAU;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BarStyle{Normal,Wavy,Rounded,Thin,Thick,Gradient,Segmented,Dots}
impl BarStyle{
    pub const ALL:[Self;8]=[Self::Normal,Self::Wavy,Self::Rounded,Self::Thin,Self::Thick,Self::Gradient,Self::Segmented,Self::Dots];
    pub fn key(self)->&'static str{match self{Self::Normal=>"Normal",Self::Wavy=>"Wavy",Self::Rounded=>"Rounded",Self::Thin=>"Thin",Self::Thick=>"Thick",Self::Gradient=>"Gradient",Self::Segmented=>"Segmented",Self::Dots=>"Dots"}}
    pub fn detail(self)->&'static str{match self{Self::Normal=>"标准填充",Self::Wavy=>"随播放起伏的波浪",Self::Rounded=>"两端完全圆角",Self::Thin=>"纤细线条",Self::Thick=>"加粗填充",Self::Gradient=>"主色渐变",Self::Segmented=>"分段刻度",Self::Dots=>"圆点串联"}}
    /// Unknown values keep the shipped Wavy bar instead of failing to render.
    pub fn from_setting(s:&str)->Self{match s{"Normal"=>Self::Normal,"Wavy"|""=>Self::Wavy,"Rounded"=>Self::Rounded,"Thin"=>Self::Thin,"Thick"=>Self::Thick,"Gradient"=>Self::Gradient,"Segmented"=>Self::Segmented,"Dots"=>Self::Dots,_=>Self::Wavy}}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ThumbStyle{None,Default,Circle,Square,Pill,Diamond,Flower,Heart,Cookie,Puffy,Clover,Clover8,Gem,Triangle,Pentagon,Cookie12,Clam}
impl ThumbStyle{
    /// Selectable thumbs. `None` is reserved for the disabled switch and is not offered here.
    pub const ALL:[Self;16]=[Self::Default,Self::Circle,Self::Square,Self::Pill,Self::Diamond,Self::Flower,Self::Heart,Self::Cookie,Self::Puffy,Self::Clover,Self::Clover8,Self::Gem,Self::Triangle,Self::Pentagon,Self::Cookie12,Self::Clam];
    pub fn key(self)->&'static str{match self{Self::None=>"None",Self::Default=>"Default",Self::Circle=>"Circle",Self::Square=>"Square",Self::Pill=>"Pill",Self::Diamond=>"Diamond",Self::Flower=>"Flower",Self::Heart=>"Heart",Self::Cookie=>"Cookie",Self::Puffy=>"Puffy",Self::Clover=>"Clover",Self::Clover8=>"Clover8",Self::Gem=>"Gem",Self::Triangle=>"Triangle",Self::Pentagon=>"Pentagon",Self::Cookie12=>"Cookie12",Self::Clam=>"Clam"}}
    pub fn label(self)->&'static str{match self{Self::Clover8=>"Clover 8",Self::Cookie12=>"Cookie 12",_=>self.key()}}
    /// Unknown values keep the shipped vertical pill.
    pub fn from_setting(s:&str)->Self{Self::ALL.into_iter().find(|t|t.key()==s).unwrap_or(Self::Pill)}
}

/// Colors are ARGB so the island can keep its translucent rails.
#[derive(Clone,Copy)]
pub struct Look{pub fg:u32,pub track:u32,pub dot:u32}

fn color(argb:u32)->Color{Color::from_rgba8((argb>>16)as u8,(argb>>8)as u8,argb as u8,(argb>>24)as u8)}
fn mix(a:u32,b:u32,t:f64)->u32{
    let t=t.clamp(0.,1.);let c=|s:u32|{let x=((a>>s)&0xff)as f64;let y=((b>>s)&0xff)as f64;(x+(y-x)*t).round()as u32};
    (c(24)<<24)|(c(16)<<16)|(c(8)<<8)|c(0)
}
fn rect(x:f64,y:f64,w:f64,h:f64)->Rect{Rect::new(x,y,x+w,y+h)}

/// Draws the track, the played portion and the thumb for a seekable playback bar.
/// `unit` is 1.0 for the control centre and scales down inside the island.
pub fn draw(scene:&mut Scene,r:Rect,fraction:f64,phase:f64,unit:f64,thumb_size:f64,wave:f64,spin:f64,look:Look,bar:BarStyle,thumb:ThumbStyle){
    let fraction=if fraction.is_finite(){fraction.clamp(0.,1.)}else{0.};let wave=wave.clamp(0.,1.);
    let unit=unit.max(0.2);let mid=r.center().y;let end=r.x0+r.width()*fraction;
    let fg=color(look.fg);let track=color(look.track);
    match bar{
        BarStyle::Normal|BarStyle::Rounded|BarStyle::Thick|BarStyle::Gradient=>{
            let h=if bar==BarStyle::Thick{12.*unit}else{8.*unit};
            let corner=if bar==BarStyle::Normal{2.*unit}else{h/2.};
            scene.fill(Fill::NonZero,Affine::IDENTITY,track,None,&RoundedRect::from_rect(rect(r.x0,mid-h/2.,r.width(),h),corner));
            if end>r.x0+0.5{
                let fill=rect(r.x0,mid-h/2.,(end-r.x0).max(h*0.5),h);
                if bar==BarStyle::Gradient{
                    let gradient=Gradient::new_linear((r.x0,mid),(end.max(r.x0+1.),mid)).with_stops([fg,color(mix(look.fg,0x6C5CE7,0.55))]);
                    scene.fill(Fill::NonZero,Affine::IDENTITY,&gradient,None,&RoundedRect::from_rect(fill,corner));
                }else{scene.fill(Fill::NonZero,Affine::IDENTITY,fg,None,&RoundedRect::from_rect(fill,corner));}
            }
            stop_dot(scene,r,mid,2.4*unit,look.dot,fraction);
        }
        BarStyle::Wavy=>{
            let h=(2.*unit).max(1.);
            scene.fill(Fill::NonZero,Affine::IDENTITY,track,None,&RoundedRect::from_rect(rect(r.x0,mid-h/2.,r.width(),h),h/2.));
            let length=end-r.x0;
            if length>1.{
                let mut path=BezPath::new();let count=(length/3.).ceil().max(2.)as usize;
                for i in 0..=count{
                    let x=r.x0+length*i as f64/count as f64;
                    let edge=((x-r.x0)/(10.*unit)).min((end-x)/(10.*unit)).clamp(0.,1.);
                    let y=mid+edge*2.5*unit*wave*((x-r.x0)/(26.*unit)*TAU-phase).sin();
                    if i==0{path.move_to((x,y))}else{path.line_to((x,y))}
                }
                scene.stroke(&Stroke::new(4.*unit).with_caps(Cap::Round),Affine::IDENTITY,fg,None,&path);
            }
            stop_dot(scene,r,mid,2.*unit,look.dot,fraction);
        }
        BarStyle::Thin=>{
            let h=(1.6*unit).max(1.);
            segment(scene,(r.x0,mid),(r.x1,mid),h,track);
            if end>r.x0+0.5{segment(scene,(r.x0,mid),(end,mid),3.*unit,fg);}
            stop_dot(scene,r,mid,2.*unit,look.dot,fraction);
        }
        BarStyle::Segmented=>{
            let tick=3.5*unit;let h=12.*unit;let period=22.*unit;
            let count=(r.width()/period).floor()as usize;
            for i in 0..=count{
                let x=r.x0+i as f64*period;
                if x+tick>r.x1+0.01{break}
                let lit=x+tick/2.<=end;
                scene.fill(Fill::NonZero,Affine::IDENTITY,if lit{fg}else{track},None,&RoundedRect::from_rect(rect(x,mid-h/2.,tick,h),tick/2.));
            }
        }
        BarStyle::Dots=>{
            let d=9.*unit;let period=24.*unit;
            let count=(r.width()/period).floor()as usize;
            for i in 0..=count{
                let x=r.x0+d/2.+i as f64*period;
                if x+d/2.>r.x1+0.01{break}
                let lit=x<=end;
                scene.fill(Fill::NonZero,Affine::IDENTITY,if lit{fg}else{track},None,&Circle::new((x,mid),d/2.));
            }
        }
    }
    if thumb!=ThumbStyle::None{scene.fill(Fill::NonZero,Affine::IDENTITY,fg,None,&thumb_path(thumb,end,mid,thumb_size.max(6.),spin));}
}

fn segment(scene:&mut Scene,a:(f64,f64),b:(f64,f64),width:f64,c:Color){
    scene.stroke(&Stroke::new(width).with_caps(Cap::Round),Affine::IDENTITY,c,None,&vello::kurbo::Line::new(a,b));
}
fn stop_dot(scene:&mut Scene,r:Rect,mid:f64,radius:f64,c:u32,fraction:f64){
    if fraction<0.985{scene.fill(Fill::NonZero,Affine::IDENTITY,color(c),None,&Circle::new((r.x1-radius-0.5,mid),radius));}
}

/// Closed Catmull-Rom through the samples keeps organic thumbs smooth at any size.
fn smooth_closed(points:&[(f64,f64)])->BezPath{
    let n=points.len();let mut path=BezPath::new();if n<3{return path}
    let at=|i:isize|points[(((i%n as isize)+n as isize)%n as isize)as usize];
    path.move_to(points[0]);
    for i in 0..n{
        let p0=at(i as isize-1);let p1=at(i as isize);let p2=at(i as isize+1);let p3=at(i as isize+2);
        path.curve_to((p1.0+(p2.0-p0.0)/6.,p1.1+(p2.1-p0.1)/6.),(p2.0-(p3.0-p1.0)/6.,p2.1-(p3.1-p1.1)/6.),p2);
    }
    path.close_path();path
}
fn lobed(cx:f64,cy:f64,outer:f64,inner:f64,lobes:usize,twist:f64)->BezPath{
    let n=(lobes*12).clamp(48,192);let points=(0..n).map(|i|{let a=twist+TAU*i as f64/n as f64;let bump=0.5+0.5*(a*lobes as f64).cos();let r=inner+(outer-inner)*bump;(cx+a.cos()*r,cy+a.sin()*r)}).collect::<Vec<_>>();
    smooth_closed(&points)
}
/// Overlapping circles read as a clover or a puffy cloud at tiny sizes, where a
/// radial scallop would collapse into a star.
fn circled(cx:f64,cy:f64,r:f64,lobes:usize,ring:f64,radius:f64)->BezPath{
    let mut path=BezPath::new();
    for i in 0..lobes{let a=-std::f64::consts::FRAC_PI_2+TAU*i as f64/lobes as f64;let c=Circle::new((cx+a.cos()*r*ring,cy+a.sin()*r*ring),r*radius).to_path(0.1);for el in c.elements(){path.push(*el);}}
    let c=Circle::new((cx,cy),r*radius*0.9).to_path(0.1);for el in c.elements(){path.push(*el);}
    path
}
/// Straight edges with cut corners keep diamonds and triangles recognisable at 20 px.
fn rounded_polygon(cx:f64,cy:f64,r:f64,sides:usize,cut:f64)->BezPath{
    let corners=polygon(cx,cy,r,sides);let n=corners.len();let mut path=BezPath::new();
    for i in 0..n{
        let prev=corners[(i+n-1)%n];let p=corners[i];let next=corners[(i+1)%n];
        let unit=|a:(f64,f64),b:(f64,f64)|{let d=((b.0-a.0).powi(2)+(b.1-a.1).powi(2)).sqrt().max(1e-6);((b.0-a.0)/d,(b.1-a.1)/d)};
        let a=(p.0+unit(p,prev).0*cut,p.1+unit(p,prev).1*cut);let b=(p.0+unit(p,next).0*cut,p.1+unit(p,next).1*cut);
        if i==0{path.move_to(a);}else{path.line_to(a);}
        path.quad_to(p,b);
    }
    path.close_path();path
}
fn polygon(cx:f64,cy:f64,r:f64,sides:usize)->Vec<(f64,f64)>{
    (0..sides).map(|i|{let a=-std::f64::consts::FRAC_PI_2+TAU*i as f64/sides as f64;(cx+a.cos()*r,cy+a.sin()*r)}).collect()
}
fn heart(cx:f64,cy:f64,r:f64)->BezPath{
    let s=r*0.95;let p=|x:f64,y:f64|(cx+x*s,cy+y*s);let mut path=BezPath::new();
    path.move_to(p(0.,0.95));
    path.curve_to(p(-0.18,0.62),p(-0.98,0.3),p(-0.95,-0.22));
    path.curve_to(p(-0.92,-0.78),p(-0.3,-0.95),p(0.,-0.5));
    path.curve_to(p(0.3,-0.95),p(0.92,-0.78),p(0.95,-0.22));
    path.curve_to(p(0.98,0.3),p(0.18,0.62),p(0.,0.95));
    path.close_path();path
}
fn gem(cx:f64,cy:f64,r:f64)->BezPath{
    let s=r*0.95;let p=|x:f64,y:f64|(cx+x*s,cy+y*s);let mut path=BezPath::new();
    path.move_to(p(1.,0.));
    path.curve_to(p(0.15,-0.78),p(-0.95,-0.62),p(-0.95,0.05));
    path.curve_to(p(-0.95,0.72),p(0.15,0.78),p(1.,0.));
    path.close_path();path
}
fn clam(cx:f64,cy:f64,r:f64)->BezPath{
    let mut path=BezPath::new();let bumps=5;let steps=bumps*2;
    let arc=|i:usize|{let a=std::f64::consts::PI+std::f64::consts::PI*i as f64/steps as f64;let radius=if i%2==0{r}else{r*0.87};(cx+a.cos()*radius,cy+a.sin()*radius)};
    path.move_to((cx,cy+0.85*r));
    path.line_to(arc(0));
    for i in 1..=steps{path.line_to(arc(i));}
    path.curve_to((cx+0.5*r,cy+0.35*r),(cx+0.3*r,cy+0.7*r),(cx,cy+0.85*r));
    path.close_path();path
}
fn thumb_path(style:ThumbStyle,cx:f64,cy:f64,size:f64,spin:f64)->BezPath{
    let r=size/2.;
    let mut path=match style{
        ThumbStyle::None=>BezPath::new(),
        ThumbStyle::Circle=>Circle::new((cx,cy),r*0.82).to_path(0.1),
        ThumbStyle::Default=>Circle::new((cx,cy),r).to_path(0.1),
        ThumbStyle::Square=>Rect::from_center_size(Point::new(cx,cy),Size::new(size*0.8,size*0.8)).to_rounded_rect(size*0.2).to_path(0.1),
        ThumbStyle::Pill=>Rect::from_center_size(Point::new(cx,cy),Size::new(size*0.24,size)).to_rounded_rect(size*0.12).to_path(0.1),
        ThumbStyle::Diamond=>rounded_polygon(cx,cy,r,4,r*0.18),
        ThumbStyle::Flower=>circled(cx,cy,r,8,0.5,0.4),
        ThumbStyle::Heart=>heart(cx,cy,r),
        ThumbStyle::Cookie=>lobed(cx,cy,r,r*0.82,8,0.),
        ThumbStyle::Puffy=>circled(cx,cy,r,4,0.33,0.55),
        ThumbStyle::Clover=>circled(cx,cy,r,4,0.42,0.52),
        ThumbStyle::Clover8=>circled(cx,cy,r,8,0.52,0.36),
        ThumbStyle::Gem=>gem(cx,cy,r),
        ThumbStyle::Triangle=>rounded_polygon(cx,cy,r,3,r*0.26),
        ThumbStyle::Pentagon=>rounded_polygon(cx,cy,r,5,r*0.24),
        ThumbStyle::Cookie12=>lobed(cx,cy,r,r*0.86,12,0.),
        ThumbStyle::Clam=>clam(cx,cy,r),
    };
    if spin.abs()>1e-4{path.apply_affine(Affine::rotate_about(spin,Point::new(cx,cy)));}
    path
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn unknown_settings_fall_back_to_the_shipped_wavy_pill(){
        assert_eq!(BarStyle::from_setting("Wavy"),BarStyle::Wavy);
        assert_eq!(BarStyle::from_setting("nope"),BarStyle::Wavy);
        assert_eq!(ThumbStyle::from_setting("Pill"),ThumbStyle::Pill);
        assert_eq!(ThumbStyle::from_setting("nope"),ThumbStyle::Pill);
        for style in BarStyle::ALL{assert_eq!(BarStyle::from_setting(style.key()),style);}
        for style in ThumbStyle::ALL{assert_eq!(ThumbStyle::from_setting(style.key()),style);}
    }
    #[test]fn every_thumb_stays_inside_its_box_and_is_not_empty(){
        for style in ThumbStyle::ALL{
            if style==ThumbStyle::None{assert!(thumb_path(style,10.,10.,20.,0.).elements().is_empty());continue}
            let path=thumb_path(style,10.,10.,20.,0.);let bounds=path.bounding_box();
            assert!(bounds.width()>4.&&bounds.height()>4.,"{:?}",style.key());
            let e=0.01;assert!(bounds.x0>=-e&&bounds.x1<=20.+e&&bounds.y0>=-e&&bounds.y1<=20.+e,"{} escapes its box: {:?}",style.key(),bounds);
        }
    }
    #[test]fn drawing_every_style_produces_geometry(){
        for bar in BarStyle::ALL{for thumb in [ThumbStyle::None,ThumbStyle::Clam,ThumbStyle::Pill]{
            let mut scene=Scene::new();
            draw(&mut scene,Rect::new(0.,0.,200.,24.),0.42,0.7,1.,22.,1.,0.,Look{fg:0xff2563eb,track:0xffd4d8e3,dot:0xff6b7280},bar,thumb);
            assert!(!scene.is_empty(),"{}",bar.key());
        }}
    }
}
