# Original PeliGames implementation using the public UDisks2 D-Bus API.
import json, os, re, subprocess, sys, time
from gi.repository import Gio, GLib
U = 'org.freedesktop.UDisks2'
B, F, P, T = [U + '.' + name for name in ('Block', 'Filesystem', 'Partition', 'PartitionTable')]
bus = None

def call(path, interface, method, signature, values):
    global bus
    if bus is None: bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
    return bus.call_sync(U, path, interface, method, GLib.Variant(signature, values), None,
                         Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, 600000 if method == "Format" else 120000, None)

def objects():
    return call('/org/freedesktop/UDisks2', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects', '()', ()).unpack()[0]

def text(value):
    return bytes(value).rstrip(b'\0').decode('utf-8', 'replace')

def fstab_items(block):
    return [entry for entry in block.get('Configuration', []) if entry[0] == 'fstab']

# Generic data partitions only; EFI, recovery, reserved and discoverable
# root/home/boot partition types must stay protected even while unmounted.
DATA_PARTITION_TYPES = {'ebd0a0a2-b9e5-4433-87c0-68b6b72699c7', '0fc63daf-8483-4772-8e79-3d69d8477de4'}

def system_partition(obj, all_objects):
    b, p = obj.get(B, {}), obj.get(P, {})
    peers = [obj] + [other for other in all_objects.values()
                    if b.get('IdUUID') and other is not obj and other.get(B, {}).get('IdUUID') == b['IdUUID']]
    # Btrfs subvolumes can expose the same filesystem through multiple objects.
    for other in peers:
        mounts = [text(x) for x in other.get(F, {}).get('MountPoints', [])]
        configured = [text(x[1]['dir']) for x in fstab_items(other.get(B, {}))]
        if any(not x.startswith(('/mnt/', '/media/', '/run/media/')) for x in mounts + configured):
            return True
    table = all_objects.get(p.get('Table'), {}).get(T, {}).get('Type')
    if table == 'gpt' and p.get('Type') and p['Type'].lower() not in DATA_PARTITION_TYPES:
        return True
    return bool(b.get('HintIgnore') or p.get('IsContainer') or p.get('Flags', 0) & 1)

def storage_candidate(obj, all_objects):
    b = obj.get(B, {})
    if B not in obj or system_partition(obj, all_objects):
        return False
    if F in obj and b.get('IdUsage') == 'filesystem' and b.get('IdType'):
        return b.get('IdType') in ('ext4', 'btrfs', 'ntfs', 'ntfs3')
    # Unformatted physical partitions/disks have no Filesystem interface or UUID.
    # A whole disk with children is represented by its partitions, never twice.
    if b.get('IdUsage', '') not in ('', 'filesystem', 'partitiontable') or b.get('IdType', '') not in ('', 'gpt', 'dos'):
        return False
    if b.get('Drive', '/') == '/' or b.get('CryptoBackingDevice', '/') != '/':
        return False
    paths = {path for path, value in all_objects.items() if value is obj}
    return not any(other.get(P, {}).get('Table') in paths for other in all_objects.values())

def reason(obj, all_objects, allow_ntfs=False):
    b, p, f = obj.get(B, {}), obj.get(P, {}), obj.get(F, {})
    if system_partition(obj, all_objects):
        return 'Partição usada ou reservada pelo sistema; alterações bloqueadas.'
    paths = {path for path, value in all_objects.items() if b.get('IdUUID') and value.get(B, {}).get('IdUUID') == b['IdUUID']}
    if any(paths.intersection(value.get(U + '.Job', {}).get('Objects', [])) for value in all_objects.values()):
        return 'Há uma operação em andamento neste dispositivo. Aguarde a conclusão e atualize a lista.'
    mounts = [text(x) for x in f.get('MountPoints', [])]
    configured = [text(x[1]['dir']) for x in fstab_items(b)]
    if any(not (x.startswith('/mnt/') or x.startswith('/media/') or x.startswith('/run/media/')) for x in mounts + configured):
        return 'Partição usada pelo sistema; alterações bloqueadas.'
    if b.get('ReadOnly') or b.get('HintIgnore') or p.get('IsContainer') or p.get('Flags', 0) & 1:
        return 'Partição protegida ou somente leitura.'
    if not b.get('IdType') or b.get('IdUsage') == 'partitiontable':
        return ('Nenhum sistema de arquivos detectado. Formate a partição em ext4 ou Btrfs antes de configurar a automontagem.' if p
                else 'Disco sem partições utilizáveis. Crie uma partição e um sistema de arquivos antes de configurar a automontagem.')
    if b.get('IdType') not in (('ext4', 'btrfs', 'ntfs', 'ntfs3') if allow_ntfs else ('ext4', 'btrfs')):
        return 'Nesta versão, a automontagem está disponível apenas para ext4 e Btrfs.'
    if not p or all_objects.get(p.get('Table'), {}).get(T, {}).get('Type') != 'gpt':
        return 'É necessária uma partição GPT para aplicar o mesmo nome à partição e ao label.'
    if b.get('CryptoBackingDevice', '/') != '/' or b.get('Drive', '/') == '/':
        return 'Volumes criptografados, virtuais ou compostos não são suportados nesta versão.'
    if any(any(option == 'bind' or option == 'rbind' or option.startswith('x-systemd.automount') for option in text(entry[1]['opts']).split(',')) for entry in fstab_items(b)):
        return 'Montagem especial; use o gerenciador de discos do sistema.'
    if not b.get('IdUUID') or len(fstab_items(b)) > 1:
        return 'Configuração complexa ou UUID ausente; use o gerenciador de discos do sistema.'
    return ''

def describe(path, obj, all_objects):
    b, p, f = obj[B], obj.get(P, {}), obj.get(F, {})
    entries = fstab_items(b)
    opts = text(entries[0][1]['opts']).split(',') if entries else []
    return dict(id=path, uuid=b.get('IdUUID', ''), device=text(b['Device']),
                label=b.get('IdLabel', ''), partition_name=p.get('Name', ''), filesystem=b.get('IdType', '') if b.get('IdUsage') == 'filesystem' else '',
                size=b.get('Size', 0), mounts=[text(x) for x in f.get('MountPoints', [])],
                mount_at_startup=bool(entries) and 'noauto' not in opts,
                configured_path=text(entries[0][1]['dir']) if entries else '', blocked_reason=reason(obj, all_objects),
                operations_blocked_reason=reason(obj, all_objects, True),
                format_blocked_reason=reason(obj, all_objects, True) or ('' if p.get('Type', '').lower() in ('ebd0a0a2-b9e5-4433-87c0-68b6b72699c7', '0fc63daf-8483-4772-8e79-3d69d8477de4') else 'Partição de sistema, recuperação ou tipo não suportado para formatação.'))

def variant_entry(entry):
    types = {'fsname':'ay', 'dir':'ay', 'type':'ay', 'opts':'ay', 'freq':'i', 'passno':'i'}
    return (entry[0], {key: GLib.Variant(types[key], value) for key, value in entry[1].items() if key in types})

def preflight(request, allow_ntfs=False):
    name = request['name']
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,15}', name):
        raise ValueError('Use de 1 a 16 caracteres: letras sem acentos, números, ponto, hífen ou sublinhado.')
    all_objects = objects()
    path = request['id']
    obj = all_objects.get(path)
    if not obj or obj.get(B, {}).get('IdUUID') != request['uuid']:
        raise ValueError('O dispositivo mudou. Atualize a lista e selecione novamente.')
    blocked = reason(obj, all_objects, allow_ntfs)
    if blocked: raise ValueError(blocked)
    b = obj[B]
    target = '/mnt/' + name
    # Refuse collisions before any authorized operation; UUID avoids duplicate-label ambiguity.
    for other_path, other in all_objects.items():
        if other_path == path: continue
        if other.get(B, {}).get('IdUUID') == b['IdUUID']:
            raise ValueError('UUID duplicado; configuração bloqueada.')
        if any(text(x) == target for x in other.get(F, {}).get('MountPoints', [])) or any(text(x[1]['dir']) == target for x in fstab_items(other.get(B, {}))):
            raise ValueError('O ponto de montagem já pertence a outro dispositivo.')
    with open('/etc/fstab', encoding='utf-8') as file:
        for line in file:
            fields = line.split()
            if fields and not fields[0].startswith('#') and len(fields) >= 2 and fields[1] == target:
                if fields[0] not in ('UUID=' + b['IdUUID'], text(b['Device']), 'LABEL=' + b.get('IdLabel', ''), 'PARTUUID=' + obj[P].get('UUID', ''), '/dev/disk/by-uuid/' + b['IdUUID']):
                    raise ValueError('Destino já reservado em /etc/fstab para outro dispositivo.')
    if os.path.islink('/mnt') or not os.path.isdir('/mnt'):
        raise ValueError('/mnt precisa ser um diretório real do sistema.')
    if os.path.lexists(target) and (os.path.islink(target) or not os.path.isdir(target)):
        raise ValueError('Destino inválido: link simbólico ou arquivo existente.')
    old_mounts = [text(x) for x in obj[F].get('MountPoints', [])]
    if os.path.exists(target) and target not in old_mounts and os.listdir(target):
        raise ValueError('A pasta de destino não está vazia.')
    return path, obj, target, old_mounts

