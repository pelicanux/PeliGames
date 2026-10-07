from pathlib import Path
import os, subprocess, json, tempfile, shutil, time
root = Path(tempfile.mkdtemp(prefix='pelinstall-integration-', dir='/tmp'))
service = Path.cwd() / 'tools/proton-service/target/debug/peligames-proton-service'
env = dict(os.environ, XDG_CONFIG_HOME=str(root/'config'), PATH=str(root/'bin')+':'+os.environ['PATH'], PELI_TEST_EXECUTABLE=str(root/'invoked.txt'))
try:
    (root/'bin').mkdir(); runner=root/'Runner com espaço'; runner.mkdir()
    (runner/'proton').write_text('#!/bin/sh\nexit 0\n'); (runner/'proton').chmod(0o755)
    installer=root/'setup com espaço & $nome.exe'; installer.write_bytes(b'MZfixture')
    umu=root/'bin/umu-run'
    umu.write_text('''#!/usr/bin/env bash
set -e
printf "%s" "$1" > "$PELI_TEST_EXECUTABLE"
case "${PELI_TEST_MODE:-install}" in
 install) mkdir -p "$WINEPREFIX/drive_c/Jogo"; printf MZfixture > "$WINEPREFIX/drive_c/Jogo/game & $nome %f.exe"; printf REGEDIT4 > "$WINEPREFIX/system.reg" ;;
 handoff) bash -c 'exec -a "ChildGame.exe" sleep 30' ;;
 background) bash -c 'exec -a "$1" sleep 30' -- "$1" ;;
 success) bash -c 'exec -a "$1" sleep 1.5' -- "$1" ;;
 warning) printf 'fixme: optional warning\\n'; bash -c 'exec -a "$1" sleep 1.5' -- "$1" ;;
 fail) printf 'wine: could not load kernel32.dll\\n' >&2; exit 7 ;;
 proton_fatal) printf 'unhandled exception\\n' > "$PROTON_LOG_DIR/steam-fixture.log"; bash -c 'exec -a "$1" sleep 1.5' -- "$1" ;;
 quick) exit 0 ;;
esac
'''); umu.chmod(0o755)
    req={'name':'Meu jogo','directory':str(root/'Meu jogo'),'executable':str(installer),'proton':str(runner)}
    def call(command, *args, mode='install', success=True):
        p=subprocess.run([str(service),command,*args],env=dict(env,PELI_TEST_MODE=mode),capture_output=True,text=True,timeout=15)
        data=json.loads(p.stdout.strip().splitlines()[-1])
        assert (p.returncode==0)==success,(command,p.returncode,data,p.stderr)
        return data
    info=call('pelinstall-info',str(installer))['result']; assert info['executable']==str(installer) and not info['error']
    result=call('run-game',json.dumps(req))['result']; assert result['registered_count']==1,result
    entry=call('list-games')['result'][0]; assert entry['prefix']==str(root/'Meu jogo/prefix')
    assert (root/'Meu jogo/logs').is_dir(); assert (root/'Meu jogo/installation.json').is_file()
    assert call('pelinstall-matches',str(installer))['result'][0]['entry']['path']==entry['path']
    assert call('pelinstall-matches',entry['executable'])['result'][0]['entry']['path']==entry['path']
    unrelated=root/'unrelated'/installer.name; unrelated.parent.mkdir(); unrelated.write_bytes(b'MZfixture')
    assert call('pelinstall-matches',str(unrelated))['result']==[], 'Names alone must not identify an installation'
    repair={'path':entry['path'],'name':'Jogo reparado','prefix':entry['prefix'],'proton':str(runner),'executable':entry['executable']}
    repaired=call('repair-game',json.dumps(repair))['result']; assert repaired['registered_count']==1,repaired
    assert (root/'invoked.txt').read_text()==str(installer), 'Same Proton repair must not run UMU or the executable'
    assert repaired['log']=='', 'Same Proton repair should complete without creating a launch log'
    entries=call('list-games')['result']; assert len(entries)==1 and entries[0]['path']==entry['path'] and entries[0]['name']=='Jogo reparado'
    alternate=Path(entry['executable']).with_name('Alternative.exe'); alternate.write_bytes(b'MZfixture')
    call('repair-game',json.dumps(dict(repair,executable=str(alternate))))
    assert call('list-games')['result'][0]['executable']==str(alternate), 'Repair must persist the selected executable'
    call('repair-game',json.dumps(repair))
    manifest_path=root/'Meu jogo/installation.json'; before=manifest_path.read_bytes()
    newrunner=root/'Outro Proton'; newrunner.mkdir(); (newrunner/'proton').write_text('#!/bin/sh\nexit 0\n'); (newrunner/'proton').chmod(0o755)
    call('repair-game',json.dumps(dict(repair,name='Falha não salva',proton=str(newrunner))),mode='fail',success=False)
    assert manifest_path.read_bytes()==before, 'Failed repairs must preserve the registration'
    updated=call('repair-game',json.dumps(dict(repair,proton=str(newrunner))))['result']
    assert updated['registered_count']==1 and Path(updated['log']).is_file()
    assert (root/'invoked.txt').read_text()=='wineboot', 'Changed Proton must only initialize/update the prefix'
    assert call('list-games')['result'][0]['proton']==str(newrunner)
    newprefix=root/'Outro prefixo'; (newprefix/'drive_c').mkdir(parents=True); (newprefix/'system.reg').write_text('REGEDIT4')
    moved=call('repair-game',json.dumps(dict(repair,prefix=str(newprefix))))['result']; assert moved['prefix']==str(newprefix)
    entry=call('list-games','true')['result'][0]; assert entry['prefix']==str(newprefix) and entry['executable'].startswith(str(newprefix))
    wrapper=root/'my wrapper.sh'; marker=root/'wrapper-ran.txt'
    wrapper.write_text('#!/bin/sh\nprintf wrapper > "'+str(marker)+'"\nexec "$@"\n')
    call('update-game',json.dumps(dict(path=entry['path'],name=entry['name'],proton=entry['proton'],executable=entry['executable'],prefix=entry['prefix'],advanced=dict(schema_version=2,wrappers=[dict(program='/bin/sh',arguments='"'+str(wrapper)+'"',enabled=True)]))))
    assert call('launch-shortcut',entry['path'],mode='success')['result']['state']=='exited'
    assert marker.read_text()=='wrapper', 'Custom wrapper must execute before the game command'
    first_log=call('game-logs',entry['path'])['result']; assert 'Executable:' in first_log['current'] and first_log['previous']==''
    assert call('launch-shortcut',entry['path'],mode='warning')['result']['state']=='exited'
    retained=call('game-logs',entry['path'])['result']; assert retained['previous']==first_log['current']
    import hashlib
    logs_root=root/'config/peligames/game-logs'/hashlib.sha256(entry['path'].encode()).hexdigest()
    assert {p.name for p in logs_root.iterdir()}=={'current','previous'}
    assert call('game-logs','../unregistered',success=False)['error']
    failure=call('launch-shortcut',entry['path'],mode='fail',success=False); assert '7' in failure['error']
    report=json.loads(next((root/'config/peligames/launch-reports').glob('*.json')).read_text()); assert report['entry']==entry['path'] and Path(report['log']).is_file()
    assert call('launch-shortcut',entry['path'],mode='proton_fatal')['result']['state']=='exited', 'Log keywords must not override a successful exit'
    assert call('launch-shortcut',entry['path'],mode='quick')['result']['state']=='exited', 'A fast successful close is normal'
    # A fresh launcher process discovers the headless monitor and can stop it.
    monitor=subprocess.Popen([str(service),'monitor-game',entry['path']],env=dict(env,PELI_TEST_MODE='background'),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    outsider=subprocess.Popen(['sleep','30'])
    try:
        started=json.loads(monitor.stdout.readline())['execution']
        for _ in range(60):
            shared=call('execution-status')['result']
            if shared and shared['state']=='running': break
            time.sleep(.05)
        assert shared['state']=='running' and shared['path']==entry['path'], shared
        assert call('game-logs',entry['path'])['result']['current'], 'Logs must be readable while running'
        assert call('cancel-execution','../invalid',success=False)['error']
        assert call('cancel-execution',shared['id'])['result']['state']=='stopping'
        output,_=monitor.communicate(timeout=10)
        assert monitor.returncode==0 and 'cancelled' in output, output
        assert call('execution-status')['result'] is None
        assert outsider.poll() is None, 'Stopping a shared session must preserve unrelated processes'
    finally:
        if monitor.poll() is None: monitor.kill();monitor.wait()
        outsider.terminate();outsider.wait()
    handoff=subprocess.Popen([str(service),'monitor-game',entry['path']],env=dict(env,PELI_TEST_MODE='handoff'),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        initial=json.loads(handoff.stdout.readline())['execution']
        for _ in range(60):
            state=call('execution-status')['result']
            if state and state['state']=='running': break
            time.sleep(.05)
        assert state['state']=='running', 'A launcher handing off to another EXE must be recognized as running'
        call('cancel-execution',state['id']);out,_=handoff.communicate(timeout=10);assert 'cancelled' in out
    finally:
        if handoff.poll() is None: handoff.kill();handoff.wait()
    # No display server: successful native shortcuts must not initialize a GUI.
    native_env=dict(env, PELI_TEST_MODE='success', APPIMAGE_EXTRACT_AND_RUN='1')
    native_binaries=['PeliGames','Pelinstall']
    appimages=list((Path.cwd()/'Release').glob('PeliGames_*.AppImage'))
    if len(appimages)==1: native_binaries.append(appimages[0].name)
    native_env.pop('DISPLAY',None); native_env.pop('WAYLAND_DISPLAY',None)
    for binary in native_binaries:
        executable=Path.cwd()/'Release'/binary
        if executable.is_file():
            ui=subprocess.run([str(executable),'--verify-ui'],env=native_env,capture_output=True,text=True,timeout=15)
            assert ui.returncode==0,(binary,ui.stderr,ui.stdout)
            ui_info=json.loads(ui.stdout)
            assert ui_info['production'] and ui_info['embedded_ui'],(binary,ui_info)
            native=subprocess.run([str(executable),'--launch-game',entry['path']],env=native_env,capture_output=True,text=True,timeout=15)
            assert native.returncode==0,(binary,native.stderr,native.stdout)
    assert not list((root/'config/peligames/launch-reports').glob('*.json'))
    portable = root/'programa existente.exe'
    portable.write_bytes(b'MZportable-fixture')
    added = call('add-game', json.dumps({'name':'Programa existente', 'directory':str(root/'Programa existente'), 'executable':str(portable), 'proton':str(runner)}))['result']
    assert added['executable'] == str(portable) and added['proton'] == str(runner)
    manifest = json.loads((root/'Programa existente/installation.json').read_text())
    assert manifest['installer'] == '' and Path(added['prefix']).is_dir()
    assert not (Path(added['prefix'])/'drive_c').exists(), 'Adicionar não deve executar o UMU/instalador'
    assert portable.read_bytes() == b'MZportable-fixture'
    duplicate_request = {'name':'Programa existente', 'directory':str(root/'Programa existente'), 'executable':str(portable), 'proton':str(runner)}
    duplicate = call('add-game', json.dumps(duplicate_request))['result']
    matches=call('pelinstall-matches',str(portable))['result']; assert {m['entry']['path'] for m in matches}=={added['path'],duplicate['path']}
    assert duplicate['path'] != added['path'] and duplicate['prefix'] == added['prefix']
    assert call('launch-shortcut', duplicate['path'], mode='success')['result']['state'] == 'exited'
    listed = call('list-games', 'true')['result']
    assert len([item for item in listed if item['executable'] == str(portable)]) == 2
    for binary in native_binaries:
        native = subprocess.run([str(Path.cwd()/'Release'/binary), '--launch-game', duplicate['path']], env=native_env, capture_output=True, text=True, timeout=15)
        assert native.returncode == 0, (binary, native.stderr, native.stdout)
    # The actual native headless launcher publishes the same session to a later process.
    background=subprocess.Popen([str(Path.cwd()/'Release/PeliGames'),'--launch-game',duplicate['path']],env=dict(native_env,PELI_TEST_MODE='background'),stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        for _ in range(80):
            status=call('execution-status')['result']
            if status and status['state']=='running': break
            assert background.poll() is None, 'Native monitor exited before publishing a running session'
            time.sleep(.05)
        assert status['state']=='running' and status['path']==duplicate['path'],status
        repeated=subprocess.run([str(Path.cwd()/'Release/PeliGames'),'--launch-game',duplicate['path']],env=dict(native_env,PELI_TEST_MODE='background'),capture_output=True,text=True,timeout=5)
        assert repeated.returncode==0,(repeated.stdout,repeated.stderr)
        assert call('execution-status')['result']['id']==status['id'], 'Opening the shortcut again must reuse the same monitor'
        call('cancel-execution',status['id'])
        out,err=background.communicate(timeout=10)
        assert background.returncode==0,(out,err)
        assert call('execution-status')['result'] is None
        assert not list((root/'config/peligames/launch-reports').glob('*.json')), 'User-requested stop must not generate an error dialog'
    finally:
        if background.poll() is None: background.kill();background.wait()
    # Two simultaneous desktop invocations share a single session even before publication.
    racers=[subprocess.Popen([str(Path.cwd()/'Release/PeliGames'),'--launch-game',duplicate['path']],env=dict(native_env,PELI_TEST_MODE='background'),stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True) for _ in range(2)]
    try:
        for _ in range(80):
            state=call('execution-status')['result']
            if state and state['state']=='running': break
            time.sleep(.05)
        assert state and state['state']=='running',state
        assert len(list((root/'config/peligames/executions').glob('*.json')))==1
        call('cancel-execution',state['id'])
        for racer in racers:
            out,err=racer.communicate(timeout=10);assert racer.returncode==0,(out,err)
        assert not list((root/'config/peligames/launch-reports').glob('*.json'))
    finally:
        for racer in racers:
            if racer.poll() is None: racer.kill();racer.wait()
    print('Pelinstall: lookup by executable/installer, successful and failed repair, prefix changes, duplicate registration in a shared prefix, rescans, cross-process detection/cancellation, monitored launches and native shortcuts without a display passed.')
finally: shutil.rmtree(root)
