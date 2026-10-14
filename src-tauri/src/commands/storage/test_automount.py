import copy, importlib.util, io, unittest
from unittest.mock import patch
from pathlib import Path
spec = importlib.util.spec_from_file_location('automount', Path(__file__).with_name('automount.py'))
a = importlib.util.module_from_spec(spec); spec.loader.exec_module(a)

class AutomountTests(unittest.TestCase):
    def setUp(self):
        self.path = '/org/freedesktop/UDisks2/block_devices/sdb1'
        self.obj = {a.B: {'Device':b'/dev/sdb1\0', 'IdType':'ext4', 'IdUUID':'test-uuid', 'IdLabel':'Old', 'Drive':'/drive', 'Size':1000}, a.F:{'MountPoints':[]}, a.P:{'Name':'Old', 'Table':'/table'}}
        self.objects = {self.path:self.obj, '/table':{a.T:{'Type':'gpt'}}}
        self.request = dict(id=self.path, uuid='test-uuid', name='Games', mount_at_startup=True)
    def test_critical_mounts_and_fstab_are_blocked(self):
        for mount in ['/', '/home', '/boot', '/usr', '/var', '/etc']:
            obj = copy.deepcopy(self.obj); obj[a.F]['MountPoints']=[(mount+'\0').encode()]
            self.assertTrue(a.reason(obj, self.objects))
            obj[a.F]['MountPoints']=[];obj[a.B]['Configuration']=[('fstab',{'dir':(mount+'\0').encode(), 'opts':b'defaults\0'})]
            self.assertTrue(a.reason(obj, self.objects))
    def test_ntfs_mbr_readonly_encrypted_blocked(self):
        for interface, key, value in [(a.B,'IdType','ntfs'),(a.B,'ReadOnly',True),(a.B,'CryptoBackingDevice','/crypto'),(a.P,'Flags',1)]:
            obj=copy.deepcopy(self.obj);obj[interface][key]=value
            self.assertTrue(a.reason(obj,self.objects))
        self.objects['/table'][a.T]['Type']='dos'
        self.assertTrue(a.reason(self.obj,self.objects))
    def test_listing_hides_system_mounts_and_fstab_even_when_unmounted(self):
        self.obj[a.B]['IdUsage'] = 'filesystem'
        for mount in ['/', '/home', '/home/user', '/boot', '/boot/efi', '/efi', '/usr', '/var']:
            obj = copy.deepcopy(self.obj)
            obj[a.F]['MountPoints'] = [(mount + '\0').encode()]
            self.assertFalse(a.storage_candidate(obj, self.objects), mount)
            obj[a.F]['MountPoints'] = []
            obj[a.B]['Configuration'] = [('fstab', {'dir': (mount + '\0').encode(), 'opts': b'defaults\0'})]
            self.assertFalse(a.storage_candidate(obj, self.objects), mount)

    def test_listing_hides_unmounted_efi_and_recovery_but_keeps_game_data(self):
        self.obj[a.B]['IdUsage'] = 'filesystem'
        for filesystem in ['ext4', 'btrfs', 'ntfs', 'ntfs3']:
            self.obj[a.B]['IdType'] = filesystem
            self.obj[a.P]['Type'] = 'ebd0a0a2-b9e5-4433-87c0-68b6b72699c7' if filesystem.startswith('ntfs') else '0fc63daf-8483-4772-8e79-3d69d8477de4'
            self.assertTrue(a.storage_candidate(self.obj, self.objects))
        for partition_type, filesystem in [('c12a7328-f81f-11d2-ba4b-00a0c93ec93b', 'vfat'), ('de94bba4-06d1-4d40-a16a-bfd50179d6ac', 'ntfs'), ('933ac7e1-2eb4-4f13-b844-0e14e2aef915', 'ext4')]:
            self.obj[a.P]['Type'] = partition_type
            self.obj[a.B]['IdType'] = filesystem
            self.assertFalse(a.storage_candidate(self.obj, self.objects))
            self.assertTrue(a.reason(self.obj, self.objects, True))

    def test_listing_protects_shared_btrfs_root_uuid(self):
        self.obj[a.B].update(IdUsage='filesystem', IdType='btrfs')
        peer = copy.deepcopy(self.obj)
        peer[a.F]['MountPoints'] = [b'/\0', b'/home\0']
        self.objects['/root-volume'] = peer
        self.assertFalse(a.storage_candidate(self.obj, self.objects))
        self.assertTrue(a.reason(self.obj, self.objects, True))

    def test_unformatted_partition_is_listed_but_operations_stay_blocked(self):
        self.obj.pop(a.F)
        self.obj[a.B].update(IdUsage='', IdType='', IdUUID='')
        self.obj[a.P]['Type'] = '0fc63daf-8483-4772-8e79-3d69d8477de4'
        self.assertTrue(a.storage_candidate(self.obj, self.objects))
        self.assertIn('Nenhum sistema de arquivos', a.reason(self.obj, self.objects, True))
        self.assertEqual(a.describe(self.path, self.obj, self.objects)['filesystem'], '')
        self.obj[a.P]['Type'] = 'c12a7328-f81f-11d2-ba4b-00a0c93ec93b'
        self.assertFalse(a.storage_candidate(self.obj, self.objects))

    def test_empty_physical_disk_is_listed_without_duplicate_partition_parent(self):
        disk = {a.B: {'Device': b'/dev/sdz\0', 'Drive': '/drive', 'IdUsage': '', 'IdType': ''}}
        all_objects = {'/disk': disk}
        self.assertTrue(a.storage_candidate(disk, all_objects))
        disk[a.T] = {'Type': 'gpt'}
        disk[a.B].update(IdUsage='partitiontable', IdType='gpt')
        self.assertTrue(a.storage_candidate(disk, all_objects))
        all_objects['/partition'] = {a.P: {'Table': '/disk'}}
        self.assertFalse(a.storage_candidate(disk, all_objects))
        del all_objects['/partition']
        disk[a.B]['Drive'] = '/'
        self.assertFalse(a.storage_candidate(disk, all_objects))

    def test_bad_names_rejected_before_bus_or_mutation(self):
        for name in ['../root','a/b','', '-abc', 'a'*17, 'a;id', 'a\nroot']:
            with patch.object(a,'objects',side_effect=AssertionError('unexpected bus call')):
                with self.assertRaises(ValueError):a.save(dict(self.request,name=name))
    def test_changed_device_and_mount_conflicts(self):
        with patch.object(a,'objects',return_value=self.objects):
            with self.assertRaises(ValueError):a.save(dict(self.request,uuid='stale'))
            self.objects['/other']={a.B:{'IdUUID':'other'},a.F:{'MountPoints':[b'/mnt/Games\0']}}
            with self.assertRaises(ValueError):a.save(self.request)
    def test_symlink_destination_rejected(self):
        with patch.object(a,'objects',return_value=self.objects), patch.object(a.os.path,'islink',side_effect=lambda x:x=='/mnt/Games'), patch.object(a.os.path,'isdir',return_value=True), patch.object(a.os.path,'lexists',return_value=True), patch('builtins.open',return_value=io.StringIO('')):
            with self.assertRaises(ValueError):a.save(self.request)
    def test_save_variants_uuid_nofail_and_no_format(self):
        calls=[]
        def call(path, interface, method, signature, values):
            # Construction checks actual GLib D-Bus wire types without contacting a daemon.
            a.GLib.Variant(signature, values)
            calls.append((method,values))
            if method == 'Mount': self.assertTrue(reload_mock.called)
            return a.GLib.Variant('(s)',('/mnt/Games',)) if method=='Mount' else a.GLib.Variant('()',())
        with patch.object(a.subprocess,'run',return_value=a.subprocess.CompletedProcess([],0,'','')) as reload_mock, patch.object(a,'objects',return_value=self.objects), patch.object(a,'call',side_effect=call), patch.object(a.os.path,'islink',return_value=False), patch.object(a.os.path,'isdir',return_value=True), patch.object(a.os.path,'lexists',return_value=True), patch.object(a.os.path,'exists',return_value=True), patch.object(a.os,'listdir',return_value=[]), patch('builtins.open',return_value=io.StringIO('')):
            a.save(dict(self.request,mount_at_startup=False))
        self.assertEqual(reload_mock.call_args.args[0], ['/usr/bin/pkexec', '/usr/bin/systemctl', 'daemon-reload'])
        self.assertEqual([x[0] for x in calls],['SetLabel','SetName','AddConfigurationItem','Mount'])
        details=calls[2][1][0][1]
        self.assertEqual(bytes(details['fsname'].unpack()),b'UUID=test-uuid\0')
        self.assertIn(b'noauto',bytes(details['opts'].unpack()))
        self.assertIn(b'nofail',bytes(details['opts'].unpack()))
    def test_partial_failure_reports_completed_step(self):
        def call(path,interface,method,signature,values):
            if method=='SetName':raise RuntimeError('authorization cancelled')
        with patch.object(a.subprocess,'run',return_value=a.subprocess.CompletedProcess([],0,'','')) as reload_mock, patch.object(a,'objects',return_value=self.objects), patch.object(a,'call',side_effect=call), patch.object(a.os.path,'islink',return_value=False), patch.object(a.os.path,'isdir',return_value=True), patch.object(a.os.path,'lexists',return_value=True), patch.object(a.os.path,'exists',return_value=True), patch.object(a.os,'listdir',return_value=[]), patch('builtins.open',return_value=io.StringIO('')):
            with self.assertRaisesRegex(RuntimeError,'label alterado'):a.save(self.request)

    def test_reload_denial_reports_saved_config_and_does_not_mount(self):
        calls=[]
        def call(path,interface,method,signature,values):calls.append(method)
        with patch.object(a.subprocess,'run',return_value=a.subprocess.CompletedProcess([],126,'','Authorization cancelled')), patch.object(a,'objects',return_value=self.objects), patch.object(a,'call',side_effect=call), patch.object(a.os.path,'islink',return_value=False), patch.object(a.os.path,'isdir',return_value=True), patch.object(a.os.path,'lexists',return_value=True), patch.object(a.os.path,'exists',return_value=True), patch.object(a.os,'listdir',return_value=[]), patch('builtins.open',return_value=io.StringIO('')):
            with self.assertRaisesRegex(RuntimeError,'automontagem salva.*daemon-reload'):a.save(self.request)
        self.assertNotIn('Mount',calls)

    def format_request(self):
        self.obj[a.B]['IdType']='ntfs'
        self.obj[a.P].update(Type='ebd0a0a2-b9e5-4433-87c0-68b6b72699c7', UUID='stable-partuuid')
        return dict(self.request, filesystem='ext4', backup_confirmed=True, confirmation_device='/dev/sdb1', confirmation_word='confirmar')
    def test_format_requires_backup_and_exact_device(self):
        request=self.format_request()
        with patch.object(a,'preflight',return_value=(self.path,self.obj,'/mnt/Games',[])), patch.object(a,'call',side_effect=AssertionError('unexpected mutation')):
            for change in [dict(backup_confirmed=False), dict(confirmation_device='/dev/sda1'),dict(filesystem='ntfs'), dict(confirmation_word=''), dict(confirmation_word='apagar')]:
                with self.assertRaises(ValueError):a.format_ntfs(dict(request,**change))
    def test_format_blocks_recovery_and_non_ntfs(self):
        request=self.format_request()
        with patch.object(a,'preflight',return_value=(self.path,self.obj,'/mnt/Games',[])), patch.object(a,'call',side_effect=AssertionError('unexpected mutation')):
            self.obj[a.P]['Type']='de94bba4-06d1-4d40-a16a-bfd50179d6ac'
            with self.assertRaises(ValueError):a.format_ntfs(request)
            self.obj[a.B]['IdType']='ext4'
            with self.assertRaises(ValueError):a.format_ntfs(request)
    def test_missing_format_utility_leaves_data_untouched(self):
        request=self.format_request()
        with patch.object(a,'preflight',return_value=(self.path,self.obj,'/mnt/Games',[])), patch.object(a,'call',return_value=a.GLib.Variant('((bs))',((False,'mkfs.ext4'),))) as calls:
            with self.assertRaisesRegex(ValueError,'mkfs.ext4'):a.format_ntfs(request)
        self.assertEqual([x.args[2] for x in calls.call_args_list],['CanFormat'])
    def test_format_success_uses_new_uuid_and_mounts(self):
        request=self.format_request(); calls=[]
        fresh=copy.deepcopy(self.obj);fresh[a.B].update(IdUUID='new-uuid',IdType='ext4',IdLabel='Games')
        def call(path,interface,method,signature,values):
            a.GLib.Variant(signature,values);calls.append((method,values))
            return a.GLib.Variant('((bs))',((True,''),)) if method=='CanFormat' else a.GLib.Variant('()',())
        with patch.object(a,'preflight',return_value=(self.path,self.obj,'/mnt/Games',[])),patch.object(a,'call',side_effect=call),patch.object(a.os.path,'exists',return_value=True),patch.object(a,'objects',return_value={self.path:fresh}),patch.object(a,'save',return_value=dict(mount_path='/mnt/Games')) as saved:
            result=a.format_ntfs(request)
        self.assertTrue(result['formatted']);self.assertEqual(saved.call_args.args[0]['uuid'],'new-uuid')
        self.assertEqual([x[0] for x in calls],['CanFormat','Format'])
        self.assertTrue(calls[1][1][1]['take-ownership'].unpack())
    def test_format_auth_denial_reports_possible_data_loss(self):
        request=self.format_request()
        def call(path,interface,method,signature,values):
            if method=='CanFormat':return a.GLib.Variant('((bs))',((True,''),))
            raise RuntimeError('Authorization cancelled')
        with patch.object(a,'preflight',return_value=(self.path,self.obj,'/mnt/Games',[])),patch.object(a,'call',side_effect=call),patch.object(a.os.path,'exists',return_value=True),patch.object(a,'save',side_effect=AssertionError('must not mount')):
            with self.assertRaisesRegex(RuntimeError,'dados podem já ter sido apagados'):a.format_ntfs(request)
    def test_device_changed_during_preparation_is_not_formatted(self):
        request=self.format_request();changed=copy.deepcopy(self.obj);changed[a.P]['UUID']='different-device'
        with patch.object(a,'preflight',side_effect=[(self.path,self.obj,'/mnt/Games',[]),(self.path,changed,'/mnt/Games',[])]),patch.object(a,'call',return_value=a.GLib.Variant('((bs))',((True,''),))) as calls,patch.object(a.os.path,'exists',return_value=True):
            with self.assertRaisesRegex(RuntimeError,'formatação não foi iniciada'):a.format_ntfs(request)
        self.assertEqual([x.args[2] for x in calls.call_args_list],['CanFormat'])

    def test_active_disk_job_blocks_another_operation(self):
        self.objects['/job']={a.U + '.Job': {'Objects':[self.path]}}
        self.assertTrue(a.reason(self.obj,self.objects))
        self.assertTrue(a.reason(self.obj,self.objects,True))

    def test_eject_only_unmounts_selected_partition_without_force(self):
        self.obj[a.F]['MountPoints']=[b'/mnt/Games\0']
        self.objects['/sibling']={a.B:{'IdUUID':'sibling'}, a.F:{'MountPoints':[b'/mnt/Other\0']}}
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',return_value=a.GLib.Variant('()',())) as calls:
            a.storage_operation(dict(self.request,operation='unmount'))
        self.assertEqual(calls.call_args.args, (self.path,a.F,'Unmount','(a{sv})',({},)))
        self.assertEqual(calls.call_count,1)
    def test_remove_only_selected_fstab_entry_and_reload(self):
        self.obj[a.B]['Configuration']=[('fstab',dict(dir=b'/mnt/Games\0',opts=b'defaults\0',fsname=b'UUID=test-uuid\0',type=b'ext4\0',freq=0,passno=2))]
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',return_value=a.GLib.Variant('()',())) as calls,patch.object(a,'reload_systemd') as reload:
            a.storage_operation(dict(self.request,operation='remove_automount'))
        self.assertEqual([x.args[2] for x in calls.call_args_list],['RemoveConfigurationItem'])
        self.assertEqual(calls.call_args.args[0],self.path)
        self.assertEqual(calls.call_args.args[4][0][0],'fstab');reload.assert_called_once()
    def test_remount_unmounts_before_mounting_configured_path(self):
        self.obj[a.F]['MountPoints']=[b'/mnt/Games\0']
        self.obj[a.B]['Configuration']=[('fstab',dict(dir=b'/mnt/Games\0',opts=b'defaults\0'))]
        calls=[]
        def call(path,interface,method,signature,values):
            calls.append(method)
            if method=='Unmount': self.obj[a.F]['MountPoints']=[]
            return a.GLib.Variant('(s)',('/mnt/Games',)) if method=='Mount' else a.GLib.Variant('()',())
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',side_effect=call),patch.object(a,'reload_systemd') as reload,patch.object(a.os.path,'isdir',return_value=True):
            result=a.storage_operation(dict(self.request,operation='remount'))
        self.assertEqual(calls,['Unmount','Mount']);reload.assert_called_once();self.assertEqual(result['mount_path'],'/mnt/Games')
    def test_operations_reject_system_stale_and_missing_configuration(self):
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',side_effect=AssertionError('must not mutate')):
            for request in [dict(self.request,operation='bad'),dict(self.request,operation='unmount',uuid='stale'),dict(self.request,operation='remount'),dict(self.request,operation='remove_automount')]:
                with self.assertRaises((ValueError,RuntimeError)):a.storage_operation(request)
            self.obj[a.F]['MountPoints']=[b'/\0']
            with self.assertRaises(ValueError):a.storage_operation(dict(self.request,operation='unmount'))
    def test_busy_eject_does_not_force_or_touch_configuration(self):
        self.obj[a.F]['MountPoints']=[b'/mnt/Games\0']
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',side_effect=RuntimeError('DeviceBusy')) as calls:
            with self.assertRaisesRegex(RuntimeError,'DeviceBusy'):a.storage_operation(dict(self.request,operation='unmount'))
        self.assertEqual(calls.call_count,1);self.assertEqual(calls.call_args.args[4],({},))

    def test_mount_does_not_unmount_and_requires_configuration(self):
        self.obj[a.B]['Configuration']=[('fstab',dict(dir=b'/mnt/Games\0',opts=b'noauto,nofail\0'))]
        with patch.object(a,'objects',return_value=self.objects),patch.object(a,'call',return_value=a.GLib.Variant('(s)',('/mnt/Games',))) as calls,patch.object(a,'reload_systemd'),patch.object(a.os.path,'isdir',return_value=True):
            a.storage_operation(dict(self.request,operation='mount'))
            self.assertEqual([x.args[2] for x in calls.call_args_list],['Mount'])
            self.obj[a.F]['MountPoints']=[b'/mnt/Games\0']
            with self.assertRaisesRegex(RuntimeError,'já está montada'):a.storage_operation(dict(self.request,operation='mount'))
            self.assertEqual(calls.call_count,1)

if __name__=='__main__':unittest.main()