def save(request):
    path, obj, target, old_mounts = preflight(request)
    name, b = request['name'], obj[B]
    stages = []
    try:
        if not os.path.exists(target):
            subprocess.run(['/usr/bin/pkexec', '/usr/bin/mkdir', '-m', '0755', '--', target], check=True)
            stages.append('pasta criada')
        if old_mounts:
            call(path, F, 'Unmount', '(a{sv})', ({},))
            stages.append('volume desmontado')
        refreshed = objects()
        if refreshed.get(path, {}).get(B, {}).get('IdUUID') != request['uuid'] or reason(refreshed.get(path, {}), refreshed):
            raise ValueError('O estado do dispositivo mudou; operação interrompida.')
        if b.get('IdLabel') != name:
            call(path, F, 'SetLabel', '(sa{sv})', (name, {})); stages.append('label alterado')
        if obj[P].get('Name') != name:
            call(path, P, 'SetName', '(sa{sv})', (name, {})); stages.append('nome da partição alterado')
        entries = fstab_items(b)
        old_opts = text(entries[0][1]['opts']).split(',') if entries else ['defaults']
        opts = [x for x in old_opts if x and x not in ('auto', 'noauto', 'nofail') and not x.startswith('x-systemd.device-timeout=')]
        opts += ['nofail', 'x-systemd.device-timeout=10s']
        if not request['mount_at_startup']: opts.append('noauto')
        details = {key: GLib.Variant('ay', (value + '\0').encode()) for key, value in
                   dict(fsname='UUID=' + b['IdUUID'], dir=target, type=b['IdType'], opts=','.join(opts)).items()}
        details.update(freq=GLib.Variant('i', 0), passno=GLib.Variant('i', 2 if b['IdType'] == 'ext4' else 0))
        new = ('fstab', details)
        if entries:
            call(path, B, 'UpdateConfigurationItem', '((sa{sv})(sa{sv})a{sv})', (variant_entry(entries[0]), new, {}))
        else:
            call(path, B, 'AddConfigurationItem', '((sa{sv})a{sv})', (new, {}))
        stages.append('automontagem salva')
        # Reload generated mount units before mounting with the new fstab configuration.
        # Fixed argv and Polkit authorization; no service restart and no mount -a.
        reload_systemd()
        stages.append('configuração do systemd recarregada')
        mounted = call(path, F, 'Mount', '(a{sv})', ({},)).unpack()[0]
        if mounted != target: raise ValueError('Montagem retornou um caminho diferente do solicitado.')
        return dict(mount_path=mounted)
    except Exception as error:
        raise RuntimeError('Não foi possível concluir. Etapas concluídas: ' + (', '.join(stages) or 'nenhuma') + '. ' + str(error))

