use super::*;
#[test]
fn layout_abi_rejects_bad_buffers_and_reports_capacity(){unsafe{
 let mut out=[0u8;65536];let raw=b"null";
 let n=dotisle_desktop_layout(raw.as_ptr(),raw.len(),out.as_mut_ptr(),out.len());assert!(n>0);
 let value:serde_json::Value=serde_json::from_slice(&out[..n as usize]).unwrap();assert!(value["elements"].as_array().unwrap().len()>8);
 let mut tiny=[0u8;2];assert_eq!(dotisle_desktop_layout(raw.as_ptr(),raw.len(),tiny.as_mut_ptr(),tiny.len()),-n);
 assert_eq!(tiny,[0,0]);
 let n=dotisle_desktop_layout(std::ptr::null(),4,out.as_mut_ptr(),out.len());assert!(n>0);assert!(serde_json::from_slice::<serde_json::Value>(&out[..n as usize]).unwrap()["error"].is_string());
 assert_eq!(dotisle_desktop_snapshot(raw.as_ptr(),raw.len()),1);
}}
#[test]
fn snapshot_is_owned_and_coalesces(){unsafe{
 let mut raw=br#"{"title":"first"}"#.to_vec();assert_eq!(dotisle_desktop_snapshot(raw.as_ptr(),raw.len()),0);raw.fill(0);
 assert_eq!(SNAPSHOT.lock().unwrap().as_ref().unwrap()["title"],"first");
 let raw=br#"{"title":"latest"}"#;assert_eq!(dotisle_desktop_snapshot(raw.as_ptr(),raw.len()),0);
 assert_eq!(SNAPSHOT.lock().unwrap().take().unwrap()["title"],"latest");
}}
