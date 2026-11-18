"""Package DotIsle's existing Rust runtime and standard Inno Setup installer."""
from pathlib import Path
import argparse,hashlib,json,shutil,subprocess,zipfile
from pe_resources import embed_icon,update_resources

def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def write_runtime_inventory(runtime, version):
    manifest = runtime / 'Assets/RuntimeFiles.json'
    files = {p.relative_to(runtime).as_posix(): sha(p)
             for p in sorted(runtime.rglob('*')) if p.is_file() and p != manifest}
    manifest.write_text(json.dumps(dict(schemaVersion=1, product='DotIsle', version=version, files=files),
                                   ensure_ascii=False) + '\n', encoding='utf-8')

def archive(directory,path,prefix,excluded=()):
    with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for p in sorted(directory.rglob('*')):
            if p.is_file() and not set(p.relative_to(directory).parts).intersection(excluded):
                z.write(p,Path(prefix)/p.relative_to(directory))
    with zipfile.ZipFile(path) as z:
        assert z.testzip() is None
        assert all('..' not in Path(p).parts and 'Data' not in Path(p).parts for p in z.namelist())

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument('--binary-dir',type=Path,required=True)
    ap.add_argument('--output',type=Path,required=True)
    ap.add_argument('--work-dir',type=Path,required=True)
    ap.add_argument('--iscc',type=Path,required=True)
    ap.add_argument('--version',required=True)
    ap.add_argument('--shader-cache',type=Path,required=True)
    ap.add_argument('--qa-setup',action='store_true')
    args=ap.parse_args()
    root=Path(__file__).resolve().parents[1]
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    scratch=args.work_dir.resolve();scratch.mkdir(parents=True,exist_ok=True)
    # Reject a missing or stale Center before creating any runtime files.
    center=args.binary_dir/'LyricIsland.Center.exe'
    if not center.is_file() or center.stat().st_size == 0:
        raise RuntimeError('Build the Go control center before packaging')
    import struct
    pe=center.read_bytes()
    offset=struct.unpack_from('<I',pe,0x3c)[0]
    if struct.unpack_from('<H',pe,offset+24+68)[0] != 2:
        raise RuntimeError('Center must use the Windows GUI subsystem (-H=windowsgui)')
    source_center=root/'Go/settings-center/LyricIsland.Center.exe'
    if source_center.is_file() and sha(source_center) != sha(center):
        raise RuntimeError('Stale Center in binary-dir: copy the current Go build before packaging')
    name='DotIsle-'+args.version
    runtime=out/name;assert not runtime.exists(),'Use a fresh version directory'
    runtime.mkdir()
    binaries={'LyricIsland.exe':'LyricIsland.exe','lyric-updater.exe':'LyricIsland.Updater.exe','lyric-uninstall.exe':'Uninstall.exe'}
    for built,shipped in binaries.items():
        target=runtime/shipped
        shutil.copy2(args.binary_dir/built,target)
        embed_icon(target,root/'Assets/LyricIsland-v3.ico')
    # The MyGo control center ships beside LyricIsland.exe (show_settings_page
    # prefers it); its icon and manifest are embedded at link time via
    # rsrc_windows_amd64.syso, so no PE resource rewrite here.
    shutil.copy2(center,runtime/'LyricIsland.Center.exe')
    for module in ['dotisle_core.dll', 'dotisle_desktop.dll', 'dotisle_focus.dll', 'dotisle_todos.dll', 'dotisle_music.dll', 'dotisle_lyrics.dll', 'dotisle_notifications.dll', 'dotisle_history.dll', 'dotisle_automation.dll', 'dotisle_devices.dll', 'dotisle_osd.dll', 'dotisle_sound.dll', 'dotisle_widgets.dll', 'dotisle_themes.dll', 'dotisle_glow.dll']:
        shutil.copy2(args.binary_dir/module,runtime/module)
    shutil.copytree(root/'Assets',runtime/'Assets',ignore=shutil.ignore_patterns('ShaderCache'))
    shutil.copytree(args.shader_cache,runtime/'Assets/ShaderCache/Hybrid29')
    shutil.copytree(root/'licenses',runtime/'licenses')
    # Read-only SQLite reader for the notification listener and legacy song_maps.db import.
    shutil.copy2(root/'build/e_sqlite3.dll',runtime/'e_sqlite3.dll')
    for doc in ['README.md','GESTURE-TUTORIAL.md','NETEASE-SEEKFIX.md','CHANGELOG.md','LICENSE','FONT-NOTICE.md','THIRD-PARTY-NOTICES.md','REWRITE.md']:
        shutil.copy2(root/doc,runtime/doc)
    # Preserve relative wordmark references in the bundled README.
    shutil.copytree(root/'docs/brand',runtime/'docs/brand')
    shutil.copy2(root/'docs/PLUGIN-DEVELOPMENT.md',runtime/'docs/PLUGIN-DEVELOPMENT.md')
    shutil.copy2(root/'docs/UNIFIED-CONTROL-CENTER.md',runtime/'docs/UNIFIED-CONTROL-CENTER.md')
    shutil.copy2(root/'docs/DISCLAIMER.md',runtime/'docs/DISCLAIMER.md')
    shutil.copytree(root/'examples/plugins',runtime/'examples/plugins')
    shutil.copy2(out/(name+'-Notes.md'),runtime/'发布说明.md')
    # Ownership and original hashes let a later successful update retire only
    # obsolete distribution files; user-added or modified files remain intact.
    write_runtime_inventory(runtime, args.version)
    installer_stage=scratch/'installer-stage'
    shutil.copytree(runtime,installer_stage)
    payload=scratch/'InstallPayload.zip'
    archive(installer_stage,payload,'LyricIsland-'+args.version)
    engine=scratch/'DotIsle.Engine.exe'
    shutil.copy2(args.binary_dir/'lyric-setup.exe',engine)
    embed_icon(engine,root/'Assets/LyricIsland-v3.ico')
    update_resources(engine,[(10,'DotIsle.InstallPayload.zip',payload.read_bytes()),(10,'DotIsle.InstallPayload.sha256',sha(payload).encode('ascii'))])
    def compile_setup(destination,extra):
        destination.mkdir(parents=True,exist_ok=True)
        command=[str(args.iscc),'/Qp','/DAppVersion='+args.version,'/DStageDir='+str(installer_stage),'/DOutputDir='+str(destination),'/DEngineFile='+str(engine),*extra,str(root/'build/installer.iss')]
        with (scratch/('inno-qa.log' if extra else 'inno.log')).open('wb') as log:
            subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
    compile_setup(out,[])
    if args.qa_setup:
        compile_setup(scratch/'qa-setup',['/DAppId=DotIsle.QA.Release318','/DOutputBase='+name+'-QA-Setup','/DIncludeVerifyEngine=1'])
    (runtime/'Portable.json').write_text(json.dumps({'portable':True,'version':args.version})+'\n',encoding='utf8')
    write_runtime_inventory(runtime, args.version)
    archive(runtime,out/(name+'-Portable.zip'),name)
    legacy='LyricIsland-'+args.version
    archive(runtime,out/(legacy+'-Portable.zip'),legacy)
    (out/(legacy+'-SHA256.txt')).write_text(sha(out/(legacy+'-Portable.zip'))+'  '+legacy+'-Portable.zip\n',encoding='ascii')
    report={'version':args.version,'runtime':str(runtime),'setup':str(out/(name+'-Setup.exe')),'engine':str(engine),'payloadSha256':sha(payload),'binaries':{p.name:sha(p) for p in runtime.glob('*.exe')}}
    (scratch/'package.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8')
    print(json.dumps(report,ensure_ascii=False,indent=2))

if __name__=='__main__': main()