def reload_systemd():
    result = subprocess.run(['/usr/bin/pkexec', '/usr/bin/systemctl', 'daemon-reload'],
                            capture_output=True, text=True, timeout=120)
    if result.returncode != 0:
        raise RuntimeError('Falha ao recarregar o systemd (daemon-reload). ' +
                           (result.stderr.strip() or 'Autorização cancelada ou comando indisponível.'))

def storage_operation(request):
    action = request.get('operation')
    if action not in ('unmount', 'remove_automount', 'remount', 'mount'):
        raise ValueError('Operação de armazenamento inválida.')
    all_objects = objects()
    path = request['id']
    obj = all_objects.get(path, {})
    if obj.get(B, {}).get('IdUUID') != request['uuid']:
        raise ValueError('O dispositivo mudou. Atualize a lista antes de continuar.')
    blocked = reason(obj, all_objects, True)
    if blocked: raise ValueError(blocked)
    if sum(value.get(B, {}).get('IdUUID') == request['uuid'] for value in all_objects.values()) != 1:
        raise ValueError('UUID duplicado; operação bloqueada.')
    original = identity(obj)
    entries = fstab_items(obj[B])
    stages = []
    def revalidate():
        current_objects = objects()
        current = current_objects.get(path, {})
        if current.get(B, {}).get('IdUUID') != request['uuid'] or identity(current) != original:
            raise ValueError('A identificação do dispositivo mudou; operação interrompida.')
        blocked = reason(current, current_objects, True)
        if blocked: raise ValueError(blocked)
        return current
    try:
        if action == 'remove_automount':
            if not entries: raise ValueError('Esta partição não possui uma entrada de automontagem em /etc/fstab.')
            current = revalidate()
            if fstab_items(current[B]) != entries:
                raise ValueError('A configuração de montagem mudou; atualize a lista.')
            call(path, B, 'RemoveConfigurationItem', '((sa{sv})a{sv})', (variant_entry(entries[0]), {}))
            stages.append('entrada de automontagem removida')
            reload_systemd()
            stages.append('configuração do systemd recarregada')
            return dict(operation=action)
        if action in ('remount', 'mount'):
            if len(entries) != 1 or not text(entries[0][1]['dir']).startswith('/mnt/'):
                raise ValueError('Configure primeiro a automontagem desta partição em /mnt.')
            target = text(entries[0][1]['dir'])
            if os.path.realpath(target) != target or not os.path.isdir(target):
                raise ValueError('Destino de montagem inválido ou pasta ausente. Configure novamente a automontagem.')
        current = revalidate()
        if action == 'mount' and current[F].get('MountPoints'):
            raise ValueError('A partição já está montada. Atualize a lista.')
        if current[F].get('MountPoints'):
            call(path, F, 'Unmount', '(a{sv})', ({},))
            stages.append('partição desmontada sem força')
        elif action == 'unmount':
            raise ValueError('A partição já está desmontada.')
        if action in ('remount', 'mount'):
            current = revalidate()
            if fstab_items(current[B]) != entries or current[F].get('MountPoints'):
                raise ValueError('A configuração ou o estado de montagem mudou; operação interrompida.')
            reload_systemd()
            revalidate()
            mounted = call(path, F, 'Mount', '(a{sv})', ({},)).unpack()[0]
            if mounted != target: raise ValueError('O caminho montado difere do configurado.')
            return dict(operation=action, mount_path=mounted)
        return dict(operation=action)
    except Exception as error:
        raise RuntimeError('Não foi possível concluir. Etapas concluídas: ' + (', '.join(stages) or 'nenhuma') + '. ' + str(error))

