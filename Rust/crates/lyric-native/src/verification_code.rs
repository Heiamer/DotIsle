//! Conservative local OTP extraction. Codes are never logged or persisted.
use crate::layout::Rect;
pub const WIDTH:f64=384.;
pub const HEIGHT:f64=84.;
#[derive(Clone,Copy,Debug)]
pub struct Layout{pub body:Rect,pub icon:Rect,pub source:Rect,pub digits:Rect,pub action:Rect,pub compact_action:bool}
/// Content keeps its final geometry while the live shell reveals and clips it.
pub fn layout(shell:Rect,host_width:f64)->Layout{
    let width=WIDTH.min((host_width-48.).max(140.));
    let body=Rect::new((host_width-width)/2.,shell.y,width,HEIGHT);
    let compact_action=width<336.;let action_width=if compact_action{28.}else{84.};
    let x=body.x+60.;let action=Rect::new(body.x+width-20.-action_width,body.y+38.,action_width,28.);
    Layout{body,icon:Rect::new(body.x+18.,body.y+27.,30.,30.),
        source:Rect::new(x,body.y+13.,width-80.,18.),
        digits:Rect::new(x,body.y+34.,(action.x-x-14.).max(1.),36.),action,compact_action}
}
pub fn content_alpha(shell_height:f64,mix:f64)->f32{
    (mix.clamp(0.,1.)*((shell_height-(HEIGHT-8.))/8.).clamp(0.,1.)) as f32
}
pub fn extract(title:&str,body:&str)->Option<String>{
    let text=format!("{title}\n{body}").chars().map(|c|{
        if ('０'..='９').contains(&c){char::from_u32(c as u32-'０' as u32+'0' as u32).unwrap()}else{c}
    }).collect::<Vec<_>>();
    let matching=text.iter().map(char::to_ascii_lowercase).collect::<Vec<_>>();
    let mut markers=Vec::new();
    for word in ["验证码","验证代码","登录代码","安全代码","登录码","安全码","校验码","动态码","动态密码","短信码","确认码","一次性密码","verification code","security code","one-time code","one time code","sign-in code","login code","otp"]{
        let key=word.chars().collect::<Vec<_>>();
        for i in 0..text.len().saturating_sub(key.len()-1){
            if matching.get(i..i+key.len())==Some(key.as_slice()){
                if word=="otp"&&((i>0&&text[i-1].is_ascii_alphanumeric())||text.get(i+key.len()).is_some_and(char::is_ascii_alphanumeric)){continue}
                markers.push((i,i+key.len()));
            }
        }
    }
    if markers.is_empty(){return None}
    let mut candidates=Vec::new();let mut i=0;
    while i<text.len(){
        if !text[i].is_ascii_alphanumeric(){i+=1;continue}
        let start=i;while i<text.len()&&text[i].is_ascii_alphanumeric(){i+=1}
        if !valid_code(&text[start..i].iter().collect::<String>()) || start>0&&text[start-1].is_ascii_alphanumeric()
            || text.get(i).is_some_and(char::is_ascii_alphanumeric){continue}
        if text[i..].iter().find(|c|!c.is_whitespace()).is_some_and(|c|"年月日分秒元".contains(*c)){continue}
        let context=matching[start.saturating_sub(8)..start].iter().collect::<String>();
        if ["订单","快递","单号","尾号","电话","手机号","金额","余额","order","tracking"].iter().any(|w|context.contains(w)){continue}
        let distance=markers.iter().map(|(a,b)|if start>=*b{start-b}else if i<=*a{a-i}else{0}).min()?;
        if distance<=24{candidates.push((distance,text[start..i].iter().collect::<String>()));}
    }
    candidates.sort_by_key(|(d,_)|*d);
    if candidates.get(1).is_some_and(|c|c.0==candidates[0].0&&c.1!=candidates[0].1){return None}
    candidates.first().map(|(_,c)|c.clone())
}

fn valid_code(code:&str)->bool {
    (4..=8).contains(&code.len()) && code.bytes().all(|c|c.is_ascii_alphanumeric())
        && code.bytes().any(|c|c.is_ascii_digit())
}

