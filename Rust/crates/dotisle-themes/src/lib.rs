mod music_controls;
use serde_json::{json,Value};mod dynamic_color;mod monet;mod cover_color;mod theme_accent;
fn parse<T:serde::de::DeserializeOwned>(a:&Value,key:&str)->Result<T,String>{serde_json::from_value(a[key].clone()).map_err(|e|e.to_string())}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];match v["method"].as_str().ok_or("method")?{
"roles"=>Ok(json!(dynamic_color::roles(parse(a,"seed")?,parse(a,"dark")?,parse(a,"content")?,parse(a,"high_contrast")?))),
"resolve"=>Ok(json!(dynamic_color::resolve(parse(a,"requested")?,parse(a,"content")?,parse(a,"system")?,parse(a,"custom")?,parse(a,"dark")?,parse(a,"high")?))),
"resolve_center"=>Ok(json!(dynamic_color::resolve_center(parse(a,"requested")?,parse(a,"content")?,parse(a,"system")?,parse(a,"custom")?,parse(a,"dark")?,parse(a,"high")?))),
"scheme"=>Ok(json!(dynamic_color::scheme(parse(a,"seed")?,parse(a,"dark")?))),
"roles.mix"=>{let r:dynamic_color::Roles=parse(a,"self")?;Ok(json!(r.mix(parse(a,"other")?,parse(a,"t")?)))},"roles.ui"=>{let r:dynamic_color::Roles=parse(a,"self")?;Ok(json!(r.ui()))},
"SeedMotion.update"=>{let mut state:dynamic_color::SeedMotion=parse(a,"state")?;let result=state.update(parse(a,"target")?,parse(a,"now")?,parse(a,"animate")?);Ok(json!({"state":state,"result":result}))},
"SeedMotion.sample"=>{let state:dynamic_color::SeedMotion=parse(a,"state")?;Ok(json!(state.sample(parse(a,"now")?)))},
"SeedMotion.moving"=>{let state:dynamic_color::SeedMotion=parse(a,"state")?;Ok(json!(state.moving(parse(a,"now")?)))},
"RoleMotion.update"=>{let mut state:dynamic_color::RoleMotion=parse(a,"state")?;let result=state.update(parse(a,"target")?,parse(a,"now")?,parse(a,"animate")?);Ok(json!({"state":state,"result":result}))},
"RoleMotion.sample"=>{let state:dynamic_color::RoleMotion=parse(a,"state")?;Ok(json!(state.sample(parse(a,"now")?)))},
"RoleMotion.moving"=>{let state:dynamic_color::RoleMotion=parse(a,"state")?;Ok(json!(state.moving(parse(a,"now")?)))},
"seed"=>Ok(json!(monet::seed(&parse::<Vec<u8>>(a,"rgba")?))),"contrast"=>Ok(json!(monet::contrast(parse(a,"a")?,parse(a,"b")?))),
"cover.palette"=>Ok(json!(cover_color::palette(&parse::<Vec<u8>>(a,"bgra")?))),
"cover.mix"=>Ok(json!(cover_color::mix(parse(a,"a")?,parse(a,"b")?,parse(a,"t")?))),
"cover.luminance"=>Ok(json!(cover_color::luminance(parse(a,"c")?))),
"cover.contrast"=>Ok(json!(cover_color::contrast(parse(a,"a")?,parse(a,"b")?))),
"cover.default"=>Ok(json!(cover_color::CoverPalette::default())),
"accent.resolve"=>Ok(json!(theme_accent::resolve(a))),
"accent.system_dark"=>Ok(json!(theme_accent::system_dark())),
"accent.system_accent"=>Ok(json!(theme_accent::system_accent())),
 "controls.on_surface"=>Ok(json!(music_controls::on_surface(parse(a,"surface")?,parse(a,"preferred")?))),
"controls.tinted"=>Ok(json!(music_controls::tinted_on_surfaces(&parse::<Vec<u32>>(a,"surfaces")?,parse(a,"preferred")?))),
"controls.skip"=>Ok(json!(music_controls::skip_colors(parse(a,"surface")?,parse(a,"preferred")?))),
 _=>Err("unknown theme method".into())}}
dotisle_module_api::export_service!("themes",dispatch);
