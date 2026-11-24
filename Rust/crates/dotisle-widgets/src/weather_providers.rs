//! Public sources need no keys. Interpolated model data is never called minute nowcasting.
use super::pool::Source;
use serde_json::{json,Value};
use std::{collections::BTreeMap,sync::{Mutex,OnceLock},time::{Duration,Instant}};

#[derive(Clone)]
struct Place { name:String, region:Value, lat:f64, lon:f64, offset:Option<i64> }
fn get(url:&str)->Result<Value,String>{crate::weather_http::get_json(url)}
pub(super) fn now()->i64{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64}
fn fixed_offset(zone:&str)->Option<i64>{match zone {
    "Asia/Shanghai"|"Asia/Hong_Kong"|"Asia/Macau"|"Asia/Taipei"|"Asia/Singapore"=>Some(8*3600),
    "Asia/Tokyo"|"Asia/Seoul"=>Some(9*3600),"Asia/Kolkata"=>Some(19800),"UTC"|"Etc/UTC"=>Some(0),_=>None,
}}
fn wttr(city:&str)->Result<Value,String>{
    get(&format!("https://wttr.in/{}?format=j1",dotisle_module_storage::http::encode(city)))
}
// Keep successful coordinates longer than their refresh TTL: a transient geocoder
// outage should not prevent an otherwise healthy forecast endpoint from serving the city.
fn place(city:&str, fallback:impl FnOnce()->Result<Value,String>)->Result<Place,String>{
    static PLACES:OnceLock<Mutex<BTreeMap<String,(Instant,Place)>>>=OnceLock::new();
    let cache=PLACES.get_or_init(Default::default);
    let previous=cache.lock().ok().and_then(|c|c.get(city).cloned());
    if let Some((at,p))=&previous {if at.elapsed()<Duration::from_secs(86400){return Ok(p.clone())}}
    let resolved=validate_place((||{
        let raw=get(&format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh&format=json",dotisle_module_storage::http::encode(city)))?;
        let p=raw["results"].as_array().and_then(|a|a.first()).ok_or("未找到城市")?;
        Ok(Place{name:p["name"].as_str().unwrap_or(city).into(),region:p["admin1"].clone(),
            lat:super::number(&p["latitude"]).ok_or("城市坐标缺失")?,lon:super::number(&p["longitude"]).ok_or("城市坐标缺失")?,
            offset:fixed_offset(p["timezone"].as_str().unwrap_or(""))})
    })()).or_else(|_|validate_place((||{
        let raw=fallback()?;let p=&raw["nearest_area"][0];
        Ok(Place{name:p["areaName"][0]["value"].as_str().unwrap_or(city).into(),region:p["region"][0]["value"].clone(),
            lat:super::wttr_number(&p["latitude"]).ok_or("城市坐标缺失")?,lon:super::wttr_number(&p["longitude"]).ok_or("城市坐标缺失")?,
            offset:(p["country"][0]["value"].as_str()==Some("China")).then_some(8*3600)})
    })()));
    let resolved=match resolved {
        Ok(p)=>p,
        Err(error)=>return previous.filter(|(at,_)|at.elapsed()<Duration::from_secs(7*86400)).map(|(_,p)|p).ok_or(error),
    };
    if let Ok(mut cache)=cache.lock(){if cache.len()>=32{if let Some(key)=cache.iter().min_by_key(|(_,v)|v.0).map(|(k,_)|k.clone()){cache.remove(&key);}}cache.insert(city.into(),(Instant::now(),resolved.clone()));}
    Ok(resolved)
}
fn validate_place(result:Result<Place,String>)->Result<Place,String>{
    let place=result?;
    if !(-90. ..=90.).contains(&place.lat)||!(-180. ..=180.).contains(&place.lon){return Err("城市坐标无效".into())}
    Ok(place)
}
#[derive(Default)]
struct PerRefresh<T>(Option<Result<T,String>>);
impl<T> PerRefresh<T>{
    fn get(&mut self,load:impl FnOnce()->Result<T,String>)->Result<&T,String>{
        self.0.get_or_insert_with(load).as_ref().map_err(Clone::clone)
    }
}
pub(super) struct Requests<'a> { city:&'a str, place:PerRefresh<Place>, wttr:PerRefresh<Value> }
impl<'a> Requests<'a>{
    pub fn new(city:&'a str)->Self{Self{city,place:PerRefresh(None),wttr:PerRefresh(None)}}
    fn coordinates(&mut self)->Result<&Place,String>{
        let city=self.city;let weather=&mut self.wttr;
        self.place.get(||place(city,||weather.get(||wttr(city)).cloned()))
    }
    pub fn request(&mut self,source:Source)->Result<Value,String>{
    let city=self.city;
    let mut data=match source {
        Source::Qq=>fetch_qq(city)?,
        Source::Toutiao=>{
            let raw=get(&format!("https://www.toutiao.com/stream/widget/local_weather/data/?city={}",dotisle_module_storage::http::encode(city)))?;
            parse_toutiao(city,&raw,now())?
        }
        Source::OpenMeteo=>{
            let p=self.coordinates()?;
            let raw=get(&format!("https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,weather_code,is_day&hourly=precipitation,rain,showers,snowfall,precipitation_probability&daily=temperature_2m_max,temperature_2m_min&forecast_days=2&timezone=auto&timeformat=unixtime",p.lat,p.lon))?;
            super::parse(&json!({"name":p.name,"admin1":p.region}),&raw)?
        }
        Source::MetNorway=>{
            let p=self.coordinates()?;
            // Four decimals respect MET's cache requirement; the app identifies its project.
            let raw=get(&format!("https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={:.4}&lon={:.4}",p.lat,p.lon))?;
            parse_met(p,&raw,now())?
        }
        Source::BrightSky=>{
            let p=self.coordinates()?;let clock=now();
            let raw=get(&format!("https://api.brightsky.dev/weather?lat={:.4}&lon={:.4}&date={}&last_date={}&max_dist=50000&tz=UTC",p.lat,p.lon,utc_text(clock-3600),utc_text(clock+30*3600)))?;
            parse_brightsky(p,&raw,clock)?
        }
        Source::Wttr=>super::parse_wttr(city,self.wttr.get(||wttr(city))?)?,
        Source::SevenTimer=>{
            let p=self.coordinates()?;let raw=get(&format!("https://www.7timer.info/bin/api.pl?lon={:.4}&lat={:.4}&product=civil&output=json",p.lon,p.lat))?;
            parse_seven(p,&raw,now())?
        }
    };
    let at=data["time"].as_i64().ok_or("天气时间缺失")?;let clock=now();
    if at>clock+900||at<clock-5400{return Err("天气数据已过期".into())}
    data["sourceUrl"]=json!(match source {Source::Qq=>"https://tianqi.qq.com/",Source::Toutiao=>"https://www.toutiao.com/",Source::OpenMeteo=>"https://open-meteo.com/",Source::MetNorway=>"https://api.met.no/",Source::BrightSky=>"https://brightsky.dev/",Source::Wttr=>"https://github.com/chubin/wttr.in",Source::SevenTimer=>"https://www.7timer.info/"});
    if matches!(source,Source::OpenMeteo|Source::MetNorway){data["dataLicense"]=json!("CC BY 4.0");}
    Ok(data)
}

}

#[cfg(test)]
#[test]
fn failed_shared_requests_are_tried_once_per_refresh(){
    let mut batch=PerRefresh::<Value>(None);
    assert!(batch.get(||Err("offline".into())).is_err());
    assert!(batch.get(||panic!("fallback retried the failed geocoder")).is_err());
    let mut next=PerRefresh(None);
    assert_eq!(next.get(||Ok(json!("online"))).unwrap(),"online");
}

// Civil calendar conversion avoids a timezone library for UTC request bounds.
fn utc_text(epoch:i64)->String{
    let z=epoch.div_euclid(86400)+719468;let era=z.div_euclid(146097);let doe=z-era*146097;
    let yoe=(doe-doe/1460+doe/36524-doe/146096)/365;let mut y=yoe+era*400;
    let doy=doe-(365*yoe+yoe/4-yoe/100);let mp=(5*doy+2)/153;let d=doy-(153*mp+2)/5+1;
    let m=mp+if mp<10{3}else{-9};y+=i64::from(m<=2);let seconds=epoch.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:00Z",seconds/3600)
}
fn iso_stamp(raw:&str)->Option<i64>{
    let offset=if raw.ends_with('Z'){0}else{
        let suffix=raw.get(raw.len().checked_sub(6)?..)?;let sign=match suffix.as_bytes()[0]{b'+'=>1,b'-'=>-1,_=>return None};
        if suffix.as_bytes()[3]!=b':'{return None}let h=suffix[1..3].parse::<i64>().ok()?;let m=suffix[4..6].parse::<i64>().ok()?;
        if h>14||m>59{return None}sign*(h*3600+m*60)
    };stamp(raw,offset)
}
fn parse_brightsky(p:&Place,raw:&Value,clock:i64)->Result<Value,String>{
    let sources=raw["sources"].as_array().ok_or("DWD 站点缺失")?;
    let valid=|v:&Value|sources.iter().any(|s|s["id"]==v["source_id"]&&super::number(&s["distance"]).is_some_and(|d|(0. ..=50000.).contains(&d)));
    let series=raw["weather"].as_array().ok_or("DWD 小时预报缺失")?;
    let mut slots=Vec::new();
    for v in series.iter().filter(|v|valid(v)){
        let Some(at)=v["timestamp"].as_str().and_then(iso_stamp)else{continue};
        let symbol=match v["condition"].as_str(){Some("snow")=>"snow",Some("sleet"|"hail")=>"sleet",_=>"rain"};
        slots.push(Slot{start:at-3600,minutes:60,kind:precipitation(super::number(&v["precipitation"]),symbol),
            probability:super::number(&v["precipitation_probability"]).filter(|v|(0. ..=100.).contains(v))});
    }
    slots.sort_by_key(|v|v.start);let (rain,through)=windows(&slots,clock,p.offset.unwrap_or(0),p.offset.is_none());
    let (at,current)=series.iter().filter(|v|valid(v)).filter_map(|v|{
        let at=iso_stamp(v["timestamp"].as_str()?)?;
        (at>=clock-3600&&at<=clock+3600&&super::number(&v["temperature"]).is_some_and(|t|(-100. ..=100.).contains(&t))).then_some((at,v))
    }).min_by_key(|(at,_)|(*at-clock).abs()).ok_or("DWD 当前温度样本缺失")?;
    let code=match current["icon"].as_str().unwrap_or(""){s if s.contains("thunder")=>95,s if s.contains("sleet")||s=="hail"=>66,
        s if s.contains("snow")=>73,s if s.contains("rain")=>63,"fog"=>45,"cloudy"=>3,s if s.starts_with("partly")=>2,s if s.starts_with("clear")=>0,_=>255};
    Ok(json!({"city":p.name,"region":p.region,"temperature":current["temperature"],"temperatureKind":"forecast","temperatureValidAt":at,
        "time":clock,"timestampKind":"retrieved","code":code,"day":if current["icon"].as_str().unwrap_or("").ends_with("night"){0}else{1},
        "precipitation":rain,"precipitationForecastAvailable":through>=clock+10800,"forecastThrough":through,"forecastResolutionMinutes":60,
        "utcOffsetSeconds":p.offset,"source":"DWD / Bright Sky","stale":false,"dataAttribution":"Deutscher Wetterdienst (DWD), via Bright Sky"}))
}

#[derive(Clone,Debug)]
pub(super) struct Slot { pub start:i64,pub minutes:u32,pub kind:Option<&'static str>,pub probability:Option<f64> }
pub(super) fn windows(slots:&[Slot],clock:i64,offset:i64,utc:bool)->(Value,i64){
    let mut output=Vec::new();let mut i=0;
    let mut through=clock;
    for s in slots {
        let end=s.start+s.minutes as i64*60;
        if end<=clock{continue}if s.kind.is_none()||s.start>through+if through==clock{3600}else{0}{break}
        through=through.max(end);
    }
    let text=|at|format!("{}{}",if utc{"UTC "}else{""},super::label(at,clock,offset));
    while i<slots.len(){
        let s=&slots[i];let Some(kind)=s.kind.filter(|k|*k!="dry")else{i+=1;continue};
        if s.start+s.minutes as i64*60<=clock||s.start>clock+86400{i+=1;continue}
        let mut j=i+1;
        while j<slots.len()&&slots[j].start==slots[j-1].start+slots[j-1].minutes as i64*60&&slots[j].kind==Some(kind)&&slots[j].minutes==s.minutes{j+=1;}
        let end=slots.get(j).filter(|next|next.start==slots[j-1].start+slots[j-1].minutes as i64*60&&next.kind.is_some()).map(|s|s.start);
        output.push(json!({"kind":kind,"startsAt":s.start,"endsAt":end,"startLabel":text(s.start),"endLabel":end.map(text),
            "ongoing":s.start<=clock,"probability":s.probability,"resolutionMinutes":s.minutes,"approximate":true}));i=j;
    }
    (json!(output),through)
}
fn precipitation(amount:Option<f64>,symbol:&str)->Option<&'static str>{
    let amount=amount.filter(|v|*v>=0.&&*v<1000.)?;
    Some(if amount<0.1{"dry"}else if symbol.contains("sleet"){"mixed"}else if symbol.contains("snow"){"snow"}else{"rain"})
}
fn met_code(symbol:&str)->u32{
    if symbol.contains("thunder"){95}else if symbol.contains("sleet"){66}else if symbol.contains("snow"){73}
    else if symbol.contains("rain"){63}else if symbol.contains("fog"){45}else if symbol.starts_with("cloudy"){3}
    else if symbol.starts_with("partlycloudy"){2}else if symbol.starts_with("fair"){1}else if symbol.starts_with("clearsky"){0}else{255}
}
fn parse_met(p:&Place,raw:&Value,clock:i64)->Result<Value,String>{
    let series=raw["properties"]["timeseries"].as_array().ok_or("MET 预报数据缺失")?;
    let current=series.iter().filter_map(|s|stamp(s["time"].as_str()?,0).map(|at|(at,s))).filter(|(at,_)|*at<=clock+3600).min_by_key(|(at,_)|(*at-clock).abs()).ok_or("MET 当前预报缺失")?.1;
    let temperature=super::number(&current["data"]["instant"]["details"]["air_temperature"]).filter(|v|(-100. ..=100.).contains(v)).ok_or("MET 温度缺失")?;
    let slots=series.iter().filter_map(|s|{
        let start=stamp(s["time"].as_str()?,0)?;let hourly=&s["data"]["next_1_hours"];
        Some(Slot{start,minutes:60,kind:precipitation(super::number(&hourly["details"]["precipitation_amount"]),hourly["summary"]["symbol_code"].as_str().unwrap_or("")),probability:None})
    }).collect::<Vec<_>>();
    let (rain,through)=windows(&slots,clock,p.offset.unwrap_or(0),p.offset.is_none());
    let symbol=current["data"]["next_1_hours"]["summary"]["symbol_code"].as_str().unwrap_or("");
    let today=(clock+p.offset.unwrap_or(0)).div_euclid(86400);
    let temperatures=series.iter().filter_map(|s|{
        let at=stamp(s["time"].as_str()?,0)?;
        ((at+p.offset.unwrap_or(0)).div_euclid(86400)==today).then(||super::number(&s["data"]["instant"]["details"]["air_temperature"])).flatten()
    }).collect::<Vec<_>>();
    Ok(json!({"city":p.name,"region":p.region,"temperature":temperature,"time":clock,"timestampKind":"retrieved",
        "forecastUpdatedAt":raw["properties"]["meta"]["updated_at"],"code":met_code(symbol),"day":if symbol.ends_with("_night"){0}else{1},
        "high":temperatures.iter().copied().reduce(f64::max),"low":temperatures.iter().copied().reduce(f64::min),"dailyRangeKind":"available-hourly-samples",
        "precipitation":rain,"precipitationForecastAvailable":through>=clock+3*3600,"forecastThrough":through,
        "forecastResolutionMinutes":60,"utcOffsetSeconds":p.offset,"source":"MET Norway","stale":false}))
}
pub(super) fn stamp(raw:&str,offset:i64)->Option<i64>{
    if !raw.is_ascii()||raw.len()<16{return None}
    let date=raw.get(..10)?.replace('/',"-");let day=crate::widget_config::date_ordinal(&date)?-719162;
    let hour=raw.get(11..13)?.parse::<i64>().ok()?;let minute=raw.get(14..16)?.parse::<i64>().ok()?;
    let second=if raw.len()>=19{raw.get(17..19)?.parse::<i64>().ok()?}else{0};
    if !(0..24).contains(&hour)||!(0..60).contains(&minute)||!(0..60).contains(&second){return None}
    Some(day*86400+hour*3600+minute*60+second-offset)
}
pub(super) fn wttr_forecast(raw:&Value,clock:i64)->Option<(Value,i64)>{
    let country=raw["nearest_area"][0]["country"][0]["value"].as_str()?;
    let offset=match country {"China"|"Hong Kong"|"Taiwan"|"Singapore"=>8*3600,"Japan"|"South Korea"=>9*3600,_=>return None};
    let mut slots=Vec::new();
    for day in raw["weather"].as_array()? {
        let date=day["date"].as_str()?;
        for hour in day["hourly"].as_array()? {
            let time=hour["time"].as_str()?.parse::<u32>().ok()?;if time%100!=0||time/100>23{continue}
            let start=stamp(&format!("{date} {:02}:00",time/100),offset)?;
            let code=hour["weatherCode"].as_str().unwrap_or("");
            let symbol=match code {"179"|"182"|"185"|"281"|"284"|"311"|"314"|"317"|"320"=>"sleet",
                "227"|"230"|"323"|"326"|"329"|"332"|"335"|"338"|"368"|"371"|"395"=>"snow",_=>"rain"};
            slots.push(Slot{start,minutes:180,kind:precipitation(super::wttr_number(&hour["precipMM"]),symbol),
                probability:super::wttr_number(&hour[if symbol=="snow"{"chanceofsnow"}else{"chanceofrain"}]).filter(|v|(0. ..=100.).contains(v))});
        }
    }
    slots.sort_by_key(|s|s.start);Some(windows(&slots,clock,offset,false))
}

fn compact_stamp(raw:&str,offset:i64)->Option<i64>{
    if !raw.is_ascii()||raw.len()<10||!raw.bytes().all(|b|b.is_ascii_digit()){return None}
    stamp(&format!("{}-{}-{} {}:{}:{}",&raw[..4],&raw[4..6],&raw[6..8],&raw[8..10],raw.get(10..12).unwrap_or("00"),raw.get(12..14).unwrap_or("00")),offset)
}
fn cn_kind(code:&str)->Option<&'static str>{
    match code.parse::<u32>().ok()?{
        0..=2|18|20|29..=32|53=>Some("dry"),6|19=>Some("mixed"),13..=17|26..=28=>Some("snow"),
        3..=5|7..=12|21..=25|301=>Some("rain"),_=>None,
    }
}
fn cn_code(code:&str)->u32 {match cn_kind(code){Some("snow")=>73,Some("mixed")=>66,Some("rain")=>63,
    Some("dry")=>match code.parse::<u32>(){Ok(0)=>0,Ok(1)=>2,Ok(2)=>3,Ok(18)=>45,_=>255},_=>255}}
