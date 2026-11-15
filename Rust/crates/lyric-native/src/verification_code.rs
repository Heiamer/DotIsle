//! Conservative local OTP extraction. Codes are never logged or persisted.
pub fn extract(title:&str,body:&str)->Option<String>{
    let text=format!("{title}\n{body}").to_lowercase().chars().map(|c|{
        if ('０'..='９').contains(&c){char::from_u32(c as u32-'０' as u32+'0' as u32).unwrap()}else{c}
    }).collect::<Vec<_>>();
    let mut markers=Vec::new();
    for word in ["验证码","校验码","动态码","短信码","一次性密码","verification code","security code","one-time code","one time code","otp"]{
        let key=word.chars().collect::<Vec<_>>();
        for i in 0..text.len().saturating_sub(key.len()-1){
            if text.get(i..i+key.len())==Some(key.as_slice()){
                if word=="otp"&&((i>0&&text[i-1].is_ascii_alphanumeric())||text.get(i+key.len()).is_some_and(char::is_ascii_alphanumeric)){continue}
                markers.push((i,i+key.len()));
            }
        }
    }
    if markers.is_empty(){return None}
    let mut candidates=Vec::new();let mut i=0;
    while i<text.len(){
        if !text[i].is_ascii_digit(){i+=1;continue}
        let start=i;while i<text.len()&&text[i].is_ascii_digit(){i+=1}
        if !(4..=8).contains(&(i-start)) || start>0&&text[start-1].is_ascii_alphanumeric()
            || text.get(i).is_some_and(char::is_ascii_alphanumeric){continue}
        if text[i..].iter().find(|c|!c.is_whitespace()).is_some_and(|c|"年月日分秒元".contains(*c)){continue}
        let context=text[start.saturating_sub(8)..start].iter().collect::<String>();
        if ["订单","快递","单号","尾号","电话","手机号","金额","余额","order","tracking"].iter().any(|w|context.contains(w)){continue}
        let distance=markers.iter().map(|(a,b)|if start>=*b{start-b}else if i<=*a{a-i}else{0}).min()?;
        if distance<=24{candidates.push((distance,text[start..i].iter().collect::<String>()));}
    }
    candidates.sort_by_key(|(d,_)|*d);
    if candidates.get(1).is_some_and(|c|c.0==candidates[0].0&&c.1!=candidates[0].1){return None}
    candidates.first().map(|(_,c)|c.clone())
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
    if !(4..=8).contains(&code.len())||!code.bytes().all(|c|c.is_ascii_digit()){return Err("验证码无效".into())}
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
    }
    #[test]fn rejects_order_phone_expiry_and_unrelated_numbers(){
        for body in ["订单 123456 已发货","验证码有效期为 2026 年，订单号 12345678","验证码为 12345678901","电话 13800138000","photoOTP 123456"]{
            assert_eq!(extract("",body),None,"{body}");
        }
        assert_eq!(extract("验证码","AB123456CD"),None);
    }
}
