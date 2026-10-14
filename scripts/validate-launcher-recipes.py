#!/usr/bin/env python3
"""Offline validation of the catalog, schema subset and cross-file invariants.

Uses only the standard library. The schema evaluator implements precisely the
keywords used by recipe.schema.json; it is not a general JSON Schema library.
This validates data, never downloads files or executes installation plans.
"""
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1] / 'installers'
DOWNLOAD_HOSTS = {
    'ubisoft-connect': 'ubistatic3-a.akamaihd.net',
    'ea-app': 'origin-a.akamaihd.net',
    'battle-net': 'www.battle.net',
}
STEPS = ['preflight', 'select_runner', 'download_installer', 'create_prefix',
         'review_dependencies', 'run_installer', 'verify_executable', 'register_launcher']


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    def unique_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f'Duplicate JSON key: {key}')
            result[key] = value
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique_pairs)


def shape(value, schema, location='$'):
    allowed_keywords = {'type', 'const', 'enum', 'minimum', 'minLength', 'pattern',
                        'properties', 'required', 'additionalProperties', 'items', '$schema', 'title'}
    require(set(schema) <= allowed_keywords, f'{location}: unsupported schema keyword')
    types = {'object': lambda x: isinstance(x, dict), 'array': lambda x: isinstance(x, list),
             'string': lambda x: isinstance(x, str), 'boolean': lambda x: type(x) is bool,
             'integer': lambda x: type(x) is int, 'null': lambda x: x is None}
    if 'type' in schema:
        names = schema['type'] if isinstance(schema['type'], list) else [schema['type']]
        require(any(types[t](value) for t in names), f'{location}: wrong type')
    if 'const' in schema:
        require(type(value) is type(schema['const']) and value == schema['const'], f'{location}: wrong constant')
    if 'enum' in schema:
        require(value in schema['enum'], f'{location}: unknown value {value!r}')
    if isinstance(value, str):
        require(len(value) >= schema.get('minLength', 0), f'{location}: empty value')
        if 'pattern' in schema:
            require(re.search(schema['pattern'], value), f'{location}: invalid format')
    if type(value) is int and 'minimum' in schema:
        require(value >= schema['minimum'], f'{location}: below minimum')
    if isinstance(value, dict):
        properties = schema.get('properties', {})
        require(set(schema.get('required', [])) <= set(value), f'{location}: missing properties')
        if schema.get('additionalProperties') is False:
            require(set(value) <= set(properties), f'{location}: unknown properties')
        for key, item in value.items():
            if key in properties:
                shape(item, properties[key], f'{location}.{key}')
    if isinstance(value, list) and 'items' in schema:
        for index, item in enumerate(value):
            shape(item, schema['items'], f'{location}[{index}]')


def https(url):
    parsed = urlsplit(url)
    require(parsed.scheme == 'https' and parsed.hostname and not parsed.username
            and not parsed.password and not parsed.fragment and parsed.port in (None, 443),
            f'Unsafe URL: {url}')
    return parsed.hostname


def relative(path, glob=False):
    parts = PurePosixPath(path).parts
    require(parts and not path.startswith('/') and not any(p in ('.', '..') for p in path.split('/'))
            and '\\' not in path and ':' not in path and '\x00' not in path, f'Unsafe path: {path}')
    if glob:
        require(all(not any(c in p for c in '?[]') and ('*' not in p or p == '*') for p in parts),
                f'Unbounded glob: {path}')
    else:
        require(not any(c in path for c in '*?[]'), f'Unexpected glob: {path}')
    return parts


