//! Hourly precipitation windows. Missing samples never become a fabricated end.
use serde_json::{json, Value};

pub fn fetch(city: &str) -> Result<Value, String> {
    fetch_sources(city, || fetch_open_meteo(city), || {
        let data=crate::weather_http::get_json(&format!("https://wttr.in/{}?format=j1",dotisle_module_storage::http::encode(city.trim())))?;
        parse_wttr(city,&data)
    })
}
fn fetch_sources(city:&str,primary:impl FnOnce()->Result<Value,String>,fallback:impl FnOnce()->Result<Value,String>)->Result<Value,String>{
    if city.trim().is_empty(){return Err("请先设置城市".into())}
    match primary(){Ok(data)=>Ok(data),Err(first)=>match fallback(){
        Ok(mut data)=>{data["sourceWarnings"]=json!([format!("Open-Meteo 暂不可用，已切换 wttr.in：{first}")]);Ok(data)},
        Err(_)=>Err("天气服务暂不可用，两个来源均未返回有效数据，请稍后重试".into())
    }}
}
fn fetch_open_meteo(city: &str) -> Result<Value, String> {
    let get = |url: &str| crate::weather_http::get_json(url)
        .map_err(|_| "天气服务暂不可用，请稍后重试".to_owned());
    let location = get(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh&format=json",
        dotisle_module_storage::http::encode(city.trim())
    ))?;
    let place = location["results"].as_array().and_then(|v| v.first())
        .ok_or("未找到城市，请补充省份或国家")?;
    let lat = place["latitude"].as_f64().ok_or("城市坐标缺失")?;
    let lon = place["longitude"].as_f64().ok_or("城市坐标缺失")?;
    let response = get(&format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,is_day&hourly=precipitation,rain,showers,snowfall,precipitation_probability&daily=temperature_2m_max,temperature_2m_min&forecast_days=2&timezone=auto&timeformat=unixtime"))?;
    parse(place, &response)
}

