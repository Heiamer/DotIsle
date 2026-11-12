"""Real isolated-pipe acceptance: replace only focus DLL, keep CORE byte-identical, rollback."""
import argparse, hashlib, json, os, pathlib, time, traceback
from verify_pomodoro import Runtime

def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def await_state(app, predicate, timeout=120):
    deadline=time.monotonic()+timeout; last=None
    while time.monotonic()<deadline:
        try:
            last=app.request('state')
            if predicate(last): return last
        except (OSError,TimeoutError): pass
        time.sleep(.1)
    raise AssertionError(f'Module state timed out: {last}')

def stage(app, source, expected=True):
    reply=app.request('moduleStage',dict(id='focus',path=str(source.resolve())))
    assert reply['success'],reply
    result=await_state(app,lambda v:v.get('moduleManagement',{}).get('phase') in ('staged','failed'))
    phase=result['moduleManagement']['phase']
    assert (phase=='staged')==expected,result['moduleManagement']
    return result

def apply(app, success=True):
    previous=app.request('state')['processId']
    journal=app.data/'module-update-result.json'
    old=journal.read_bytes() if journal.exists() else None
    reply=app.request('moduleApply',dict(callerSettingsPid=0))
    assert reply['success'],reply
    deadline=time.monotonic()+150; result=None
    while time.monotonic()<deadline:
        try:
            content=journal.read_bytes()
            if content!=old:
                value=json.loads(content)
                if value.get('phase') in ('completed','failed'):
                    result=value;break
        except (OSError,ValueError): pass
        time.sleep(.1)
    assert result is not None,'Module helper did not finish'
    assert (result['phase']=='completed')==success,result
    state=await_state(app,lambda v:v['processId']!=previous and v.get('moduleReady') is True)
    return state,result

def run(a):
    runtime=a.runtime.resolve();a.scratch.mkdir(parents=True,exist_ok=True)
    app=Runtime(runtime,a.scratch/'data');report=dict(success=False,checks=[],coreHashBefore=sha(runtime/'dotisle_core.dll'))
    focus=runtime/'dotisle_focus.dll';original=sha(focus)
    try:
        # Production acceptance must load the packaged files, not a developer's target directory.
        os.environ.pop('DOTISLE_MODULE_DIR',None)
        state=app.start();assert state['moduleReady'],state.get('moduleErrors')
        report['checks'].append('all service sessions initialize from packaged DLLs')
        identifier=app.create(dict(mode='countdown',duration=600,name='module acceptance'))
        assert 'DLL-PROBE' not in app.timer(identifier)['subtitle']
        stage(app,a.probe);state,result=apply(app)
        assert sha(focus)==sha(a.probe)
        assert any(t['id']==identifier and 'DLL-PROBE' in t['subtitle'] for t in state['timers']),state['timers']
        assert sha(runtime/'dotisle_core.dll')==report['coreHashBefore']
        report['checks'].append('focus-only DLL changes behavior while CORE remains byte-identical')
        reply=app.request('moduleRollback',dict(id='focus'));assert reply['success'],reply
        await_state(app,lambda v:v.get('moduleManagement',{}).get('phase') in ('staged','failed'))
        assert app.request('state')['moduleManagement']['phase']=='staged'
        state,result=apply(app);assert sha(focus)==original
        assert all('DLL-PROBE' not in t['subtitle'] for t in state['timers'])
        report['checks'].append('previous DLL can be restored through the same transactional interface')
        stage(app,runtime/'dotisle_themes.dll',False);assert sha(focus)==original
        bad=a.scratch/'not-a-module.dll';bad.write_bytes(b'not a valid PE DLL')
        stage(app,bad,False);assert sha(focus)==original
        report['checks'].append('wrong module identity and invalid PE are rejected before touching live DLLs')
        stage(app,a.broken);state,result=apply(app,False)
        assert sha(focus)==original
        assert sha(runtime/'dotisle_core.dll')==report['coreHashBefore']
        assert state['moduleReady'] is True
        report['checks'].append('startup failure automatically rolls back and restarts the old working module')
        report.update(success=True,coreHashAfter=sha(runtime/'dotisle_core.dll'),moduleVersions=state['moduleVersions'])
    except Exception:
        report['error']=traceback.format_exc()
        raise
    finally:
        try: app.request('prepareUpdateExit')
        except (OSError,TimeoutError): pass
        app.stop()
        a.report.parent.mkdir(parents=True,exist_ok=True)
        a.report.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    return report

if __name__=='__main__':
    p=argparse.ArgumentParser()
    for name in ['runtime','probe','broken','scratch','report']:p.add_argument('--'+name,type=pathlib.Path,required=True)
    run(p.parse_args())
