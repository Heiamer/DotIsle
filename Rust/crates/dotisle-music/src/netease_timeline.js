(() => {
  if (location.href.split(/[?#]/)[0] !== 'orpheus://orpheus/pub/app.html') throw Error('page_mismatch');
  const element = document.getElementById('root');
  if (!element) throw Error('root_unavailable');
  const key = Object.keys(element).find(k => /^__react(Container|Fiber)\$/.test(k));
  const root = element._reactRootContainer?._internalRoot?.current || element[key]?.current || element[key];
  const stack = root ? [root] : [];
  const seen = new Set();
  const until = Date.now() + 100;
  let playing;
  while (stack.length && seen.size < 4096 && Date.now() < until) {
    const node = stack.pop();
    if (!node || seen.has(node)) continue;
    seen.add(node);
    const props = node.memoizedProps || {};
    for (const candidate of [props.store, props.value?.store, node.stateNode?.store]) {
      if (typeof candidate?.getState === 'function') {
        const state = candidate.getState().playing;
        if (state?.resourceType === 'track' && state.resourceTrackId) { playing = state; break; }
      }
    }
    if (playing) break;
    if (node.sibling) stack.push(node.sibling);
    if (node.child) stack.push(node.child);
  }
  if (!playing || playing.isLoadingFirst || playing.isPlayingVideo) throw Error('track_unavailable');
  const id = String(playing.resourceTrackId);
  const track = playing.curPlaying?.track;
  if (!/^\d+$/.test(id) || !track || String(track.id) !== id) throw Error('metadata_mismatch');
  const duration = Number(track.duration) / 1000 || Number(playing.resourceDuration);
  const sliders = [...document.querySelectorAll('[aria-label="播放进度调节"]')];
  if (sliders.length !== 1) throw Error('timeline_unavailable');
  const percentText = sliders[0].style.getPropertyValue('--track-precent').trim();
  if (!/^\d+(\.\d+)?%$/.test(percentText)) throw Error('position_unavailable');
  const fraction = Number(percentText.slice(0, -1)) / 100;
  if (!Number.isFinite(duration) || duration <= 0 || duration > 86400 || fraction < 0 || fraction > 1) throw Error('invalid_timeline');
  if (![1,2].includes(playing.playingState)) throw Error('playback_state_unavailable');
  // This client uses 1 for paused and 2 for playing. A restored item has no live playId yet.
  const restored = playing.restoreResource;
  if (restored && (String(restored.trackId || restored.id) !== id || restored.pause !== true || playing.playingState !== 1 || !Number.isFinite(restored.current) || Math.abs(restored.current - duration * fraction) > Math.max(0.6, duration / 1000))) throw Error('restore_unavailable');
  return {
    songId: id, title: String(track.name || playing.resourceName || ''),
    artist: (track.artists || track.ar || []).map(a => a.name).filter(Boolean).join(' / '),
    album: String((track.album || track.al)?.name || ''),
    cover: String(playing.resourceCoverUrl || (track.album || track.al)?.picUrl || ''),
    position: restored ? restored.current : duration * fraction, duration, playing: playing.playingState === 2,
    controlReady: !restored && typeof playing.playId === 'string' && !!playing.playId,
    capturedMs: Date.now(), route: 'player-page'
  };
})()