def identity(obj):
    b, p = obj.get(B, {}), obj.get(P, {})
    return (text(b.get('Device', b'')), b.get('DeviceNumber'), b.get('Drive'), b.get('Size'), p.get('UUID'), p.get('Table'))

def format_ntfs(request):
    fs = request.get('filesystem')
    if fs not in ('ext4', 'btrfs') or request.get('backup_confirmed') is not True:
        raise ValueError('Escolha ext4 ou Btrfs e confirme que o backup foi verificado.')
    if request.get('confirmation_word') not in ('confirmar', 'confirm'):
        raise ValueError('Digite confirmar para autorizar a formatação.')
    path, obj, target, old_mounts = preflight(request, True)
    if obj[B].get('IdType') not in ('ntfs', 'ntfs3'):
        raise ValueError('O assistente de formatação aceita somente uma partição NTFS.')
    if request.get('confirmation_device') != text(obj[B]['Device']):
        raise ValueError('A confirmação precisa corresponder exatamente ao dispositivo selecionado.')
    if obj[P].get('Type', '').lower() not in ('ebd0a0a2-b9e5-4433-87c0-68b6b72699c7', '0fc63daf-8483-4772-8e79-3d69d8477de4'):
        raise ValueError('Partições de sistema, recuperação e tipos especiais não podem ser formatados aqui.')
    if not obj[P].get('UUID'):
        raise ValueError('Identificação estável da partição ausente; formatação bloqueada.')
    available, missing = call('/org/freedesktop/UDisks2/Manager', U + '.Manager', 'CanFormat', '(s)', (fs,)).unpack()[0]
    if not available: raise ValueError('O sistema não tem suporte para criar ' + fs + '. Instale o utilitário: ' + missing)
    original = identity(obj)
    stages, format_started = [], False
    try:
        # Preparation/authorization failure happens before the destructive call.
        if not os.path.exists(target):
            subprocess.run(['/usr/bin/pkexec', '/usr/bin/mkdir', '-m', '0755', '--', target], check=True)
            stages.append('pasta criada')
        _, before_unmount, _, _ = preflight(request, True)
        if identity(before_unmount) != original:
            raise ValueError('A identificação da partição mudou; operação interrompida.')
        if before_unmount[F].get('MountPoints'):
            call(path, F, 'Unmount', '(a{sv})', ({},)); stages.append('partição desmontada')
        _, current, _, _ = preflight(request, True)
        if identity(current) != original or current[F].get('MountPoints'):
            raise ValueError('A partição mudou ou voltou a ser montada; formatação interrompida.')
        # Explicitly remove only this partition's old fstab entry; never tear down other devices.
        for entry in fstab_items(current[B]):
            call(path, B, 'RemoveConfigurationItem', '((sa{sv})a{sv})', (variant_entry(entry), {}))
            stages.append('entrada antiga de automontagem removida')
        _, current, _, _ = preflight(request, True)
        if identity(current) != original or current[F].get('MountPoints'):
            raise ValueError('A partição mudou; formatação interrompida.')
        format_started = True
        call(path, B, 'Format', '(sa{sv})', (fs, {
            'label': GLib.Variant('s', request['name']),
            'take-ownership': GLib.Variant('b', True),
            'update-partition-type': GLib.Variant('b', True),
        }))
        stages.append('partição formatada; dados anteriores apagados')
        # Wait for udev/UDisks metadata, checking stable GPT identity throughout.
        fresh = None
        for _ in range(40):
            candidate = objects().get(path, {})
            if identity(candidate) != original: raise ValueError('A identificação da partição mudou após a formatação.')
            b = candidate.get(B, {})
            if b.get('IdType') == fs and b.get('IdUUID') and b['IdUUID'] != request['uuid'] and F in candidate:
                fresh = candidate; break
            time.sleep(0.25)
        if fresh is None: raise ValueError('Formatação concluída, mas os metadados ainda não foram atualizados. Atualize a lista antes de configurar a montagem.')
        result = save(dict(request, uuid=fresh[B]['IdUUID']))
        return dict(result, formatted=True)
    except Exception as error:
        warning = ' A formatação foi iniciada: os dados podem já ter sido apagados.' if format_started else ' A formatação não foi iniciada.'
        raise RuntimeError('Não foi possível concluir.' + warning + ' Etapas concluídas: ' + (', '.join(stages) or 'nenhuma') + '. ' + str(error))

