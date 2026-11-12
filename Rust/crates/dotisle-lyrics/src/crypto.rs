//! Format interoperability. QRC block codec is isolated in vendor/qq-lyric-codec.
use std::io::Read;
use base64::{Engine,engine::general_purpose::STANDARD};
const MAX:usize=8*1024*1024;
fn inflate(bytes:&[u8])->Result<String,String>{let mut out=Vec::new();flate2::read::ZlibDecoder::new(bytes).take(MAX as u64+1).read_to_end(&mut out).map_err(|e|e.to_string())?;if out.len()>MAX{return Err("decompressed lyric exceeds limit".into())}String::from_utf8(out).map(|s|s.trim_start_matches('\u{feff}').trim_end_matches('\0').to_owned()).map_err(|e|e.to_string())}
pub fn krc_text(input:&[u8])->Result<String,String>{
 if input.len()>MAX*2{return Err("KRC exceeds limit".into())}
 let decoded;if input.starts_with(b"krc1"){decoded=input.to_vec()}else if let Ok(s)=std::str::from_utf8(input){if s.trim_start().starts_with('['){return Ok(s.into())}decoded=STANDARD.decode(s.trim()).map_err(|e|e.to_string())?;}else{return Err("invalid KRC encoding".into())}
 let body=decoded.strip_prefix(b"krc1").ok_or("invalid KRC header")?;let key=[64,71,97,119,94,50,116,71,81,54,49,45,206,210,110,105];let compressed=body.iter().enumerate().map(|(i,b)|b^key[i%16]).collect::<Vec<_>>();inflate(&compressed)
}
type Schedule=[[[u8;6];16];3];
extern "C"{fn three_des_key_setup(key:*const u8,schedule:*mut Schedule,mode:i32);fn three_des_crypt(input:*const u8,output:*mut u8,schedule:*const Schedule);}
fn qrc_blocks(input:&[u8],decrypt:bool)->Result<Vec<u8>,String>{if input.is_empty()||input.len()>MAX||input.len()%8!=0{return Err("invalid QRC block length".into())}let mut schedule=[[[0u8;6];16];3];let mut out=vec![0;input.len()];unsafe{three_des_key_setup(b"!@#)(*$%123ZXC!@!@#)(NHL".as_ptr(),&mut schedule,i32::from(decrypt));for(src,dst)in input.chunks_exact(8).zip(out.chunks_exact_mut(8)){three_des_crypt(src.as_ptr(),dst.as_mut_ptr(),&schedule);}}Ok(out)}
pub fn qrc_text(hex:&str)->Result<String,String>{if hex.len()>MAX*2||hex.len()%16!=0{return Err("invalid QRC hex length".into())}let input=hex.as_bytes().chunks_exact(2).map(|b|{let h=(b[0]as char).to_digit(16).ok_or("invalid QRC hex")?;let l=(b[1]as char).to_digit(16).ok_or("invalid QRC hex")?;Ok(((h<<4)|l)as u8)}).collect::<Result<Vec<_>,&str>>()?;inflate(&qrc_blocks(&input,true)?) }
#[cfg(test)]mod tests{use super::*;use std::io::Write;
 fn compressed(s:&str)->Vec<u8>{let mut w=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());w.write_all(s.as_bytes()).unwrap();w.finish().unwrap()}
 #[test]fn independent_unicode_payload_survives_both_codecs(){let s="[3250,1420]点(3250,500)屿(3750,920)";let mut b=compressed(s);b.resize(b.len().div_ceil(8)*8,0);let encrypted=qrc_blocks(&b,false).unwrap().iter().map(|b|format!("{b:02x}")).collect::<String>();assert_eq!(qrc_text(&encrypted).unwrap(),s);let key=[64,71,97,119,94,50,116,71,81,54,49,45,206,210,110,105];let bytes=b"krc1".iter().copied().chain(compressed(s).into_iter().enumerate().map(|(i,b)|b^key[i%16])).collect::<Vec<_>>();assert_eq!(krc_text(&bytes).unwrap(),s);assert_eq!(krc_text(STANDARD.encode(bytes).as_bytes()).unwrap(),s);}
 #[test]fn bad_ciphertext_returns_errors(){for s in ["", "fg", "0000000000000000", "汉字"]{assert!(qrc_text(s).is_err())}assert!(krc_text(b"krc1bad").is_err());}
}
