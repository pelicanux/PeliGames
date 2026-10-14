"""Root-side verified staging for the official MangoHUD installer."""
import hashlib
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile

LIMIT = 256 * 1024 * 1024

def extract_package(archive, destination):
    total = 0
    with tarfile.open(archive, 'r:*') as package:
        entries = package.getmembers()
        if len(entries) > 2000:
            raise ValueError('Pacote contém arquivos demais.')
        for member in entries:
            path = PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or not (member.isdir() or member.isfile()):
                raise ValueError('Pacote contém um caminho ou tipo de arquivo inseguro.')
            total += member.size
            if member.size < 0 or total > LIMIT:
                raise ValueError('Pacote maior que o permitido.')
            target = destination.joinpath(*path.parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with package.extractfile(member) as source, target.open('wb') as output:
                    shutil.copyfileobj(source, output)
                target.chmod(0o644)

def prepare(archive, digest, size, stage):
    copied = stage / 'official.tar.gz'
    hashed = hashlib.sha256()
    count = 0
    # Copy into a root-owned directory before verification or execution.
    with open(archive, 'rb') as source, copied.open('xb') as output:
        while True:
            chunk = source.read(1024 * 1024)
            if not chunk:
                break
            count += len(chunk)
            if count > size or count > LIMIT:
                raise ValueError('Tamanho do pacote inválido.')
            hashed.update(chunk)
            output.write(chunk)
    if count != size or hashed.hexdigest() != digest:
        raise ValueError('A integridade do pacote não pôde ser confirmada.')
    extracted = stage / 'extracted'
    extracted.mkdir()
    extract_package(copied, extracted)
    scripts = list(extracted.rglob('mangohud-setup.sh'))
    if len(scripts) != 1:
        raise ValueError('Instalador oficial não encontrado no pacote.')
    script = scripts[0]
    inner = script.parent / 'MangoHud-package.tar'
    if not inner.is_file():
        raise ValueError('Conteúdo de instalação não encontrado.')
    # The upstream root branch expects this payload to have already been unpacked.
    extract_package(inner, script.parent)
    expected = {}
    for relative in ('usr/bin/mangohud', 'usr/lib/mangohud/lib64/libMangoHud.so', 'usr/lib/mangohud/lib32/libMangoHud.so'):
        file = script.parent / relative
        if not file.is_file():
            raise ValueError('Pacote oficial incompleto.')
        expected['/' + relative] = hashlib.sha256(file.read_bytes()).hexdigest()
    return script, expected

def main():
    if os.geteuid() != 0:
        raise RuntimeError('É necessário autorizar a instalação como administrador.')
    archive, digest, raw_size = sys.argv[1:]
    if len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError('Checksum inválido.')
    size = int(raw_size)
    if not 0 < size <= LIMIT:
        raise ValueError('Tamanho inválido.')
    with tempfile.TemporaryDirectory(prefix='peligames-mangohud-install-', dir='/tmp') as root:
        stage = Path(root)
        script, expected = prepare(archive, digest, size, stage)
        home = stage / 'home'
        home.mkdir()
        env = {'PATH':'/usr/sbin:/usr/bin:/sbin:/bin', 'HOME':str(home), 'XDG_CONFIG_HOME':str(home / '.config'), 'LANG':'C.UTF-8'}
        result = subprocess.run(['/bin/bash', str(script), 'install'], cwd=script.parent, env=env, check=False)
        if result.returncode != 0:
            raise RuntimeError('O instalador oficial não concluiu a instalação.')
        for file, checksum in expected.items():
            if not Path(file).is_file() or hashlib.sha256(Path(file).read_bytes()).hexdigest() != checksum:
                raise RuntimeError('O instalador não instalou todos os arquivos esperados.')
        if not os.access('/usr/bin/mangohud', os.X_OK):
            raise RuntimeError('O executável MangoHUD não ficou disponível.')

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print('PeliGames: ' + str(error), file=sys.stderr)
        sys.exit(1)