fn number(v: &Value) -> Option<f64> { v.as_f64().filter(|v| v.is_finite()) }
fn wttr_number(v:&Value)->Option<f64>{v.as_f64().or_else(||v.as_str()?.parse().ok()).filter(|v|v.is_finite())}
fn parse_wttr(city:&str,data:&Value)->Result<Value,String>{
    let c=&data["current_condition"][0];
    let temperature=wttr_number(&c["temp_C"]).filter(|v|(-100. ..=100.).contains(v)).ok_or("天气数据暂不可用")?;
    let weather_code=c["weatherCode"].as_u64().and_then(|v|u32::try_from(v).ok()).or_else(||c["weatherCode"].as_str()?.parse::<u32>().ok()).unwrap_or(0);
    let code=match weather_code {
        113=>0,116=>2,119|122=>3,143|149|248|260=>45,
        176|263|266=>51,281|284=>56,293|296|353=>61,299|302|356=>63,
        305|308|359=>65,185|311|314=>66,179|182|227|323|326|368=>71,
        230|329|332|335|338|371=>75,200|386|389|392|395=>95,_=>255,
    };
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    // This endpoint does not always include a timezone or observation date.
    // Label the timestamp as retrieval time and do not invent hourly rain windows.
    Ok(json!({"city":city.trim(),"region":data["nearest_area"][0]["region"][0]["value"],
        "temperature":temperature,"code":code,"time":now,"timestampKind":"retrieved",
        "observationTimeLabel":c["observation_time"],
        "high":wttr_number(&data["weather"][0]["maxtempC"]),"low":wttr_number(&data["weather"][0]["mintempC"]),
        "precipitation":[],"precipitationForecastAvailable":false,"source":"wttr.in","stale":false}))
}
fn sample(h: &Value, i: usize) -> Option<&'static str> {
    let total = number(&h["precipitation"][i])?;
    let snow = number(&h["snowfall"][i])?;
    let rain = number(&h["rain"][i])? + number(&h["showers"][i])?;
    Some(if snow >= 0.01 && rain >= 0.1 { "mixed" }
        else if snow >= 0.01 { "snow" }
        else if total >= 0.1 { "rain" } else { "dry" })
}
fn label(at: i64, now: i64, offset: i64) -> String {
    let local = at + offset;
    let day = local.div_euclid(86400) - (now + offset).div_euclid(86400);
    let clock = local.rem_euclid(86400);
    format!("{}{:02}:{:02}", match day { 0 => "今天", 1 => "明天", _ => "后天" }, clock / 3600, clock / 60 % 60)
}
pub fn parse(place: &Value, response: &Value) -> Result<Value, String> {
    let c = &response["current"];
    let temperature = number(&c["temperature_2m"]).ok_or("天气数据暂不可用")?;
    let now = c["time"].as_i64().ok_or("天气时间缺失")?;
    let offset = response["utc_offset_seconds"].as_i64().unwrap_or(0);
    let h = &response["hourly"];
    let times = h["time"].as_array().ok_or("降水预报暂不可用")?;
    let mut windows = Vec::new();
    let mut i = 0;
    while i < times.len() {
        let Some(start) = times[i].as_i64() else { i += 1; continue };
        if start + 3600 <= now || start > now + 86400 { i += 1; continue }
        let Some(kind) = sample(h, i).filter(|k| *k != "dry") else { i += 1; continue };
        let mut j = i + 1;
        while j < times.len() && times[j].as_i64() == Some(start + (j-i) as i64 * 3600)
            && sample(h, j) == Some(kind) { j += 1; }
        // A changed wet type or a dry sample is evidence of an end; a gap is not.
        let end = if times.get(j).and_then(Value::as_i64) == Some(start + (j-i) as i64 * 3600)
            && sample(h, j).is_some() { times[j].as_i64() } else { None };
        windows.push(json!({"kind":kind,"startsAt":start,"endsAt":end,
            "startLabel":label(start,now,offset),"endLabel":end.map(|v|label(v,now,offset)),
            "ongoing":start<=now,"probability":h["precipitation_probability"][i],
            "resolutionMinutes":60,"approximate":true}));
        i = j;
    }
    Ok(json!({"city":place["name"],"region":place["admin1"],"temperature":temperature,
        "code":c["weather_code"],"day":c["is_day"],"time":now,"utcOffsetSeconds":offset,
        "high":response["daily"]["temperature_2m_max"][0],"low":response["daily"]["temperature_2m_min"][0],
        "precipitation":windows,"precipitationForecastAvailable":true,"source":"Open-Meteo","stale":false}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the public wttr.in service"]
    fn live_wttr_transport_and_normalization(){
        let mut results=serde_json::Map::new();
        for city in ["Shanghai","Beijing"] {
            let raw=crate::weather_http::get_json(&format!("https://wttr.in/{city}?format=j1")).unwrap();
            let value=parse_wttr(city,&raw).unwrap();
            assert_eq!(value["source"],"wttr.in");assert!(value["temperature"].as_f64().is_some());
            assert_eq!(value["precipitationForecastAvailable"],false);
            results.insert(city.into(),value);
        }
        if let Some(path)=std::env::var_os("DOTISLE_WEATHER_REPORT") {
            std::fs::write(path,serde_json::to_vec_pretty(&json!({"success":true,"cities":results})).unwrap()).unwrap();
        }
    }
    #[test] fn fallback_preserves_temperature_and_never_claims_missing_rain_forecasts(){
        let response=json!({"current_condition":[{"temp_C":"15","weatherCode":"116","observation_time":"11:43 AM"}],
            "weather":[{"maxtempC":"18","mintempC":"13"}]});
        let v=fetch_sources("上海",||Err("timeout".into()),||parse_wttr("上海",&response)).unwrap();
        assert_eq!(v["temperature"],15.);assert_eq!(v["source"],"wttr.in");assert_eq!(v["code"],2);
        assert_eq!(v["precipitationForecastAvailable"],false);assert_eq!(v["timestampKind"],"retrieved");
        assert_eq!(v["precipitation"],json!([]));assert!(v["sourceWarnings"].is_array());
        assert!(parse_wttr("上海",&json!({})).is_err());
        assert!(fetch_sources("上海",||Err("timeout".into()),||Err("timeout".into())).is_err());
        assert_eq!(fetch_sources("上海",||Ok(json!({"source":"Open-Meteo"})),||panic!("fallback must stay secondary")).unwrap()["source"],"Open-Meteo");
    }
    fn fixture() -> Value { json!({"current":{"temperature_2m":12,"time":1000},"utc_offset_seconds":28800,
        "hourly":{"time":[0,3600,7200,10800,14400],"precipitation":[0,1,1,1,0],
            "rain":[0,1,1,0,0],"showers":[0,0,0,0,0],"snowfall":[0,0,0,0.3,0]}}) }
    #[test] fn separates_rain_and_snow_and_reports_both_ends() {
        let v=parse(&json!({"name":"测试城市"}),&fixture()).unwrap();
        assert_eq!(v["precipitation"][0]["kind"],"rain");
        assert_eq!(v["precipitation"][0]["startsAt"],3600);
        assert_eq!(v["precipitation"][0]["endsAt"],10800);
        assert_eq!(v["precipitation"][1]["kind"],"snow");
        assert_eq!(v["precipitation"][1]["endsAt"],14400);
    }
    #[test] fn missing_data_is_not_an_end_or_dry_weather() {
        let mut f=fixture(); f["hourly"]["precipitation"][2]=Value::Null;
        let v=parse(&json!({}),&f).unwrap();
        assert!(v["precipitation"][0]["endsAt"].is_null());
    }
    #[test] fn local_labels_cross_midnight_and_temperature_is_required() {
        assert_eq!(label(7200,0,23*3600),"明天01:00");
        let mut f=fixture(); f["current"]["temperature_2m"]=Value::Null;
        assert!(parse(&json!({}),&f).is_err());
    }
}