def validate_recipe(recipe, schema, source_ids):
    shape(recipe, schema)
    require(recipe['id'] in DOWNLOAD_HOSTS, 'Launcher outside current catalog scope')
    require(https(recipe['download']['url']) == DOWNLOAD_HOSTS[recipe['id']], 'Non-official download host')
    filename = recipe['download']['filename']
    require(len(relative(filename)) == 1 and filename.lower().endswith('.' + recipe['download']['kind']), 'Invalid installer filename')
    require(recipe['steps'] == STEPS, 'Missing or reordered verification steps')
    require(recipe['runners'], 'No runner candidates')
    require(recipe['executable']['relative_paths'], 'No executable paths')
    for path in recipe['executable']['relative_paths'] + recipe['executable']['relative_globs']:
        parts = relative(path, path in recipe['executable']['relative_globs'])
        require(parts[0] == 'drive_c' and path.endswith('.exe'), 'Executable outside prefix or wrong extension')
    for candidate in recipe['runners']:
        require(candidate['phases'] and len(set(candidate['phases'])) == len(candidate['phases']), 'Invalid runner phases')
        if candidate['family'].endswith('proton'):
            require(candidate['backend'] == 'umu', 'Proton outside Steam requires UMU')
    for key in ('dependencies', 'mitigations', 'known_issues'):
        field = 'verb' if key == 'dependencies' else 'id'
        require(len({item[field] for item in recipe[key]}) == len(recipe[key]), f'Duplicate {key}')
    status = recipe['validation']
    if status['status'] == 'research-only':
        require(not status['installation_tested'] and not status['launch_tested']
                and not any(r['tested_by_peligames'] for r in recipe['runners']), 'Contradictory test status')
    elif status['status'] == 'installation-tested':
        require(status['installation_tested'] and not status['launch_tested'], 'Contradictory test status')
    else:
        require(status['installation_tested'] and status['launch_tested'], 'Launch-tested requires installation test')
    def references(value):
        if isinstance(value, dict):
            if 'evidence' in value:
                require(value['evidence'] and set(value['evidence']) <= source_ids, 'Missing or unknown evidence')
            for child in value.values():
                references(child)
        elif isinstance(value, list):
            for child in value:
                references(child)
    references(recipe)


def validate_catalog(root=ROOT):
    catalog = read(root / 'index.json')
    require(catalog['schema_version'] == 1 and catalog['minimum_recipe_schema'] == 1, 'Unknown catalog schema')
    require(catalog['status'] == 'experimental', 'Catalog needs explicit promotion review')
    sources = catalog['sources']
    ids = {s['id'] for s in sources}
    require(len(ids) == len(sources), 'Duplicate sources')
    for source in sources:
        https(source['url'])
    for check in catalog['preflight']:
        require(set(check['evidence']) <= ids, 'Unknown preflight evidence')
    require(catalog['policy']['arbitrary_shell'] is False and catalog['policy']['requires_root'] is False, 'Unsafe catalog policy')
    schema = read(root / 'recipe.schema.json')
    entries = catalog['launchers']
    require({e['id'] for e in entries} == set(DOWNLOAD_HOSTS) and len(entries) == len(DOWNLOAD_HOSTS), 'Wrong catalog scope')
    require('gog-galaxy' in catalog['excluded'], 'GOG must remain excluded')
    expected_files = set()
    for entry in entries:
        expected = f"recipes/{entry['id']}.json"
        require(entry['path'] == expected, 'Invalid catalog path')
        path = root / expected
        require(not path.is_symlink(), 'Symlink recipe forbidden')
        require(hashlib.sha256(path.read_bytes()).hexdigest() == entry['sha256'], 'Recipe hash mismatch; review and update index')
        recipe = read(path)
        require(recipe['id'] == entry['id'] and recipe['name'] == entry['name']
                and recipe['recipe_version'] == entry['recipe_version'], 'Index and recipe disagree')
        validate_recipe(recipe, schema, ids)
        expected_files.add(path.name)
    require({p.name for p in (root / 'recipes').glob('*.json')} == expected_files, 'Unindexed recipe')
    return len(entries)


if __name__ == '__main__':
    try:
        count = validate_catalog()
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f'Invalid launcher catalog: {error}', file=sys.stderr)
        sys.exit(1)
    print(f'Validated {count} experimental launcher recipes (offline; no installers executed).')