fn fetch_qq(city:&str)->Result<Value,String>{
    let encode=dotisle_module_storage::http::encode;
    let locations=get(&format!("https://wis.qq.com/city/like?source=pc&city={}",encode(city)))?;
    if locations["status"]!=200{return Err("国内城市检索暂不可用".into())}
    let candidates=locations["data"].as_object().ok_or("国内城市检索无结果")?;
    let matches=candidates.values().filter_map(Value::as_str).filter_map(|raw|{
        let parts=raw.split(',').map(str::trim).collect::<Vec<_>>();
        (parts.len()>=2&&parts.iter().any(|p|p.trim_end_matches('市')==city.trim_end_matches('市')||raw.replace([',',' '],"")==city)).then_some(parts)
    }).collect::<Vec<_>>();
    let parts=if matches.len()==1{&matches[0]}else{return Err("城市未精确匹配或存在重名，请补充省份".into())};
    fetch_qq_location(city,&parts.join(","))
}
fn fetch_qq_location(city:&str,location:&str)->Result<Value,String>{
    let parts=location.split(',').map(str::trim).collect::<Vec<_>>();if parts.len()<2{return Err("城市名称缺失".into())}
    let encode=dotisle_module_storage::http::encode;
    let county=parts.get(2).map(|s|format!("&county={}",encode(s))).unwrap_or_default();
    let raw=get(&format!("https://wis.qq.com/weather/common?source=pc&weather_type=observe%7Cforecast_1h%7Cforecast_24h&province={}&city={}{}",encode(parts[0]),encode(parts[1]),county))?;
    parse_qq(city,&raw,now())
}
fn parse_qq(city:&str,raw:&Value,clock:i64)->Result<Value,String>{
    if raw["status"]!=200{return Err("QQ 天气暂不可用".into())}
    let data=&raw["data"];let current=&data["observe"];
    let observation_at=current["update_time"].as_str().and_then(|s|compact_stamp(s,28800));
    let observation_ok=observation_at.is_some_and(|at|at<=clock+900&&at>=clock-5400)
        &&super::wttr_number(&current["degree"]).is_some_and(|v|(-100. ..=100.).contains(&v));
    // Around local midnight the observation endpoint sometimes stamps yesterday's
    // reading with today's date. Never repair that date by guessing. A valid
    // hourly model sample may still be used, explicitly marked as a forecast.
    let closest=data["forecast_1h"].as_object().into_iter().flat_map(|v|v.values()).filter_map(|v|{
        let at=compact_stamp(v["update_time"].as_str()?,28800)?;
        (at<=clock+900&&at>=clock-3600).then_some((at,v))
    }).min_by_key(|(at,_)|(*at-clock).abs());
    let (at,reading)=if observation_ok{(observation_at.unwrap(),current)}else{closest.ok_or("国内天气缺少时间有效的实况或预报")?};
    let temperature=super::wttr_number(&reading["degree"]).filter(|v|(-100. ..=100.).contains(v)).ok_or("国内天气温度缺失")?;
    let mut slots=data["forecast_1h"].as_object().into_iter().flat_map(|v|v.values()).filter_map(|v|Some(Slot{
        start:compact_stamp(v["update_time"].as_str()?,28800)?,minutes:60,
        kind:cn_kind(v["weather_code"].as_str().unwrap_or("")),probability:None,
    })).collect::<Vec<_>>();slots.sort_by_key(|s|s.start);
    let (rain,through)=windows(&slots,clock,28800,false);
    let today=(clock+28800).div_euclid(86400);
    let daily=data["forecast_24h"].as_object().into_iter().flat_map(|v|v.values()).find(|v|v["time"].as_str().and_then(|date|stamp(&format!("{date} 00:00"),28800)).is_some_and(|at|(at+28800).div_euclid(86400)==today));
    Ok(json!({"city":city,"temperature":temperature,"time":at,"code":cn_code(reading["weather_code"].as_str().unwrap_or("")),
        "temperatureKind":if observation_ok{"observation"}else{"forecast"},"observationRejected":!observation_ok,
        "high":daily.and_then(|d|super::wttr_number(&d["max_degree"])),"low":daily.and_then(|d|super::wttr_number(&d["min_degree"])),
        "precipitation":rain,"precipitationForecastAvailable":through>=clock+10800,"forecastThrough":through,
        "forecastResolutionMinutes":60,"forecastBasis":"hourly-weather-category","utcOffsetSeconds":28800,"source":"QQ 天气","stale":false}))
}
fn parse_seven(p:&Place,raw:&Value,clock:i64)->Result<Value,String>{
    let init=compact_stamp(raw["init"].as_str().ok_or("7Timer 起报时间缺失")?,0).ok_or("7Timer 起报时间无效")?;
    if init>clock+3600||init<clock-36*3600{return Err("7Timer 起报数据已过期".into())}
    let samples=raw["dataseries"].as_array().ok_or("7Timer 预报缺失")?;
    let mut slots=Vec::new();let mut temperatures=Vec::new();
    for sample in samples {
        let Some(hour)=sample["timepoint"].as_i64().filter(|h|*h>0&&*h<=240)else{continue};
        let valid_at=init+hour*3600;let kind=match sample["prec_type"].as_str(){Some("none")=>Some("dry"),Some("rain")=>Some("rain"),Some("snow")=>Some("snow"),Some("frzr"|"icep")=>Some("mixed"),_=>None};
        // timepoint is the forecast valid time, not the end of an accumulated
        // precipitation period. Discrete samples only imply approximate windows.
        slots.push(Slot{start:valid_at,minutes:180,kind,probability:None});
        if let Some(t)=super::number(&sample["temp2m"]).filter(|t|(-100. ..=100.).contains(t)){temperatures.push((valid_at,t,sample["weather"].as_str().unwrap_or("")));}
    }
    slots.sort_by_key(|s|s.start);let (rain,through)=windows(&slots,clock,p.offset.unwrap_or(0),p.offset.is_none());
    let (_,temperature,symbol)=temperatures.iter().min_by_key(|(at,_,_)|(*at-clock).abs()).ok_or("7Timer 温度缺失")?;
    let code=if symbol.starts_with("clear"){0}else if symbol.starts_with("pcloudy"){2}else if symbol.starts_with("mcloudy"){3}else{met_code(symbol)};
    Ok(json!({"city":p.name,"region":p.region,"temperature":temperature,"time":clock,"timestampKind":"retrieved","temperatureKind":"forecast",
        "forecastIssuedAt":init,"forecastBasis":"three-hour-model-samples","code":code,"precipitation":rain,"precipitationForecastAvailable":through>=clock+10800,
        "forecastThrough":through,"forecastResolutionMinutes":180,"source":"7Timer","stale":false}))
}

