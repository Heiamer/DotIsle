//! Warm ceramic surfaces and electric-blue controls inspired by the reference dashboard.
use dotisle_domain::monet::Scheme;
pub fn active(m:&crate::model::Model)->bool{m.prefs.control_center_theme=="Porcelain"}
pub fn scheme(dark:bool)->Scheme{
 if dark{Scheme{
  primary:0x68AAFF,on_primary:0x102139,primary_container:0x242A35,on_primary_container:0xF3F4F8,
  secondary_container:0x68AAFF,on_secondary_container:0x102139,tertiary_container:0x25262E,on_tertiary_container:0xF3F4F8,
  surface:0x191A20,container:0x25262E,high:0x31323B,on_surface:0xF3F4F8,on_surface_variant:0xBCC0CD,outline:0x4B4E5B,outline_variant:0x393B46,error:0xFF777F,
 }}else{Scheme{
  primary:0x007AFF,on_primary:0xFFFFFF,primary_container:0xFCF9FB,on_primary_container:0x1D2027,
  secondary_container:0x007AFF,on_secondary_container:0xFFFFFF,tertiary_container:0xFCF9FB,on_tertiary_container:0x1D2027,
  surface:0xEFEDF0,container:0xFCF9FB,high:0xEEECF0,on_surface:0x1D2027,on_surface_variant:0x494F60,outline:0xD4D2D9,outline_variant:0xE3E1E6,error:0xBB3442,
 }}
}
