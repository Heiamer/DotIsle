"""Real packaged runtime: Go owns the center and embeds desktop widgets in-process."""
import argparse
import ctypes
from ctypes import wintypes as w
import json
import os
import subprocess
import shutil
import time
from pathlib import Path
from verify_pomodoro import Runtime

kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.CreateToolhelp32Snapshot.argtypes=[w.DWORD,w.DWORD]
kernel.CreateToolhelp32Snapshot.restype=w.HANDLE
kernel.CloseHandle.argtypes=[w.HANDLE]
user=ctypes.WinDLL('user32',use_last_error=True)
WindowCallback=ctypes.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
user.EnumWindows.argtypes=[WindowCallback,w.LPARAM]
user.GetWindowThreadProcessId.argtypes=[w.HWND,ctypes.POINTER(w.DWORD)]
user.GetWindowTextW.argtypes=[w.HWND,w.LPWSTR,ctypes.c_int]
user.GetClassNameW.argtypes=[w.HWND,w.LPWSTR,ctypes.c_int]
user.IsWindowVisible.argtypes=[w.HWND]
user.GetWindowRect.argtypes=[w.HWND,ctypes.POINTER(w.RECT)]
user.GetWindowRgn.argtypes=[w.HWND,w.HANDLE]
user.GetDpiForSystem.restype=w.UINT
user.SetProcessDpiAwarenessContext.argtypes=[w.HANDLE]
user.SetProcessDpiAwarenessContext(w.HANDLE(-4)) # Match the physical desktop region coordinates.
gdi=ctypes.WinDLL('gdi32',use_last_error=True)
gdi.CreateRectRgn.argtypes=[ctypes.c_int]*4
gdi.CreateRectRgn.restype=w.HANDLE
gdi.PtInRegion.argtypes=[w.HANDLE,ctypes.c_int,ctypes.c_int]
gdi.DeleteObject.argtypes=[w.HANDLE]
class Process(ctypes.Structure):
    _fields_=[('size',w.DWORD),('usage',w.DWORD),('pid',w.DWORD),('heap',ctypes.c_size_t),('module',w.DWORD),('threads',w.DWORD),('parent',w.DWORD),('priority',w.LONG),('flags',w.DWORD),('name',w.WCHAR*260)]
class Module(ctypes.Structure):
    _fields_=[('size',w.DWORD),('id',w.DWORD),('pid',w.DWORD),('globalUse',w.DWORD),('use',w.DWORD),('base',ctypes.c_void_p),('bytes',w.DWORD),('handle',w.HMODULE),('name',w.WCHAR*256),('path',w.WCHAR*260)]
for name, record in [('Process32FirstW',Process),('Process32NextW',Process),('Module32FirstW',Module),('Module32NextW',Module)]:
    fn=getattr(kernel,name);fn.argtypes=[w.HANDLE,ctypes.POINTER(record)];fn.restype=w.BOOL

def snapshot(flag,pid,record,prefix):
    handle=kernel.CreateToolhelp32Snapshot(flag,pid)
    if handle==ctypes.c_void_p(-1).value: return []
    try:
        row=record();row.size=ctypes.sizeof(row);items=[]
        more=getattr(kernel,prefix+'FirstW')(handle,ctypes.byref(row))
        while more:
            items.append({name:getattr(row,name) for name in ('pid','name')+(('parent',) if record is Process else ())})
            more=getattr(kernel,prefix+'NextW')(handle,ctypes.byref(row))
        return items
    finally: kernel.CloseHandle(handle)

def children(app):
    return [p for p in snapshot(2,0,Process,'Process32') if p['parent']==app.process.pid]

def widget_windows(pid):
    windows=[]
    @WindowCallback
    def collect(hwnd,_):
        owner=w.DWORD()
        user.GetWindowThreadProcessId(hwnd,ctypes.byref(owner))
        if owner.value==pid and user.IsWindowVisible(hwnd):
            title=ctypes.create_unicode_buffer(256)
            user.GetWindowTextW(hwnd,title,len(title))
            if title.value == 'DotIsle Allegro Canvas': windows.append(hwnd)
        return True
    user.EnumWindows(collect,0)
    return windows


def widget_coverage(pid, points):
    # The current host batches widgets in one real HWND per occupied display.
    # Probe its actual input region so a blank canvas or missing slot cannot pass.
    covered=[False]*len(points)
    for hwnd in widget_windows(pid):
        bounds=w.RECT()
        region=gdi.CreateRectRgn(0,0,0,0)
        if not region:
            continue
        try:
            if user.GetWindowRect(hwnd,ctypes.byref(bounds)) and user.GetWindowRgn(hwnd,region)>1:
                for i,(x,y) in enumerate(points):
                    covered[i] |= bool(gdi.PtInRegion(region,x-bounds.left,y-bounds.top))
        finally:
            gdi.DeleteObject(region)
    return covered