MIB = 1024 ** 2
MIN_FREE_SPACE = 64 * MIB

def child_partitions(path, all_objects):
    return {key: obj for key, obj in all_objects.items() if obj.get(P, {}).get('Table') == path}

def preparation_identity(obj, all_objects):
    b, p = obj.get(B, {}), obj.get(P, {})
    drive = all_objects.get(b.get('Drive'), {}).get(U + '.Drive', {})
    paths = [key for key, value in all_objects.items() if value is obj]
    children = child_partitions(paths[0], all_objects) if paths else {}
    return dict(device=list(identity(obj)), serial=drive.get('Serial', ''), usage=b.get('IdUsage', ''),
                filesystem=b.get('IdType', ''), uuid=b.get('IdUUID', ''), partition_type=p.get('Type', ''),
                offset=p.get('Offset'), partition_size=p.get('Size'), table_type=obj.get(T, {}).get('Type'),
                children=sorted([key, list(identity(value)), value[P].get('Offset'), value[P].get('Size'), value[P].get('Type')] for key, value in children.items()))

def empty_storage_reason(obj, all_objects, whole=False):
    b = obj.get(B, {})
    if B not in obj or b.get('ReadOnly') or b.get('Drive', '/') == '/' or b.get('CryptoBackingDevice', '/') != '/':
        return 'Dispositivo protegido, virtual ou criptografado.'
    if system_partition(obj, all_objects) or obj.get(F, {}).get('MountPoints') or fstab_items(b):
        return 'Dispositivo usado pelo sistema, montado ou com configuração existente.'
    if b.get('IdUsage', '') not in ('', 'partitiontable') or b.get('IdType', '') not in ('', 'gpt', 'dos'):
        return 'O dispositivo já tem um sistema de arquivos ou outra assinatura. Atualize a lista.'
    paths = [key for key, value in all_objects.items() if value is obj]
    if not paths: return 'Dispositivo não encontrado.'
    if any(paths[0] in value.get(U + '.Job', {}).get('Objects', []) for value in all_objects.values()):
        return 'Há uma operação em andamento neste dispositivo.'
    if whole:
        if P in obj or child_partitions(paths[0], all_objects) or obj.get(T, {}).get('Partitions'):
            return 'O disco já possui partições; não é permitido substituir sua tabela.'
    elif P not in obj or all_objects.get(obj[P].get('Table'), {}).get(T, {}).get('Type') != 'gpt':
        return 'Esta operação requer uma partição de dados em tabela GPT.'
    return ''

