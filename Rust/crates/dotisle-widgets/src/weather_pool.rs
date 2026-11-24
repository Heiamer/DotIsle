//! Capability-first failover with measured latency, cooldowns and bounded per-city caching.
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::{Arc, Mutex, OnceLock}, time::{Duration, Instant}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source { Qq, Toutiao, OpenMeteo, MetNorway, BrightSky, Wttr, SevenTimer }
impl Source {
    pub fn name(self) -> &'static str { match self {Self::Qq=>"QQ 天气",Self::Toutiao=>"今日头条天气",Self::OpenMeteo=>"Open-Meteo",Self::MetNorway=>"MET Norway",Self::BrightSky=>"DWD / Bright Sky",Self::Wttr=>"wttr.in",Self::SevenTimer=>"7Timer"} }
    fn resolution(self) -> u32 { if matches!(self,Self::Wttr|Self::SevenTimer) {180} else {60} }
}
pub(super) const SOURCES: [Source;7] = [Source::Qq,Source::Toutiao,Source::OpenMeteo,Source::MetNorway,Source::BrightSky,Source::Wttr,Source::SevenTimer];
#[derive(Clone, Default)]
struct Health { latency_ms: Option<u64>, failed_at: Option<Instant>, failures: u32 }
impl Health {
    fn cooling(&self) -> bool { self.failed_at.is_some_and(|at|at.elapsed()<Duration::from_secs((30u64<<self.failures.min(4)).min(600))) }
    fn record(&mut self, elapsed: Duration, usable: bool) {
        if usable {
            let measured=elapsed.as_millis().min(u64::MAX as u128) as u64;
            self.latency_ms=Some(self.latency_ms.map_or(measured,|old|(old*3+measured)/4));
            self.failed_at=None; self.failures=0;
        } else { self.failures=self.failures.saturating_add(1);self.failed_at=Some(Instant::now()); }
    }
}
#[derive(Default)]
struct Pool { health: [Health;7] }
impl Pool {
    fn order(&self) -> Vec<usize> {
        let mut order=(0..SOURCES.len()).collect::<Vec<_>>();
        order.sort_by_key(|&i|(self.health[i].cooling(),SOURCES[i].resolution(),self.health[i].latency_ms.unwrap_or(5000+i as u64*1000),i));
        order
    }
    fn fetch(&mut self, mut request: impl FnMut(Source)->Result<Value,String>) -> Result<Value,String> {
        let mut partial=None;let mut failures=Vec::new();let mut attempts=Vec::new();
        for index in self.order() {
            let source=SOURCES[index];
            // Only failover performs another request. A healthy domestic source
            // must not wait for unrelated overseas probes on the first refresh.
            let began=Instant::now();let result=request(source);let elapsed=began.elapsed();
            let usable=result.as_ref().is_ok_and(complete);
            attempts.push(json!({"source":source.name(),"elapsedMs":elapsed.as_millis(),"forecastUsable":usable}));
            self.health[index].record(elapsed,usable);
            match result {
                Ok(mut data) if usable => {
                    data["sourceAttempts"]=json!(attempts);
                    if !failures.is_empty(){data["sourceWarnings"]=json!(failures);}
                    return Ok(data)
                }
                Ok(data) => {failures.push(format!("{} 降水时序不完整",source.name()));if partial.is_none(){partial=Some(data);}}
                Err(_) => failures.push(format!("{} 暂不可用",source.name())),
            }
        }
        let mut data=partial.ok_or("天气源暂时均不可用，请稍后重试")?;
        data["sourceAttempts"]=json!(attempts);
        if !failures.is_empty(){data["sourceWarnings"]=json!(failures);}
        Ok(data)
    }
}
pub(super) fn complete(data: &Value) -> bool {
    data["temperature"].as_f64().is_some_and(f64::is_finite)
        && data["precipitationForecastAvailable"]==true
        && data["forecastThrough"].as_i64().zip(data["time"].as_i64()).is_some_and(|(end,now)|end>=now+3*3600)
        && data["stale"]!=true
}
#[derive(Default)]
struct City { at: Option<Instant>, result: Option<Result<Value,String>>, last_good:Option<(Instant,Value)>, pool: Pool }
type Cities = BTreeMap<String, Arc<Mutex<City>>>;
pub(super) fn fetch(city: &str) -> Result<Value,String> {
    let city=city.trim();if city.is_empty(){return Err("请先设置城市".into())}
    static CITIES: OnceLock<Mutex<Cities>>=OnceLock::new();
    let state={
        let mut cities=CITIES.get_or_init(Default::default).lock().map_err(|_|"天气缓存不可用")?;
        if !cities.contains_key(city) && cities.len()>=16 {
            let oldest=cities.iter().filter(|(_,v)|Arc::strong_count(v)==1).filter_map(|(k,v)|v.try_lock().ok().map(|c|(k.clone(),c.at)))
                .min_by_key(|(_,at)|*at).map(|(key,_)|key);
            if let Some(key)=oldest{cities.remove(&key);}
        }
        if cities.len()>=32 && !cities.contains_key(city){return Err("同时查询的城市过多，请稍后重试".into())}
        cities.entry(city.to_owned()).or_default().clone()
    };
    // Different cities do not hold a shared lock during network operations.
    // The same city shares one flight between the overview and the island alert.
    let mut state=state.lock().map_err(|_|"天气缓存不可用")?;
    if let (Some(at),Some(result))=(state.at,state.result.as_ref()) {
        let ttl=if result.as_ref().is_ok_and(complete){600}else{30};
        if at.elapsed()<Duration::from_secs(ttl){return result.clone()}
    }
    let mut requests=super::providers::Requests::new(city);
    let result=state.pool.fetch(|source|requests.request(source));
    state.finish(result)
}