def real_center_window(pid):
    matches=[]
    @WindowCallback
    def collect(hwnd,_):
        owner=w.DWORD()
        user.GetWindowThreadProcessId(hwnd,ctypes.byref(owner))
        if owner.value==pid and user.IsWindowVisible(hwnd):
            title=ctypes.create_unicode_buffer(256)
            cls=ctypes.create_unicode_buffer(256)
            user.GetWindowTextW(hwnd,title,len(title))
            user.GetClassNameW(hwnd,cls,len(cls))
            if title.value.startswith('点屿 · 控制中心') and cls.value and cls.value!='#32770':
                matches.append(hwnd)
        return True
    user.EnumWindows(collect,0)
    return bool(matches)

def wait(predicate,label,seconds=35):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        result=predicate()
        if result: return result
        time.sleep(.1)
    raise AssertionError('Timed out: '+label)

_WORKER_CACHE={}

def is_lyrics_worker(pid):
    if pid not in _WORKER_CACHE:
        try:
            cmd=subprocess.check_output(
                ['powershell','-NoProfile','-Command',f'(Get-CimInstance Win32_Process -Filter "ProcessId={pid}").CommandLine'],
                encoding='utf-8',errors='replace',creationflags=subprocess.CREATE_NO_WINDOW,timeout=5)
            _WORKER_CACHE[pid]='--lyrics-worker' in cmd
        except Exception:
            _WORKER_CACHE[pid]=False
    return _WORKER_CACHE[pid]

def center_children(app):
    return [p for p in children(app) if p['name']=='LyricIsland.Center.exe' and not is_lyrics_worker(p['pid'])]

def host(app):
    centers=center_children(app)
    if len(centers)!=1: return None
    pid=centers[0]['pid']
    names={m['name'].lower() for m in snapshot(0x18,pid,Module,'Module32')}
    return pid if 'dotisle_desktop.dll' in names else None

def stop(app):
    if app.process is not None and app.process.poll() is None:
        assert app.request('prepareUpdateExit')['success']
        app.process.wait(timeout=20)
    app.stop()

