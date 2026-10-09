#!/usr/bin/env python3
"""Preserve original dependency notices. Supply cargo metadata from the locked Linux build."""
import argparse, hashlib, json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PREFIXES = ('license', 'licence', 'copying', 'notice', 'copyright')

def notice_files(folder):
    return sorted(p for p in folder.rglob('*') if p.is_file() and p.name.lower().startswith(PREFIXES) and p.stat().st_size < 200_000)

def generate(label, packages, destination):
    documents, rows, missing = {}, [], []
    sections = [f'{label} — original third-party notices\n',
                'Generated from installed, locked dependency versions. Includes transitive and build-time dependencies; this is not a claim that every listed package is shipped in the executable.\n']
    if label == 'Rust dependencies':
        sections.append('Unmodified source archives for the MPL-2.0 crates are provided under that license in the bundled licenses/sources/ directory and at https://github.com/pelicanux/PeliGames/tree/main/licenses/sources . See manifest.json for versions and Cargo.lock-matching checksums. These archives do not cover the native libraries in the AppImage.\n')
    for package, files in sorted(packages, key=lambda item: (item[0]['name'], item[0]['version'])):
        row = dict(package, notices=[])
        sections.append(f"\n{package['name']} {package['version']}\nDeclared license: {package['license']}\nSource: {package['source']}\n")
        if package.get('authors'): sections.append('Authors declared by package: '+', '.join(package['authors'])+'\n')
        for path, filename in files:
            data = path.read_text(errors='replace')
            digest = hashlib.sha256(data.encode()).hexdigest()
            documents.setdefault(digest, data)
            row['notices'].append({'file': filename, 'sha256': digest})
            sections.append(f'Original file: {filename} — document {digest}\n')
        if not files:
            missing.append(package['name'])
            sections.append('NOTICE TEXT NOT FOUND — manual review required; metadata alone is not a substitute for the license.\n')
        rows.append(row)
    sections.append('\nFULL ORIGINAL DOCUMENTS (identical texts deduplicated by SHA-256)\n')
    for digest, data in sorted(documents.items()):
        sections.append(f'\n===== DOCUMENT {digest} =====\n{data}\n')
    destination.write_text(''.join(sections))
    return rows, missing

def main():
    parser = argparse.ArgumentParser();parser.add_argument('metadata', type=Path);args=parser.parse_args()
    metadata = json.loads(args.metadata.read_text())
    rust=[]
    for package in metadata['packages']:
        if package['name'] == 'peligames':continue
        folder=Path(package['manifest_path']).parent
        files=[(path,str(path.relative_to(folder))) for path in notice_files(folder)]
        if not files:
            fallback=ROOT/'licenses/upstream'/package['name']
            files=[(path,str(path.relative_to(ROOT))) for path in notice_files(fallback)] if fallback.exists() else []
        if not files and package['name']=='libappindicator-sys':
            parent=next(p for p in metadata['packages'] if p['name']=='libappindicator')
            folder=Path(parent['manifest_path']).parent
            files=[(path,'libappindicator/'+str(path.relative_to(folder))) for path in notice_files(folder)]
        rust.append(({'name':package['name'],'version':package['version'],'license':package.get('license') or 'UNKNOWN',
                      'source':f"https://crates.io/crates/{package['name']}/{package['version']}",'repository':package.get('repository'),'authors':package.get('authors',[])},files))
    # Only frontend production dependencies and their transitive dependencies.
    pending=list(json.loads((ROOT/'package.json').read_text())['dependencies']);seen=set();frontend=[]
    while pending:
        name=pending.pop()
        if name in seen:continue
        seen.add(name);folder=ROOT/'node_modules'/name
        package=json.loads((folder/'package.json').read_text());pending.extend(package.get('dependencies',{}))
        files=[(path,path.name) for path in sorted(folder.iterdir()) if path.is_file() and path.name.lower().startswith(PREFIXES)]
        # Plugin SPDX summaries do not contain full license terms; API has the same upstream license texts.
        if name.startswith('@tauri-apps/') and not any(path.name.startswith('LICENSE-MIT') for path,_ in files):
            api=ROOT/'node_modules/@tauri-apps/api'
            files.extend((path,'@tauri-apps/api/'+path.name) for path in sorted(api.glob('LICENSE-*')))
        frontend.append(({'name':package['name'],'version':package['version'],'license':package.get('license','UNKNOWN'),
                          'source':f"https://www.npmjs.com/package/{package['name']}/v/{package['version']}"},files))
    output=ROOT/'licenses'
    r,rm=generate('Rust dependencies',rust,output/'Rust-Dependencies.txt')
    f,fm=generate('Frontend production dependencies',frontend,output/'Frontend-Dependencies.txt')
    (output/'dependency-inventory.json').write_text(json.dumps({'rust':r,'frontend':f,'missing_notice_texts':rm+fm},indent=2)+'\n')
    print(f'Rust: {len(r)}; frontend: {len(f)}; missing notice texts: {rm+fm}')
if __name__=='__main__':main()