pub fn copy(code:&str,hwnd:windows::Win32::Foundation::HWND)->Result<(),String>{
    use std::ffi::c_void;
    #[link(name="user32")]unsafe extern "system"{
        fn OpenClipboard(hwnd:*mut c_void)->i32;fn CloseClipboard()->i32;
        fn EmptyClipboard()->i32;fn SetClipboardData(format:u32,memory:*mut c_void)->*mut c_void;
    }
    #[link(name="kernel32")]unsafe extern "system"{
        fn GlobalAlloc(flags:u32,size:usize)->*mut c_void;fn GlobalLock(memory:*mut c_void)->*mut c_void;
        fn GlobalUnlock(memory:*mut c_void)->i32;fn GlobalFree(memory:*mut c_void)->*mut c_void;
    }
    if !valid_code(code){return Err("验证码无效".into())}
    let data=code.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe{
        let memory=GlobalAlloc(0x42,data.len()*2);
        if memory.is_null(){return Err("复制失败，请重试".into())}
        let ptr=GlobalLock(memory);
        if ptr.is_null(){GlobalFree(memory);return Err("复制失败，请重试".into())}
        std::ptr::copy_nonoverlapping(data.as_ptr(),ptr.cast::<u16>(),data.len());GlobalUnlock(memory);
        if OpenClipboard(hwnd.0)==0{GlobalFree(memory);return Err("剪贴板正忙，请再点一次".into())}
        let ok=EmptyClipboard()!=0&&!SetClipboardData(13,memory).is_null();CloseClipboard();
        if !ok{GlobalFree(memory);return Err("复制失败，请重试".into())}
    }
    Ok(())
}
#[cfg(test)] mod tests{
    use super::*;
    #[test]fn recognizes_nearby_codes_and_full_width_digits(){
        assert_eq!(extract("登录提醒","验证码：１２３４５６，5 分钟内有效"),Some("123456".into()));
        assert_eq!(extract("Your OTP","Your verification code is 472913"),Some("472913".into()));
        assert_eq!(extract("8421 是您的动态码","请勿告诉他人"),Some("8421".into()));
        assert_eq!(extract("","您的动态密码：739251，请勿泄露"),Some("739251".into()));
        assert_eq!(extract("","确认码 4821"),Some("4821".into()));
    }
    #[test]fn mail_codes_preserve_case_and_copy_validation_accepts_the_same_payload(){
        for code in ["a7B2c9","0042","Z9x8Y7w6"] {
            let body=format!("QQ邮箱提醒：服务登录，您的验证码是 {code}，5 分钟内有效。");
            assert_eq!(extract("新邮件提醒",&body).as_deref(),Some(code));
            assert!(valid_code(code));
        }
        assert_eq!(extract("Sign-in code","Your login code is a7B2c9"),Some("a7B2c9".into()));
        for invalid in ["security","minutes","AB123456CD","123456789","a7 B2c9","https://"] {assert!(!valid_code(invalid));}
    }
    #[test]fn rejects_order_phone_expiry_and_unrelated_numbers(){
        for body in ["订单 123456 已发货","验证码有效期为 2026 年，订单号 12345678","验证码为 12345678901","电话 13800138000","photoOTP 123456"]{
            assert_eq!(extract("",body),None,"{body}");
        }
        assert_eq!(extract("验证码","AB123456CD"),None);
    }
}

#[cfg(test)]mod layout_tests{
    use super::*;
    #[test]fn source_digits_and_copy_action_have_separate_safe_lanes(){
        for host in [352.,400.,448.,488.,640.]{let g=layout(Rect::new(host/2.-130.,24.,260.,40.),host);
            for r in [g.icon,g.source,g.digits,g.action]{assert!(r.w>0.&&r.h>0.);assert!(r.x>=g.body.x+16.&&r.x+r.w<=g.body.x+g.body.w-16.);assert!(r.y>=g.body.y+12.&&r.y+r.h<=g.body.y+g.body.h-12.);}
            assert!(g.icon.x+g.icon.w+12.<=g.source.x);
            assert!(g.source.y+g.source.h+3.<=g.digits.y);
            assert!(g.digits.x+g.digits.w+14.<=g.action.x);
        }
    }
    #[test]fn mini_and_classic_reveal_fixed_content_without_squeezing_text(){
        for base in [Rect::new(114.,24.,260.,40.),Rect::new(194.,24.,100.,18.)]{
            let mut hub=crate::capsule_alert::Hub::default();hub.enqueue(crate::capsule_alert::Alert::verification("test","测试应用","12345678".into()));
            assert!(hub.begin(base,base.h/2.,488.,24.,0.,true));
            let expected=layout(Rect::new(52.,24.,WIDTH,HEIGHT),488.);
            for i in 0..120{let now=i as f64/120.;let(shell,_,mix)=hub.banner.presentation(now);let g=layout(shell,488.);
                assert!((g.source.x-expected.source.x).abs()<0.001);assert_eq!(g.source.y,expected.source.y);assert_eq!(g.digits,expected.digits);assert_eq!(g.action,expected.action);
                assert!((0.0..=1.0).contains(&content_alpha(shell.h,mix)));
            }
            hub.dismiss(base,base.h/2.,0.5,true);hub.tick(base,base.h/2.,2.,true);assert!(!hub.visible());
            let(shell,_,mix)=hub.banner.presentation(2.);assert!(content_alpha(shell.h,mix)<0.001);
        }
        assert_eq!(content_alpha(18.,1.),0.);assert_eq!(content_alpha(HEIGHT,1.),1.);
    }
    #[test]fn supported_code_lengths_keep_leading_zeroes_and_use_one_payload(){
        for code in ["0042","002468","00135790"]{let a=crate::capsule_alert::Alert::verification("test","验证测试",code.into());
            assert_eq!(a.code.as_deref(),Some(code));assert!(a.trailing.is_empty());assert_eq!((a.width,a.height),(WIDTH,HEIGHT));
        }
    }
}