def free_extents(path, obj, all_objects):
    b = obj.get(B, {})
    if obj.get(T, {}).get('Type') != 'gpt' or P in obj or b.get('ReadOnly') or b.get('HintIgnore') or b.get('Drive', '/') == '/': return []
    if b.get('CryptoBackingDevice', '/') != '/' or obj.get(F, {}).get('MountPoints') or fstab_items(b): return []
    involved = {path, *child_partitions(path, all_objects)}
    if any(involved.intersection(value.get(U + '.Job', {}).get('Objects', [])) for value in all_objects.values()): return []
    ranges = []
    children = child_partitions(path, all_objects)
    if 'Partitions' in obj[T] and set(obj[T]['Partitions']) != set(children): return []
    for child in children.values():
        p = child[P]
        if not isinstance(p.get('Offset'), int) or not isinstance(p.get('Size'), int) or p['Offset'] < 0 or p['Size'] <= 0: return []
        ranges.append((p['Offset'], p['Offset'] + p['Size']))
    if not ranges: return []  # Empty disks use the whole-disk flow.
    ordered = sorted(ranges)
    if any(end > b.get('Size', 0) for _, end in ordered) or any(right[0] < left[1] for left, right in zip(ordered, ordered[1:])): return []
    limit = (int(b.get('Size', 0)) // MIB - 1) * MIB
    cursor, result = MIB, []
    for start, end in ordered + [(limit, limit)]:
        first, last = ((cursor + MIB - 1) // MIB) * MIB, (min(start, limit) // MIB) * MIB
        if last - first >= MIN_FREE_SPACE: result.append((first, last - first))
        cursor = max(cursor, end)
    return result

def storage_listing(all_objects):
    result = []
    for path, obj in all_objects.items():
        if storage_candidate(obj, all_objects):
            item = describe(path, obj, all_objects)
            if not item['filesystem']:
                whole = P not in obj
                item.update(preparation_kind='disk' if whole else 'partition', preparation_identity=preparation_identity(obj, all_objects),
                            format_blocked_reason=empty_storage_reason(obj, all_objects, whole))
            result.append(item)
        for offset, size in free_extents(path, obj, all_objects):
            item = describe(path, obj, all_objects)
            item.update(id=f'{path}#free:{offset}:{size}', label='', partition_name='', filesystem='', size=size, uuid='', mounts=[],
                        preparation_kind='free_space', preparation_identity=preparation_identity(obj, all_objects),
                        parent_id=path, offset=offset, extent_size=size, mount_at_startup=False, configured_path='',
                        blocked_reason='Espaço não particionado. Crie uma partição antes de configurar a automontagem.',
                        operations_blocked_reason='Espaço não particionado.', format_blocked_reason='')
            result.append(item)
    return result

def preparation_preflight(request):
    all_objects = objects()
    kind = request.get('preparation_kind')
    if kind not in ('disk', 'partition', 'free_space'): raise ValueError('Tipo de preparação inválido.')
    path = request.get('parent_id') if kind == 'free_space' else request.get('id')
    obj = all_objects.get(path)
    if not obj or preparation_identity(obj, all_objects) != request.get('preparation_identity'):
        raise ValueError('O dispositivo ou suas partições mudaram. Atualize a lista antes de continuar.')
    if text(obj[B].get('Device', b'')) != request.get('confirmation_device'): raise ValueError('Dispositivo de confirmação diferente.')
    if kind == 'free_space':
        offset, size = request.get('offset'), request.get('extent_size')
        if type(offset) is not int or type(size) is not int or (offset, size) not in free_extents(path, obj, all_objects):
            raise ValueError('O espaço livre mudou ou se sobrepõe a uma partição existente.')
        if request.get('id') != f'{path}#free:{offset}:{size}': raise ValueError('Identificação do espaço livre inválida.')
    else:
        blocked = empty_storage_reason(obj, all_objects, kind == 'disk')
        if blocked: raise ValueError(blocked)
    return path, obj, all_objects

def preparation_target(name, all_objects):
    target = '/mnt/' + name
    if os.path.islink('/mnt') or not os.path.isdir('/mnt'): raise ValueError('/mnt precisa ser um diretório real.')
    if os.path.lexists(target) and (os.path.islink(target) or not os.path.isdir(target) or os.listdir(target)):
        raise ValueError('A pasta de destino está ocupada ou é um link simbólico.')
    for obj in all_objects.values():
        if any(text(x) == target for x in obj.get(F, {}).get('MountPoints', [])) or any(text(x[1]['dir']) == target for x in fstab_items(obj.get(B, {}))):
            raise ValueError('Destino já usado por outro armazenamento.')
    with open('/etc/fstab', encoding='utf-8') as file:
        if any(len(fields) >= 2 and not fields[0].startswith('#') and fields[1] == target for fields in (line.split() for line in file)):
            raise ValueError('Destino já reservado em /etc/fstab.')
    return target

def prepare_storage(request):
    fs, name = request.get('filesystem'), request.get('name', '')
    if fs not in ('ext4', 'btrfs') or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,15}', name): raise ValueError('Nome ou formato inválido.')
    if request.get('backup_confirmed') is not True or request.get('confirmation_word') not in ('confirmar', 'confirm'):
        raise ValueError('Confirme a revisão dos dados e digite confirmar antes de continuar.')
    path, obj, all_objects = preparation_preflight(request)
    target = preparation_target(name, all_objects)
    available, missing = call('/org/freedesktop/UDisks2/Manager', U + '.Manager', 'CanFormat', '(s)', (fs,)).unpack()[0]
    if not available: raise ValueError('Instale o utilitário para criar ' + fs + ': ' + missing)
    stages, destructive = [], False
    original = identity(obj)
    original_serial = preparation_identity(obj, all_objects)['serial']
    kind = request['preparation_kind']
    try:
        if not os.path.exists(target): subprocess.run(['/usr/bin/pkexec', '/usr/bin/mkdir', '-m', '0755', '--', target], check=True)
        path, obj, all_objects = preparation_preflight(request)
        preparation_target(name, all_objects)
        if kind == 'disk' and obj.get(T, {}).get('Type') != 'gpt':
            destructive = True
            call(path, B, 'Format', '(sa{sv})', ('gpt', {})); stages.append('tabela GPT criada')
            for _ in range(40):
                all_objects = objects(); obj = all_objects.get(path, {})
                if identity(obj) != original or preparation_identity(obj, all_objects)['serial'] != original_serial: raise ValueError('A identificação do disco mudou.')
                if obj.get(T, {}).get('Type') == 'gpt': break
                time.sleep(.25)
            else: raise ValueError('Tabela criada, mas ainda não reconhecida pelo sistema.')
        if kind in ('disk', 'free_space'):
            if kind == 'free_space': path, obj, all_objects = preparation_preflight(request)
            elif identity(obj) != original or empty_storage_reason(obj, all_objects, True): raise ValueError('O disco deixou de estar vazio.')
            before_children = child_partitions(path, all_objects)
            offset = request['offset'] if kind == 'free_space' else MIB
            size = request['extent_size'] if kind == 'free_space' else (obj[B]['Size'] // MIB - 2) * MIB
            if size < MIN_FREE_SPACE: raise ValueError('Espaço insuficiente para criar a partição.')
            import uuid
            partition_uuid = str(uuid.uuid4())
            parent = path
            destructive = True
            path = call(parent, T, 'CreatePartition', '(ttssa{sv})', (offset, size, '0fc63daf-8483-4772-8e79-3d69d8477de4', name, {'partition-uuid': GLib.Variant('s', partition_uuid)})).unpack()[0]
            stages.append('nova partição criada')
            for _ in range(40):
                all_objects = objects(); created = all_objects.get(path, {})
                if identity(all_objects.get(parent, {})) != original or preparation_identity(all_objects.get(parent, {}), all_objects)['serial'] != original_serial: raise ValueError('A identificação do disco mudou.')
                if P in created: break
                time.sleep(.25)
            else: raise ValueError('A nova partição ainda não foi reconhecida.')
            p = created[P]
            if p.get('Table') != parent or p.get('UUID', '').lower() != partition_uuid or p.get('Offset', -1) < offset or p.get('Offset', 0) + p.get('Size', 0) > offset + size:
                raise ValueError('A partição criada não corresponde ao espaço confirmado; formatação interrompida.')
            if any(all_objects.get(key) != value for key, value in before_children.items()): raise ValueError('Outra partição mudou; formatação interrompida.')
            obj = created
        else:
            path, obj, all_objects = preparation_preflight(request)
        if empty_storage_reason(obj, all_objects): raise ValueError('A partição não está mais disponível para formatação.')
        partition_identity = identity(obj)
        destructive = True
        call(path, B, 'Format', '(sa{sv})', (fs, {'label': GLib.Variant('s', name), 'take-ownership': GLib.Variant('b', True), 'update-partition-type': GLib.Variant('b', True)}))
        stages.append('partição formatada')
        for _ in range(40):
            all_objects = objects(); fresh = all_objects.get(path, {})
            if identity(fresh) != partition_identity: raise ValueError('A identificação da partição mudou após formatar.')
            if F in fresh and fresh[B].get('IdType') == fs and fresh[B].get('IdUUID'): break
            time.sleep(.25)
        else: raise ValueError('Formatação concluída, mas os metadados ainda não foram atualizados.')
        result = save(dict(request, id=path, uuid=fresh[B]['IdUUID']))
        return dict(result, formatted=True)
    except Exception as error:
        raise RuntimeError('Não foi possível concluir. ' + ('A preparação foi iniciada; a tabela ou os dados podem ter sido alterados. ' if destructive else 'A formatação não foi iniciada. ') + 'Etapas concluídas: ' + (', '.join(stages) or 'nenhuma') + '. ' + str(error))

def main():
    try:
        request = json.load(sys.stdin)
        if request['action'] == 'list':
            all_objects = objects()
            result = storage_listing(all_objects)
        elif request['action'] == 'save': result = save(request)
        elif request['action'] == 'operation': result = storage_operation(request)
        elif request['action'] == 'format_ntfs': result = format_ntfs(request)
        elif request['action'] == 'prepare_storage': result = prepare_storage(request)
        else: raise ValueError('Operação inválida.')
        print(json.dumps(result))
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__": main()
