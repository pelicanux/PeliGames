import copy, importlib.util, io, json, unittest
from pathlib import Path
from unittest.mock import patch
spec = importlib.util.spec_from_file_location('preparation', Path(__file__).with_name('automount.py'))
a = importlib.util.module_from_spec(spec); spec.loader.exec_module(a)

class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.path = '/disk'
        self.disk = {a.B: {'Device': b'/dev/testdisk\0', 'DeviceNumber': 8, 'Drive': '/drive', 'Size': 256*a.MIB, 'IdUsage': '', 'IdType': '', 'IdUUID': ''}}
        self.objects = {self.path: self.disk, '/drive': {a.U+'.Drive': {'Serial': 'test-serial'}}}
    def request(self, kind='disk', path=None):
        path = path or self.path
        obj = self.objects[path]
        return dict(id=path, preparation_kind=kind, preparation_identity=json.loads(json.dumps(a.preparation_identity(obj, self.objects))),
                    name='Games', filesystem='ext4', backup_confirmed=True, confirmation_word='confirmar',
                    confirmation_device=a.text(obj[a.B]['Device']), mount_at_startup=True)
    def add_partition(self, filesystem='', offset=None, size=None):
        self.disk[a.T] = {'Type': 'gpt'}
        self.disk[a.B].update(IdUsage='partitiontable', IdType='gpt')
        self.objects['/part'] = {a.B: {'Device':b'/dev/testdisk1\0', 'DeviceNumber':9, 'Drive':'/drive', 'Size':size or 100*a.MIB, 'IdType':filesystem,'IdUsage':'filesystem' if filesystem else '', 'IdUUID':'known' if filesystem else ''},
                               a.P: {'Type':'0fc63daf-8483-4772-8e79-3d69d8477de4','UUID':'part-uuid','Table':self.path,'Offset':offset or a.MIB,'Size':size or 100*a.MIB}}
        if filesystem: self.objects['/part'][a.F] = {'MountPoints':[]}
    def free_request(self):
        self.add_partition('ext4')
        item = next(item for item in a.storage_listing(self.objects) if item.get('preparation_kind') == 'free_space')
        return json.loads(json.dumps(dict(self.request(), **{key:item[key] for key in ['id','parent_id','preparation_kind','preparation_identity','offset','extent_size']})))
    def mocks(self):
        return patch.multiple(a, objects=lambda:self.objects, preparation_target=lambda name, objects:'/mnt/'+name)
    def test_free_space_listing_excludes_partition_ranges_and_small_alignment_gaps(self):
        request = self.free_request()
        self.assertGreaterEqual(request['offset'], 101*a.MIB)
        self.assertLessEqual(request['offset']+request['extent_size'],255*a.MIB)
        self.assertFalse(any(item['id'] == self.path for item in a.storage_listing(self.objects)))
    def test_changed_identity_overlap_and_protected_partition_are_rejected(self):
        request = self.free_request()
        with self.mocks():
            for change in [dict(offset=a.MIB),dict(extent_size=300*a.MIB),dict(preparation_identity={}),dict(confirmation_device='/dev/other')]:
                with self.assertRaises(ValueError): a.preparation_preflight(dict(request, **change))
        self.objects['/part'][a.P]['Type']='c12a7328-f81f-11d2-ba4b-00a0c93ec93b'
        request=self.request('partition','/part')
        with self.mocks(), self.assertRaises(ValueError): a.preparation_preflight(request)
    def test_no_confirmation_or_missing_utility_never_mutates(self):
        request=self.request()
        with self.mocks(), patch.object(a,'call',side_effect=AssertionError('unexpected operation')):
            for change in [dict(backup_confirmed=False),dict(confirmation_word=''),dict(filesystem='ntfs'),dict(name='../root')]:
                with self.assertRaises(ValueError): a.prepare_storage(dict(request, **change))
        with self.mocks(), patch.object(a,'call',return_value=a.GLib.Variant('((bs))',((False,'mkfs.ext4'),))) as calls:
            with self.assertRaisesRegex(ValueError,'mkfs.ext4'): a.prepare_storage(request)
        self.assertEqual([c.args[2] for c in calls.call_args_list],['CanFormat'])
    def run_success(self, kind, wrong_partition=False):
        if kind == 'free_space': request=self.free_request()
        elif kind == 'partition': self.add_partition(); request=self.request('partition','/part')
        else: request=self.request()
        calls=[]
        def call(path,interface,method,signature,values):
            a.GLib.Variant(signature,values); calls.append((path,method))
            if method=='CanFormat':return a.GLib.Variant('((bs))',((True,''),))
            if method=='Format' and values[0]=='gpt':
                self.disk[a.T]={'Type':'gpt'};self.disk[a.B].update(IdUsage='partitiontable',IdType='gpt')
            elif method=='CreatePartition':
                offset,size,_,_,options=values
                self.objects['/created']={a.B:{'Device':b'/dev/testdisk2\0','DeviceNumber':10,'Drive':'/drive','Size':size,'IdType':'','IdUsage':'','IdUUID':''},a.P:{'UUID':options['partition-uuid'].unpack(),'Table':self.path,'Offset':offset,'Size':size,'Type':'0fc63daf-8483-4772-8e79-3d69d8477de4'}}
                if wrong_partition:self.objects['/created'][a.P]['Offset']=0
                return a.GLib.Variant('(o)',('/created',))
            elif method=='Format':
                self.objects[path][a.B].update(IdUsage='filesystem',IdType='ext4',IdUUID='new-uuid')
                self.objects[path][a.F]={'MountPoints':[]}
            return a.GLib.Variant('()',())
        sibling = copy.deepcopy(self.objects.get('/part'))
        with self.mocks(),patch.object(a,'call',side_effect=call),patch.object(a.os.path,'exists',return_value=True),patch.object(a,'save',return_value={'mount_path':'/mnt/Games'}) as save:
            if wrong_partition:
                with self.assertRaisesRegex(RuntimeError,'não corresponde'):a.prepare_storage(request)
                self.assertFalse(save.called)
            else:
                self.assertTrue(a.prepare_storage(request)['formatted'])
                self.assertEqual(save.call_args.args[0]['uuid'],'new-uuid')
        if kind == 'free_space':self.assertEqual(self.objects['/part'],sibling)
        return calls
    def test_empty_disk_creates_gpt_partition_then_formats(self):
        self.assertEqual([method for _,method in self.run_success('disk')],['CanFormat','Format','CreatePartition','Format'])
    def test_existing_empty_partition_formats_only_selected_device(self):
        self.assertEqual(self.run_success('partition'),[('/org/freedesktop/UDisks2/Manager','CanFormat'),('/part','Format')])
    def test_free_space_preserves_existing_partitions_and_does_not_replace_table(self):
        self.assertEqual([method for _,method in self.run_success('free_space')],['CanFormat','CreatePartition','Format'])
    def test_wrong_created_partition_is_not_formatted(self):
        self.assertNotIn('Format',[method for _,method in self.run_success('free_space', True)])
    def test_entire_disk_with_partitions_cannot_be_reinitialized(self):
        self.add_partition('ext4')
        request=self.request('disk')
        with self.mocks(),patch.object(a,'call',side_effect=AssertionError('unexpected mutation')):
            with self.assertRaises(ValueError):a.prepare_storage(request)
    def test_serial_change_blocks_request(self):
        request=self.request()
        self.objects['/drive'][a.U+'.Drive']['Serial']='changed'
        with self.mocks(),self.assertRaisesRegex(ValueError,'mudaram'):a.preparation_preflight(request)

    def test_incomplete_partition_metadata_never_offers_free_space_or_disk_reset(self):
        self.add_partition('ext4')
        self.disk[a.T]['Partitions'] = ['/part', '/missing-partition']
        self.assertEqual(a.free_extents(self.path, self.disk, self.objects), [])
        del self.objects['/part']
        self.assertTrue(a.empty_storage_reason(self.disk, self.objects, True))