impl City {
    fn finish(&mut self,result:Result<Value,String>)->Result<Value,String>{
        let now=Instant::now();
        let result=match result {
            Ok(data)=>{self.last_good=Some((now,data.clone()));Ok(data)}
            Err(message)=>match self.last_good.as_ref().filter(|(at,_)|at.elapsed()<Duration::from_secs(1800)) {
                Some((_,data))=>{let mut stale=data.clone();stale["stale"]=json!(true);stale["error"]=json!(message);Ok(stale)}
                None=>Err(message),
            },
        };
        self.at=Some(now);self.result=Some(result.clone());result
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn data(source:Source, available:bool)->Value {json!({"temperature":12,"time":1000,"forecastThrough":20000,
        "precipitationForecastAvailable":available,"forecastResolutionMinutes":source.resolution(),"precipitation":[],"source":source.name(),"stale":false})}
    #[test] fn missing_forecast_continues_and_dry_complete_forecast_stops() {
        let mut pool=Pool::default();let mut queried=Vec::new();
        let result=pool.fetch(|s|{queried.push(s);if matches!(s,Source::Qq|Source::Toutiao){return Err("offline".into())}Ok(data(s,s!=Source::OpenMeteo))}).unwrap();
        assert_eq!(result["source"],"MET Norway");assert_eq!(queried,vec![Source::Qq,Source::Toutiao,Source::OpenMeteo,Source::MetNorway]);
        assert!(result["precipitation"].as_array().unwrap().is_empty());
    }
    #[test] fn hourly_beats_fast_three_hour_data_and_latency_orders_peers() {
        let mut pool=Pool::default();for (health,ms) in pool.health.iter_mut().zip([300,400,1000,500,800,5,8]){health.latency_ms=Some(ms);}
        assert_eq!(pool.order(),vec![0,1,3,4,2,5,6]);let mut queried=Vec::new();
        pool.fetch(|s|{queried.push(s);Ok(data(s,true))}).unwrap();assert_eq!(queried,vec![Source::Qq]);
    }
    #[test] fn failures_cool_down_and_total_outage_keeps_partial_temperature() {
        let mut pool=Pool::default();pool.health[0].record(Duration::ZERO,false);assert_ne!(pool.order()[0],0);
        let result=pool.fetch(|s|if s==Source::Wttr{Ok(data(s,false))}else{Err("offline".into())}).unwrap();
        assert_eq!(result["precipitationForecastAvailable"],false);
        assert!(pool.fetch(|_|Err("offline".into())).is_err());
    }
    #[test]fn outage_can_use_last_reading_but_not_as_a_fresh_forecast(){
        let mut city=City::default();
        assert!(city.finish(Err("offline".into())).is_err());
        city.finish(Ok(data(Source::Qq,true))).unwrap();
        let stale=city.finish(Err("offline".into())).unwrap();
        assert_eq!(stale["temperature"],12);assert_eq!(stale["stale"],true);assert!(!complete(&stale));
        let recovered=city.finish(Ok(data(Source::OpenMeteo,true))).unwrap();
        assert!(complete(&recovered));assert!(recovered["error"].is_null());
    }
    #[test]fn every_provider_can_take_over_and_repeated_success_does_not_probe_other_sources(){
        for selected in SOURCES{
            let mut pool=Pool::default();let result=pool.fetch(|source|if source==selected{Ok(data(source,true))}else{Err("injected outage".into())}).unwrap();
            assert_eq!(result["source"],selected.name());
            let mut requests=0;pool.fetch(|source|{requests+=1;assert_eq!(source,selected);Ok(data(source,true))}).unwrap();assert_eq!(requests,1);
        }
    }
}
