// Read-only page fixtures exercise the exact script embedded by the Rust backend.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const test = require('node:test');
const path = require('node:path');
const source = fs.readFileSync(path.join(__dirname, '../Rust/crates/dotisle-music/src/netease_timeline.js'), 'utf8');
function fixture(change = () => {}) {
  const playing = {resourceType:'track', resourceTrackId:71, playingState:2, playId:'71_live',
    curPlaying:{track:{id:71,name:'雾中的灯',duration:180000,artists:[{name:'测试歌手'}],album:{name:'海岸'}}}};
  const f = {playing, percent:'25%', href:'orpheus://orpheus/pub/app.html', sliders:1};
  change(f);
  let writes = 0;
  function readonly(value) {
    if (!value || typeof value !== 'object') return value;
    for (const k of Object.keys(value)) value[k] = readonly(value[k]);
    return new Proxy(Object.freeze(value), {set(){writes++;throw Error('write attempted')}, deleteProperty(){writes++;throw Error('delete attempted')}});
  }
  const state = readonly({playing});
  const store = {getState:()=>state,dispatch:()=>{throw Error('dispatch attempted')},subscribe:()=>{throw Error('listener attempted')}};
  const fiber = {memoizedProps:{value:{store}}}; fiber.sibling = fiber;
  const element = {_reactRootContainer:{_internalRoot:{current:fiber}}};
  const context = {location:{href:f.href},document:{getElementById:()=>element,
    querySelectorAll:selector=>{assert.equal(selector,'[aria-label="播放进度调节"]');return Array.from({length:f.sliders},()=>({style:{getPropertyValue:key=>{assert.equal(key,'--track-precent');return f.percent}}}))}}};
  return {run:()=>vm.runInNewContext(source,context,{timeout:500}),writes:()=>writes};
}
test('reads metadata, progress and active state without writes or listeners',()=>{
  const f=fixture(); const result=f.run();
  assert.equal(result.songId,'71');assert.equal(result.position,45);assert.equal(result.duration,180);
  assert.equal(result.title,'雾中的灯');assert.equal(result.playing,true);assert.equal(f.writes(),0);
});
test('paused state does not become playing',()=>assert.equal(fixture(f=>f.playing.playingState=1).run().playing,false));
test('rejects a changing identity before returning progress',()=>assert.throws(()=>fixture(f=>f.playing.curPlaying.track.id=72).run(),/metadata_mismatch/));
test('rejects the wrong page and ambiguous sliders',()=>{
  assert.throws(()=>fixture(f=>f.href='https://example.com/').run(),/page_mismatch/);
  for(const count of [0,2])assert.throws(()=>fixture(f=>f.sliders=count).run(),/timeline_unavailable/);
});
test('rejects unsupported states and invalid progress',()=>{
  for(const state of [0,3,null])assert.throws(()=>fixture(f=>f.playing.playingState=state).run(),/playback_state_unavailable/);
  for(const percent of ['NaN%','101%','-1%','0.5',''])assert.throws(()=>fixture(f=>f.percent=percent).run(),/position_unavailable|invalid_timeline/);
});
test('rejects loading and video',()=>{
  for(const key of ['isLoadingFirst','isPlayingVideo'])assert.throws(()=>fixture(f=>f.playing[key]=true).run(),/track_unavailable/);
});
test('reads a restored paused song without advertising seek readiness',()=>{
  const f=fixture(f=>{f.playing.playingState=1;f.playing.playId='';f.playing.restoreResource={id:'71',trackId:'71',pause:true,current:45};});
  const r=f.run();assert.equal(r.position,45);assert.equal(r.playing,false);assert.equal(r.controlReady,false);assert.equal(f.writes(),0);
});
test('rejects inconsistent restored identity, state or position',()=>{
  for(const patch of [{trackId:'72'},{pause:false},{current:51},{current:NaN}])assert.throws(()=>fixture(f=>{f.playing.playingState=1;f.playing.restoreResource={id:'71',trackId:'71',pause:true,current:45,...patch};}).run(),/restore_unavailable/);
});
test('live control requires a playback instance',()=>{
  assert.equal(fixture().run().controlReady,true);assert.equal(fixture(f=>f.playing.playId='').run().controlReady,false);
});
