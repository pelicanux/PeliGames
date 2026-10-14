#!/usr/bin/env python3
"""Two local sessions; ephemeral credentials; loopback-only LiveKit server."""
import argparse, base64, hashlib, hmac, io, json, os, pathlib, platform, secrets, socket, subprocess, tarfile, tempfile, time, urllib.request
ROOT = pathlib.Path(__file__).resolve().parent.parent
VERSION = '1.13.9'
SHA256 = '0b7fa208b662d09cfdeae8c06cf4c481aead0556086558b48250501e2e2d6e20'

def server_binary():
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'amd64'):
        raise RuntimeError('Este pacote de teste requer Linux x86_64.')
    cache = ROOT / 'src-tauri/target/rtc-test-server'
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / f'livekit-{VERSION}.tar.gz'
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        print('Baixando servidor LiveKit oficial para o teste local…', flush=True)
        url = f'https://github.com/livekit/livekit/releases/download/v{VERSION}/livekit_{VERSION}_linux_amd64.tar.gz'
        with urllib.request.urlopen(url, timeout=120) as response:
            data = response.read(100 * 1024 * 1024 + 1)
        if hashlib.sha256(data).hexdigest() != SHA256:
            raise RuntimeError('Checksum do LiveKit inválido.')
        archive.write_bytes(data)
    binary = cache / 'livekit-server'
    with tarfile.open(archive, 'r:gz') as tar:
        members = [m for m in tar.getmembers() if pathlib.PurePosixPath(m.name).name == 'livekit-server' and m.isfile()]
        if len(members) != 1: raise RuntimeError('Pacote LiveKit inválido.')
        with tar.extractfile(members[0]) as src: binary.write_bytes(src.read())
    binary.chmod(0o700)
    return binary

def token(key, secret, identity):
    encode = lambda obj: base64.urlsafe_b64encode(json.dumps(obj, separators=(',', ':')).encode()).rstrip(b'=')
    now = int(time.time())
    payload = {'iss': key, 'sub': identity, 'name': 'Sessão ' + identity.upper(), 'nbf': now - 5, 'exp': now + 3600,
               'video': {'roomJoin': True, 'room': 'peligames-local-audio', 'canPublish': True, 'canSubscribe': True, 'canPublishData': False}}
    body = encode({'alg': 'HS256', 'typ': 'JWT'}) + b'.' + encode(payload)
    return (body + b'.' + base64.urlsafe_b64encode(hmac.new(secret.encode(), body, hashlib.sha256).digest()).rstrip(b'=')).decode()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', help='Executável de testes Rust para validar mídia sintética sem microfone')
    args = parser.parse_args()
    binary = server_binary()
    for port, kind in [(17880, socket.SOCK_STREAM), (17881, socket.SOCK_STREAM), (17882, socket.SOCK_DGRAM)]:
        with socket.socket(socket.AF_INET, kind) as check:
            check.bind(('127.0.0.1', port))
    processes = []
    with tempfile.TemporaryDirectory(prefix='peligames-call-') as folder:
        folder = pathlib.Path(folder)
        key, secret = secrets.token_hex(12), secrets.token_hex(32)
        config = folder / 'server.yaml'
        config.write_text(f'port: 17880\nbind_addresses: ["127.0.0.1"]\nrtc:\n  tcp_port: 17881\n  udp_port: 17882\n  node_ip: 127.0.0.1\n  use_external_ip: false\n  enable_loopback_candidate: true\nkeys:\n  {key}: {secret}\n')
        config.chmod(0o600)
        credentials = folder / 'sessions.json'
        credentials.write_text(json.dumps({'url': 'ws://127.0.0.1:17880', 'a': token(key, secret, 'a'), 'b': token(key, secret, 'b')}))
        credentials.chmod(0o600)
        log = open(folder / 'server.log', 'w+')
        try:
            server = subprocess.Popen([str(binary), '--config', str(config)], stdout=log, stderr=log)
            processes.append(server)
            for _ in range(100):
                if server.poll() is not None:
                    raise RuntimeError('O servidor local não iniciou. Verifique as portas 17880–17882.')
                try:
                    with socket.create_connection(('127.0.0.1',17880), timeout=.2): break
                except OSError: time.sleep(.1)
            else: raise RuntimeError('O servidor local não respondeu.')
            env = dict(os.environ, PELIGAMES_RTC_TEST_CONFIG=str(credentials), PELIGAMES_RTC_TEST_ID='a')
            if args.check:
                result = subprocess.run([args.check, 'commands::matrix_call::tests::two_sessions_exchange_audio', '--ignored', '--exact', '--nocapture'], env=env, timeout=60)
                return result.returncode
            app = ROOT / 'Release/peligames'
            print('Abrindo duas sessões. Clique em Entrar na chamada nas duas. Use fones e ative apenas um microfone por vez.', flush=True)
            for identity in ('a','b'):
                processes.append(subprocess.Popen([str(app), '--matrix-rtc-test'], env=dict(env, PELIGAMES_RTC_TEST_ID=identity)))
            while any(p.poll() is None for p in processes[1:]): time.sleep(.25)
            return 0
        finally:
            for p in reversed(processes):
                if p.poll() is None:
                    p.terminate()
                    try: p.wait(timeout=5)
                    except subprocess.TimeoutExpired: p.kill(); p.wait()
            log.close()
if __name__ == '__main__':
    try: raise SystemExit(main())
    except KeyboardInterrupt: raise SystemExit(130)
    except Exception as error: print(f'Teste de chamada: {error}'); raise SystemExit(1)