fn chinese_kind(text:&str)->Option<&'static str>{
    if text.contains("雨夹雪")||text.contains('雹'){Some("mixed")}else if text.contains('雪'){Some("snow")}
    else if text.contains('雨'){Some("rain")}else if ["晴","云","阴","雾","霾","浮尘","扬沙","沙尘","大风"].iter().any(|s|text.contains(s)){Some("dry")}else{None}
}
fn parse_toutiao(city:&str,raw:&Value,clock:i64)->Result<Value,String>{
    let data=&raw["data"];let weather=&data["weather"];
    let same=|v:&str|v.trim().trim_end_matches('市')==city.trim().trim_end_matches('市');
    if !data["city"].as_str().is_some_and(same)||!weather["city_name"].as_str().is_some_and(same){return Err("国内天气返回的城市不匹配".into())}
    // current_time is request time, not observation time. Anchor hour-only
    // forecasts to update_time and require strict sequential hours across midnight.
    let at=stamp(weather["update_time"].as_str().ok_or("国内天气更新时间缺失")?,28800).ok_or("国内天气更新时间无效")?;
    if at>clock+900||at<clock-5400{return Err("国内天气实况已过期".into())}
    let temperature=super::wttr_number(&weather["current_temperature"]).filter(|t|(-100. ..=100.).contains(t)).ok_or("国内天气温度缺失")?;
    let mut slots=Vec::new();let mut previous=None;
    for sample in weather["hourly_forecast"].as_array().into_iter().flatten(){
        let Some(hour)=super::wttr_number(&sample["hour"]).filter(|h|h.fract()==0.&&(0. ..24.).contains(h)).map(|h|h as i64)else{break};
        let start=if let Some(last)=previous{let next=last+3600;if ((next+28800)/3600)%24!=hour{break}next}else{
            let day=(at+28800).div_euclid(86400);let base=day*86400+hour*3600-28800;
            let closest=[base-86400,base,base+86400].into_iter().min_by_key(|t|(*t-at).abs()).unwrap();
            if (closest-at).abs()>7200{return Err("国内小时预报与更新时间不一致".into())}closest
        };
        slots.push(Slot{start,minutes:60,kind:sample["condition"].as_str().and_then(chinese_kind),probability:None});previous=Some(start);
    }
    let (rain,through)=windows(&slots,clock,28800,false);
    let condition=weather["current_condition"].as_str().unwrap_or("");
    let code=match chinese_kind(condition){Some("snow")=>73,Some("mixed")=>66,Some("rain")=>if condition.contains('雷'){95}else{63},
        Some("dry")=>if condition.contains('晴'){0}else if condition.contains('阴'){3}else if condition.contains('云'){2}else{45},_=>255};
    Ok(json!({"city":city,"temperature":temperature,"time":at,"temperatureKind":"observation","code":code,
        "high":super::wttr_number(&weather["high_temperature"]),"low":super::wttr_number(&weather["low_temperature"]),
        "precipitation":rain,"precipitationForecastAvailable":through>=clock+10800,"forecastThrough":through,
        "forecastResolutionMinutes":60,"forecastBasis":"hourly-weather-category","utcOffsetSeconds":28800,"source":"今日头条天气","stale":false}))
}

