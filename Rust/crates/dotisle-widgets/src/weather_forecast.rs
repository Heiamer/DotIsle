//! Hourly precipitation windows. Missing samples never become a fabricated end.
use serde_json::{json, Value};
#[path="weather_pool.rs"] mod pool;
#[path="weather_providers.rs"] mod providers;

pub fn fetch(city: &str) -> Result<Value, String> {
    pool::fetch(city)
}
pub(super) fn complete(value:&Value)->bool{pool::complete(value)}
#[cfg(test)]
fn fetch_sources(city:&str,primary:impl FnOnce()->Result<Value,String>,fallback:impl FnOnce()->Result<Value,String>)->Result<Value,String>{
    if city.trim().is_empty(){return Err("请先设置城市".into())}
    match primary(){Ok(data)=>Ok(data),Err(first)=>match fallback(){
        Ok(mut data)=>{data["sourceWarnings"]=json!([format!("Open-Meteo 暂不可用，已切换 wttr.in：{first}")]);Ok(data)},
        Err(_)=>Err("天气服务暂不可用，两个来源均未返回有效数据，请稍后重试".into())
    }}
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
    let (rain,through)=providers::wttr_forecast(data,now as i64).unwrap_or((json!([]),now as i64));
    // Three-hour samples remain explicitly approximate. Missing timezone or
    // hourly data keeps temperature usable while the pool tries another source.
    Ok(json!({"city":city.trim(),"region":data["nearest_area"][0]["region"][0]["value"],
        "temperature":temperature,"code":code,"time":now,"timestampKind":"retrieved",
        "observationTimeLabel":c["observation_time"],
        "high":wttr_number(&data["weather"][0]["maxtempC"]),"low":wttr_number(&data["weather"][0]["mintempC"]),
        "precipitation":rain,"precipitationForecastAvailable":through>=now as i64+10800,
        "forecastThrough":through,"forecastResolutionMinutes":180,"source":"wttr.in","stale":false}))
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
    let date=match day {-1=>"昨天".into(),0=>"今天".into(),1=>"明天".into(),2=>"后天".into(),d if d<0=>format!("{}天前",-d),d=>format!("{d}天后")};
    format!("{date}{:02}:{:02}", clock / 3600, clock / 60 % 60)
}
pub fn parse(place: &Value, response: &Value) -> Result<Value, String> {
    let c = &response["current"];
    let temperature = number(&c["temperature_2m"]).ok_or("天气数据暂不可用")?;
    let now = c["time"].as_i64().ok_or("天气时间缺失")?;
    let offset = response["utc_offset_seconds"].as_i64().unwrap_or(0);
    let h = &response["hourly"];
    let times = h["time"].as_array().ok_or("降水预报暂不可用")?;
    // Open-Meteo's hourly precipitation is the preceding hour's accumulation;
    // MET next_1_hours is the following hour. Normalize these before merging.
    let slots=times.iter().enumerate().filter_map(|(i,time)|Some(providers::Slot{
        start:time.as_i64()?-3600,minutes:60,kind:sample(h,i),
        probability:number(&h["precipitation_probability"][i]).filter(|v|(0. ..=100.).contains(v)),
    })).collect::<Vec<_>>();
    let (windows,through)=providers::windows(&slots,now,offset,false);
    Ok(json!({"city":place["name"],"region":place["admin1"],"temperature":temperature,
        "code":c["weather_code"],"day":c["is_day"],"time":now,"utcOffsetSeconds":offset,
        "high":response["daily"]["temperature_2m_max"][0],"low":response["daily"]["temperature_2m_min"][0],
        "precipitation":windows,"precipitationForecastAvailable":through>=now+10800,
        "forecastThrough":through,"forecastResolutionMinutes":60,"source":"Open-Meteo","stale":false}))
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
            assert_eq!(value["forecastResolutionMinutes"],180);
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
        assert_eq!(v["precipitation"][0]["startsAt"],0);
        assert_eq!(v["precipitation"][0]["endsAt"],7200);
        assert_eq!(v["precipitation"][1]["kind"],"snow");
        assert_eq!(v["precipitation"][1]["endsAt"],10800);
    }
    #[test] fn missing_data_is_not_an_end_or_dry_weather() {
        let mut f=fixture(); f["hourly"]["precipitation"][2]=Value::Null;
        let v=parse(&json!({}),&f).unwrap();
        assert!(v["precipitation"][0]["endsAt"].is_null());
    }
    #[test] fn local_labels_cross_midnight_and_temperature_is_required() {
        assert_eq!(label(7200,0,23*3600),"明天01:00");
        assert_eq!(label(23*3600,24*3600,0),"昨天23:00");
        assert_eq!(label(4*86400,0,0),"4天后00:00");
        let mut f=fixture(); f["current"]["temperature_2m"]=Value::Null;
        assert!(parse(&json!({}),&f).is_err());
    }
}
