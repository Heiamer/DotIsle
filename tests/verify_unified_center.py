"""Real packaged runtime: Go owns the center and embeds desktop widgets in-process."""
import argparse
import ctypes
from ctypes import wintypes as w
import json
import os
import subprocess
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
user.IsWindowVisible.argtypes=[w.HWND]
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
            if title.value.startswith('点屿桌面组件 · '): windows.append(title.value)
        return True
    user.EnumWindows(collect,0)
    return windows

def wait(predicate,label,seconds=35):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        result=predicate()
        if result: return result
        time.sleep(.1)
    raise AssertionError('Timed out: '+label)

def host(app):
    centers=[p for p in children(app) if p['name']=='LyricIsland.Center.exe']
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
        state=app.start();assert state['centerFrontend']=='go'
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
        for kind in ('focus','note','wireless'):
            reply=app.request('widgetAdd',{'kind':kind,'size':'medium','style':'original'})
            assert reply['success'],reply;ids.append(reply['id'])
        center=wait(lambda:host(app),'embedded desktop DLL in one Go center')
        wait(lambda:len(widget_windows(center))==3,'three real desktop widget windows')
        assert all(p['name']!='LyricIsland.Settings.exe' for p in children(app))
        report['checks'].append('one Go center loads desktop DLL; no legacy settings process')
        for size in ('small','large','medium'):
            assert app.request('widgetUpdate',{'id':ids[0],'size':size,'x':200,'y':450})['success']
            assert next(v for v in app.request('state')['desktopWidgets'] if v['id']==ids[0])['size']==size
        assert app.request('widgetRemove',{'id':ids[-1]})['success']
        assert not any(v['id']==ids[-1] for v in app.request('state')['desktopWidgets'])
        wait(lambda:len(widget_windows(center))==2,'removed widget window is destroyed')
        assert app.request('settings',{'page':'overview'})['success']
        assert wait(lambda:host(app),'center reopen')==center
        report['checks'].append('add, resize, remove and reopen preserve the same host process')
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
        wait(lambda:len(widget_windows(center))==2,'persisted widget windows after restart')
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
        wait(lambda:len(widget_windows(center))==2,'widgets restored from saved placements after crash')
        report['checks'].append('unexpected native exit releases widget windows and center; restart restores saved widgets')
        # A center from another portable runtime shares the default pipe; it must
        # stop instead of following this instance and opening a second window.
        other=args.scratch/'other-data'
        other.mkdir(parents=True,exist_ok=True)
        environment=dict(os.environ,LYRICISLAND_TEST_PIPE=app.pipe.split('\\')[-1])
        stray=subprocess.Popen([str(app.runtime/'LyricIsland.Center.exe'),'--data-dir',str(other)],cwd=app.runtime,env=environment,
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
        wait(lambda:stray.poll() is not None,'center from another data directory exits',seconds=15)
        assert len([p for p in children(app) if p['name']=='LyricIsland.Center.exe'])==1
        report['checks'].append('a stray center never joins another instance or opens a second window')
        report['success']=True
    except Exception as error:
        report['error']=str(error)
        raise
    finally:
        try: stop(app)
        finally:
            args.report.parent.mkdir(parents=True,exist_ok=True)
            args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return report

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--runtime',type=Path,required=True);ap.add_argument('--scratch',type=Path,required=True);ap.add_argument('--report',type=Path,required=True)
    print(json.dumps(run(ap.parse_args()),ensure_ascii=False))
