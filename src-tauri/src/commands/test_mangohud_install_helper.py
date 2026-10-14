import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('helper', Path(__file__).with_name('mangohud_install_helper.py'))
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

def archive(path, files):
    with tarfile.open(path, 'w:gz') as out:
        for name, content in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(content)
            out.addfile(info, io.BytesIO(content))

class InstallHelperTests(unittest.TestCase):
    def test_verified_staging_keeps_original_archive_and_prepares_root_branch(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            inner = root / 'inner.tar'
            archive(inner, {name:b'payload' for name in ['usr/bin/mangohud','usr/lib/mangohud/lib64/libMangoHud.so','usr/lib/mangohud/lib32/libMangoHud.so']})
            outer = root / 'outer.tar.gz'
            archive(outer, {'MangoHud/mangohud-setup.sh':b'#!/bin/bash\n', 'MangoHud/MangoHud-package.tar':inner.read_bytes()})
            stage = root / 'stage'; stage.mkdir()
            original = outer.read_bytes()
            script, expected = helper.prepare(outer, hashlib.sha256(original).hexdigest(), len(original), stage)
            self.assertEqual(outer.read_bytes(), original)
            self.assertTrue(script.is_file())
            self.assertTrue((script.parent / 'usr/bin/mangohud').is_file())
            self.assertEqual(len(expected), 3)
    def test_rejects_modified_package_before_extraction(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root); outer = root / 'package';outer.write_bytes(b'changed');stage=root/'stage';stage.mkdir()
            with self.assertRaises(ValueError): helper.prepare(outer, 'a'*64, 7, stage)
            self.assertFalse((stage/'extracted').exists())
    def test_rejects_traversal_and_absolute_paths(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root)
            for name in ['../outside','/tmp/outside']:
                path=root/'package';archive(path,{name:b'bad'})
                with self.assertRaises(ValueError):helper.extract_package(path,root/'stage')
    def test_rejects_symlinks(self):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'package'
            with tarfile.open(path,'w') as out:
                item=tarfile.TarInfo('link');item.type=tarfile.SYMTYPE;item.linkname='/etc';out.addfile(item)
            with self.assertRaises(ValueError):helper.extract_package(path,Path(root)/'stage')
    def test_rejects_incomplete_package(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);path=root/'package';archive(path,{'readme':b'not an installer'});stage=root/'stage';stage.mkdir()
            with self.assertRaises(ValueError):helper.prepare(path,hashlib.sha256(path.read_bytes()).hexdigest(),path.stat().st_size,stage)

if __name__ == '__main__': unittest.main()