def run(args):
    app=Runtime(args.runtime,args.scratch/'data')
    report={'success':False,'checks':[]}
    try:
        assert not (app.runtime/'LyricIsland.Settings.exe').exists()
        assert not (app.runtime/'dotisle_settings.dll').exists()
        assert (app.runtime/'dotisle_desktop.dll').is_file()
        trace=app.data/'center-startup.json'
        app.data.mkdir(parents=True, exist_ok=True)
        app.write_settings()
        state=app.launch(env={'DOTISLE_STARTUP_TRACE':str(trace)});assert state['centerFrontend']=='go'
        # Overview tiles own their content: the cards must read data and keep their
        # settings without any desktop placement.
        for kind,config in (('weather',{'city':'厦门'}),('storage',{'drive':'C:'}),('worldClock',{'city':'东京'}),('note',{'text':'概览便签'})):
            reply=app.request('overviewWidgetSet',{'kind':kind,'config':config})
            assert reply['success'],reply
        state=app.request('state')
        assert {item['kind'] for item in state['overviewWidgets']}=={'weather','storage','worldClock','note'}
        assert state['desktopWidgets']==[]
        storage=wait(lambda:(app.request('state')['widgetData'].get('overview-storage') or {}).get('free',0)>0,'storage reading for an overview tile')
        assert storage
        assert app.request('state')['widgetData']['overview-worldClock']['clock']
        assert app.request('state')['widgetData'].get('overview-weather') is not None
        report['checks'].append('overview tiles read weather, storage and world clock without desktop widgets')
        ids=[]
        scale=max(1.,user.GetDpiForSystem()/96.)
        step=round(360*scale)
        placements=[(48,80),(48+step,80),(48,80+step)]
        points=[(x+round(48*scale),y+round(48*scale)) for x,y in placements]
        for kind,(x,y) in zip(('focus','note','wireless'),placements):
            reply=app.request('widgetAdd',{'kind':kind,'size':'medium','style':'original'})
            assert reply['success'],reply;ids.append(reply['id'])
            assert app.request('widgetUpdate',{'id':reply['id'],'x':x,'y':y})['success']
        center=wait(lambda:host(app),'embedded desktop DLL in one Go center')
        wait(lambda:widget_coverage(center,points)==[True,True,True],
             'three widgets have input regions on the real desktop canvas')
        assert all(p['name']!='LyricIsland.Settings.exe' for p in children(app))
        report['checks'].append('one Go center loads desktop DLL; no legacy settings process')
        for size in ('small','large','medium'):
            assert app.request('widgetUpdate',{'id':ids[0],'size':size,'x':placements[0][0],'y':placements[0][1]})['success']
            assert next(v for v in app.request('state')['desktopWidgets'] if v['id']==ids[0])['size']==size
        assert app.request('widgetRemove',{'id':ids[-1]})['success']
        assert not any(v['id']==ids[-1] for v in app.request('state')['desktopWidgets'])
        wait(lambda:widget_coverage(center,points)==[True,True,False],'removed widget slot is removed from the canvas')
        assert app.request('settings',{'page':'overview'})['success']
        assert wait(lambda:host(app),'center reopen')==center
        def center_ready():
            try:
                return 'first-frame' in json.loads(trace.read_text(encoding='utf-8')) and real_center_window(center)
            except (OSError, ValueError):
                return False
        wait(center_ready, 'real control center window (not an error dialog)')
        report['checks'].append('the hardware-only control center reached its ready frame and became visible')
        dll=ctypes.CDLL(str(app.runtime/'dotisle_desktop.dll'))
        dll.dotisle_desktop_layout.argtypes=[ctypes.c_char_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t]
        dll.dotisle_desktop_layout.restype=ctypes.c_ssize_t
        out=ctypes.create_string_buffer(65536)
        n=dll.dotisle_desktop_layout(b'null',4,out,len(out));assert n>0
        layout=json.loads(out.raw[:n])['layout'];layout['enabled']=True
        layout['elements']=[{'element':1,'x':100,'y':28,'width':240}]
        assert app.request('update',{'key':'MusicCustomLayout','value':layout})['success']
        stop(app)
        assert not any(p['pid']==center for p in snapshot(2,0,Process,'Process32'))
        state=app.start(preserve_settings=True)
        assert state['settings']['MusicCustomLayout']==layout
        assert {p['id'] for p in state['desktopWidgets']}==set(ids[:2])
        assert {item['kind'] for item in state['overviewWidgets']}=={'weather','storage','worldClock','note'}
        center=wait(lambda:host(app),'persisted desktop host after restart')
        wait(lambda:widget_coverage(center,points)==[True,True,False],'persisted widget slots after restart')
        report['checks'].append('layout and widgets persist; normal exit releases center; restart restores host')
        # A crashed native process used to leave unresponsive widgets forever.
        app.process.terminate()
        app.process.wait(timeout=15)
        app.stop()
        wait(lambda:not any(p['pid']==center for p in snapshot(2,0,Process,'Process32')),
             'center exits after its native process terminates',seconds=15)
        assert not widget_windows(center)
        state=app.start(preserve_settings=True)
        assert {p['id'] for p in state['desktopWidgets']}==set(ids[:2])
        center=wait(lambda:host(app),'fresh host after unexpected native exit')
        wait(lambda:widget_coverage(center,points)==[True,True,False],'widgets restored from saved placements after crash')
        report['checks'].append('unexpected native exit releases widget windows and center; restart restores saved widgets')
        # A center from another portable runtime shares the default pipe; it must
        # stop instead of following this instance and opening a second window.
        other=(args.scratch/'other-data').resolve()
        other.mkdir(parents=True,exist_ok=True)
        # This fixture tests data-directory isolation after an explicit synthetic
        # agreement; first-use consent is exercised by its own acceptance run.
        shutil.copy2(app.data/'consent.json',other/'consent.json')
        environment=dict(os.environ,LYRICISLAND_TEST_PIPE=app.pipe.split('\\')[-1])
        stray=subprocess.Popen([str(app.runtime/'LyricIsland.Center.exe'),'--data-dir',str(other)],cwd=app.runtime,env=environment,
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
        wait(lambda:stray.poll() is not None,'center from another data directory exits',seconds=15)
        assert len(center_children(app))==1
        report['checks'].append('a stray center never joins another instance or opens a second window')
        report['success']=True
    except Exception as error:
        report['error']=str(error)
        raise
    finally:
        try:
            if 'stray' in locals() and stray.poll() is None:
                stray.terminate()
                stray.wait(timeout=10)
            stop(app)
        finally:
            args.report.parent.mkdir(parents=True,exist_ok=True)
            args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return report

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--runtime',type=Path,required=True);ap.add_argument('--scratch',type=Path,required=True);ap.add_argument('--report',type=Path,required=True)
    print(json.dumps(run(ap.parse_args()),ensure_ascii=False))