#[cfg(test)] mod tests{
    use super::*;
    #[test]
    #[ignore = "requires live domestic services using production WinInet transport"]
    fn live_domestic_sources_without_proxy(){
        let mut results=Vec::new();
        for city in ["上海","北京","广州"] {
            for source in [Source::Qq,Source::Toutiao,Source::SevenTimer] {
                let began=Instant::now();let result=Requests::new(city).request(source);
                results.push(match result {
                    Ok(data)=>json!({"city":city,"source":source.name(),"elapsedMs":began.elapsed().as_millis(),
                        "temperatureUsable":data["temperature"].as_f64().is_some(),"forecastUsable":data["precipitationForecastAvailable"]==true,"data":data}),
                    Err(error)=>json!({"city":city,"source":source.name(),"elapsedMs":began.elapsed().as_millis(),"temperatureUsable":false,"forecastUsable":false,"error":error}),
                });
            }
        }
        let success=results.iter().all(|v|v["temperatureUsable"]==true&&v["forecastUsable"]==true);
        let report=json!({"success":success,"transport":"production WinInet / current Windows proxy settings","testedAt":now(),"results":results});
        if let Some(path)=std::env::var_os("DOTISLE_WEATHER_REPORT"){std::fs::write(path,serde_json::to_vec_pretty(&report).unwrap()).unwrap();}
        println!("Domestic production probes: {} passed={success}",results.len());assert!(success,"one or more sources did not provide a complete precipitation timeline; inspect report");
    }
    #[test]
    #[ignore = "requires all public weather services using production WinInet transport"]
    fn live_all_sources(){
        let mut results=Vec::new();
        for city in ["上海","北京","广州"]{for source in super::super::pool::SOURCES{
            let began=Instant::now();let result=Requests::new(city).request(source);
            results.push(match result{
                Ok(data)=>json!({"city":city,"source":source.name(),"elapsedMs":began.elapsed().as_millis(),
                    "forecastUsable":data["precipitationForecastAvailable"]==true,"data":data}),
                Err(error)=>json!({"city":city,"source":source.name(),"elapsedMs":began.elapsed().as_millis(),"forecastUsable":false,"error":error}),
            });
        }}
        let success=results.iter().all(|v|v["forecastUsable"]==true);
        if let Some(path)=std::env::var_os("DOTISLE_WEATHER_REPORT"){
            std::fs::write(path,serde_json::to_vec_pretty(&json!({"success":success,"testedAt":now(),"transport":"production WinInet","results":results})).unwrap()).unwrap();
        }
        for v in &results{println!("{} / {}: usable={} ({}ms) {}",v["city"],v["source"],v["forecastUsable"],v["elapsedMs"],v["error"]);}
        assert!(success,"some sources are unavailable; inspect the per-source production report");
    }
    #[test]fn utc_request_dates_and_offset_timestamps_agree(){
        assert_eq!(utc_text(0),"1970-01-01T00:00Z");
        let epoch=stamp("2026-11-16T16:00:00Z",0).unwrap();assert_eq!(utc_text(epoch),"2026-11-16T16:00Z");
        assert_eq!(iso_stamp("2026-11-17T00:00:00+08:00"),Some(epoch));
        assert_eq!(iso_stamp("2026-11-16T10:00:00-06:00"),Some(epoch));
        assert!(iso_stamp("2026-11-16T10:00:00+25:00").is_none());
    }
    #[test]fn brightsky_preceding_hour_and_station_distance_are_checked(){
        let p=Place{name:"测试".into(),region:Value::Null,lat:31.,lon:121.,offset:Some(28800)};
        let samples=(0..7).map(|h|json!({"source_id":1,"timestamp":format!("1970-01-01T0{h}:00:00Z"),"temperature":12,"precipitation":if h==2{1.}else{0.},"condition":"rain"})).collect::<Vec<_>>();
        let mut raw=json!({"sources":[{"id":1,"distance":20000}],"weather":samples});let v=parse_brightsky(&p,&raw,1000).unwrap();
        assert_eq!(v["precipitation"][0]["startsAt"],3600);assert_eq!(v["precipitation"][0]["endsAt"],7200);
        assert!(v["precipitation"][0]["probability"].is_null());assert_eq!(v["precipitationForecastAvailable"],true);
        raw["sources"][0]["distance"]=json!(50001);assert!(parse_brightsky(&p,&raw,1000).is_err());
    }
    #[test]fn toutiao_cross_midnight_sequence_missing_hours_and_city_mismatch(){
        let samples=[23,0,1,2,3,4,5].iter().map(|hour|json!({"hour":hour.to_string(),"condition":if *hour==1{"雨夹雪"}else{"阴"}})).collect::<Vec<_>>();
        let mut raw=json!({"data":{"city":"上海","weather":{"city_name":"上海","update_time":"2026-11-16 23:45:00","current_temperature":12,"hourly_forecast":samples}}});
        let clock=stamp("2026-11-17 00:15",28800).unwrap();let v=parse_toutiao("上海",&raw,clock).unwrap();
        assert_eq!(v["precipitationForecastAvailable"],true);assert_eq!(v["precipitation"][0]["kind"],"mixed");
        assert_eq!(v["precipitation"][0]["startsAt"],clock+2700);assert!(v["precipitation"][0]["probability"].is_null());
        assert!(parse_toutiao("北京",&raw,clock).is_err());
        raw["data"]["weather"]["hourly_forecast"][2]["hour"]=json!("3");
        assert_eq!(parse_toutiao("上海",&raw,clock).unwrap()["precipitationForecastAvailable"],false);
        raw["data"]["weather"]["hourly_forecast"]=json!([{"hour":"0","condition":"雨"},{"hour":"1","condition":"阴"},{"hour":"2","condition":"阴"},{"hour":"3","condition":"阴"},{"hour":"4","condition":"阴"}]);
        let v=parse_toutiao("上海",&raw,clock).unwrap();assert_eq!(v["precipitationForecastAvailable"],true);
        assert_eq!(v["precipitation"][0]["startsAt"],stamp("2026-11-17 00:00",28800).unwrap());
    }
    #[test]fn date_parsing_and_offsets_cross_midnight(){assert_eq!(stamp("1970-01-01T00:00:00Z",0),Some(0));assert_eq!(stamp("2026-11-17 00:00",28800),stamp("2026-11-16T16:00:00Z",0));assert!(stamp("2026-02-30 00:00",0).is_none());}
    #[test]fn midnight_bad_observation_uses_timed_forecast_without_changing_its_date(){
        let raw=json!({"status":200,"data":{"observe":{"degree":"15","update_time":"202611172350","weather_code":"01"},
            "forecast_1h":{"0":{"degree":"14","update_time":"20261117000000","weather_code":"02"}},"forecast_24h":{}}});
        let clock=stamp("2026-11-17 00:15",28800).unwrap();let value=parse_qq("上海",&raw,clock).unwrap();
        assert_eq!(value["temperature"],14.);assert_eq!(value["temperatureKind"],"forecast");assert_eq!(value["observationRejected"],true);
        assert_eq!(value["time"],stamp("2026-11-17 00:00",28800).unwrap());
        let mut invalid=raw;invalid["data"]["forecast_1h"]=json!({});assert!(parse_qq("上海",&invalid,clock).is_err());
    }
    #[test]fn missing_samples_are_not_a_dry_end(){let slots=[Slot{start:0,minutes:60,kind:Some("rain"),probability:None},Slot{start:3600,minutes:60,kind:None,probability:None}];let (w,through)=windows(&slots,1000,0,false);assert!(w[0]["endsAt"].is_null());assert_eq!(through,3600);}
    #[test]fn three_hour_forecast_is_not_minute_nowcasting(){let raw=json!({"nearest_area":[{"country":[{"value":"China"}]}],"weather":[{"date":"1970-01-01","hourly":[{"time":"900","precipMM":"1","weatherCode":"296","chanceofrain":"80"},{"time":"1200","precipMM":"0","weatherCode":"113"}]}]});let (w,_)=wttr_forecast(&raw,3600).unwrap();assert_eq!(w[0]["resolutionMinutes"],180);assert_eq!(w[0]["startsAt"],3600);assert_eq!(w[0]["endsAt"],14400);}
    #[test]fn seven_timer_timepoint_is_a_valid_time_and_unknown_types_break_coverage(){
        let p=Place{name:"测试".into(),region:Value::Null,lat:31.,lon:121.,offset:Some(28800)};
        let mut raw=json!({"init":"1970010100","dataseries":[
            {"timepoint":3,"prec_type":"rain","temp2m":12,"weather":"pcloudyday"},
            {"timepoint":6,"prec_type":"none","temp2m":12,"weather":"clearday"},
            {"timepoint":9,"prec_type":"none","temp2m":12,"weather":"clearday"}]});
        let value=parse_seven(&p,&raw,15000).unwrap();assert_eq!(value["precipitation"][0]["startsAt"],10800);
        assert_eq!(value["precipitation"][0]["endsAt"],21600);assert_eq!(value["forecastResolutionMinutes"],180);
        assert_eq!(value["code"],2);assert_eq!(value["precipitationForecastAvailable"],true);
        raw["dataseries"][1]["prec_type"]=json!("unknown");assert_eq!(parse_seven(&p,&raw,15000).unwrap()["precipitationForecastAvailable"],false);
    }
    #[test]fn met_next_hour_starts_at_its_timestamp_and_unknown_probability_stays_unknown(){
        let p=Place{name:"测试".into(),region:Value::Null,lat:31.,lon:121.,offset:Some(28800)};
        let samples=(0..6).map(|h|json!({"time":format!("1970-01-01T0{h}:00:00Z"),"data":{
            "instant":{"details":{"air_temperature":12}},
            "next_1_hours":{"summary":{"symbol_code":"lightrain"},"details":{"precipitation_amount":if h==1{1.}else{0.}}}
        }})).collect::<Vec<_>>();
        let v=parse_met(&p,&json!({"properties":{"timeseries":samples}}),1000).unwrap();
        assert_eq!(v["precipitation"][0]["startsAt"],3600);assert_eq!(v["precipitation"][0]["endsAt"],7200);
        assert!(v["precipitation"][0]["probability"].is_null());assert_eq!(v["precipitationForecastAvailable"],true);
    }
}
