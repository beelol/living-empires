#!/usr/bin/env python3
"""Measure real commit deployment in a disposable local Git repository."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

SOURCE = Path(__file__).resolve().parents[1]
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def command(*args, cwd=None):
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed: {result.stderr[-2000:]}')
    return result.stdout.strip()


def git(repo, *args):
    return command('git', '-C', str(repo), *args)


def commit(repo, message):
    git(repo, 'add', '.')
    git(repo, '-c', 'user.name=Argon smoke test', '-c', 'user.email=smoke@example.invalid',
        'commit', '-m', message)
    return git(repo, 'rev-parse', 'HEAD')


def eventually(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except (OSError, ValueError, KeyError) as exc:
            last = exc
        time.sleep(.025)
    raise AssertionError(f'Timed out waiting for observable result: {last}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--target-cache', type=Path, help='Reuse an existing Cargo target directory; initial timing is then warm')
    args = parser.parse_args()
    timings = {}
    with tempfile.TemporaryDirectory(prefix='argon-smoke-') as temporary:
        root = Path(temporary)
        repo, remote, deployed = root/'repo', root/'remote.git', root/'deployed'
        repo.mkdir()
        for name in ['Cargo.toml', 'Cargo.lock', '.gitignore', 'rust-toolchain.toml']:
            shutil.copy2(SOURCE/name, repo/name)
        shutil.copytree(SOURCE/'dev', repo/'dev', ignore=shutil.ignore_patterns('__pycache__'))
        shutil.copytree(SOURCE/'game', repo/'game')
        if args.target_cache:
            deployed.mkdir()
            (deployed/'target').symlink_to(args.target_cache.resolve(), target_is_directory=True)
        git(repo, 'init', '-b', 'main')
        initial = commit(repo, 'Initial real runtime')
        command('git', 'init', '--bare', str(remote))
        git(repo, 'remote', 'add', 'origin', str(remote))
        git(repo, 'push', '-u', 'origin', 'main')
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        url = f'http://127.0.0.1:{port}'

        def request(path, body=None):
            data = None if body is None else json.dumps(body).encode()
            req = urllib.request.Request(url+path, data=data,
                headers={'Content-Type': 'application/json'})
            with OPENER.open(req, timeout=1) as response:
                return json.load(response)

        def status():
            return json.loads((deployed/'status.json').read_text())

        def active(revision):
            value = status()
            return value if value.get('phase') == 'serving' and (value.get('active') or {}).get('commit') == revision else None

        def client(name, moves=()):
            binary = status()['active']['binary']
            arguments = [binary, 'probe-client', f'127.0.0.1:{port}', '--session', str(root/f'{name}.json')]
            for dx, dy in moves:
                arguments.extend(['--move', f'{dx},{dy}'])
            return json.loads(command(*arguments))

        logs = root/'runner.log'
        with logs.open('w') as output:
            process = subprocess.Popen([sys.executable, str(SOURCE/'dev/runner.py'), 'run',
                '--repo', str(repo), '--deploy-dir', str(deployed), '--bind', f'127.0.0.1:{port}',
                '--fetch-interval', '0.3', '--poll-interval', '0.1'], stdout=output, stderr=subprocess.STDOUT)
        try:
            started = time.monotonic()
            first = eventually(lambda: active(initial), 600)
            timings['initial_warm_build_seconds' if args.target_cache else 'initial_cold_build_seconds'] = round(time.monotonic()-started, 3)
            a, b = client('a'), client('b')
            moved = client('a', [(1,0)])
            assert len(moved['players']) == 2
            assert moved['player']['x'] == a['player']['x'] + 12
            original_pid = first['host_pid']
            original_version = request('/health')['content_version']

            # Uncommitted source must never leak into the running main deployment.
            rules = repo/'dev/content/rules.lua'
            original_rules = rules.read_text()
            rules.write_text(original_rules.replace('local distance = 12', 'local distance = 13'))
            time.sleep(.4)
            assert request('/health')['content_version'] == original_version
            started = time.monotonic()
            # Distinct declared modules must survive validation and hot activation.
            # The names differ only by a repeated extension: strip one suffix.
            (repo/'dev/content/speed.lua').write_text('return 3\n')
            (repo/'dev/content/speed.lua.lua').write_text('return 10\n')
            manifest_path = repo/'dev/content/manifest.json'
            manifest = json.loads(manifest_path.read_text())
            manifest['files'].extend(['speed.lua', 'speed.lua.lua'])
            manifest_path.write_text(json.dumps(manifest)+'\n')
            rules.write_text(original_rules.replace('local distance = 12',
                "local distance = require('speed') + require('speed.lua')"))
            revision = commit(repo, 'Lua module step 13')
            changed = eventually(lambda: active(revision))
            timings['local_lua_commit_seconds'] = round(time.monotonic()-started, 3)
            assert changed['host_pid'] == original_pid
            assert request('/health')['content_version'] != original_version
            resumed = client('a')
            assert resumed['player'] == moved['player']
            moved2 = client('a', [(1,0)])
            assert moved2['player']['x'] == moved['player']['x'] + 13

            rules.write_text('this is invalid Lua\n')
            invalid = commit(repo, 'Invalid candidate must not deploy')
            rejected = eventually(lambda: status() if status().get('failed_commit') == invalid else None)
            assert rejected['active']['commit'] == revision
            assert rejected['host_pid'] == original_pid
            assert client('a')['player'] == moved2['player']

            rules.write_text(original_rules.replace('local distance = 12', 'local distance = 14'))
            repaired = commit(repo, 'Repair candidate')
            eventually(lambda: active(repaired))
            # Real native rebuild; comment intentionally changes only compilation inputs.
            with (repo/'game/src/lib.rs').open('a') as native:
                native.write('\n// Smoke: changed native source requires a rebuild.\n')
            started = time.monotonic()
            native_revision = commit(repo, 'Native rebuild')
            native_status = eventually(lambda: active(native_revision), 120)
            timings['warm_native_commit_seconds'] = round(time.monotonic()-started, 3)
            assert native_status['host_pid'] != original_pid
            resumed = client('a')
            assert resumed['player'] == moved2['player']
            assert len(resumed['players']) == 2

            # Simulate a second developer's push without advancing the local branch.
            git(repo, 'push', 'origin', 'main')
            other = root/'other'
            command('git', 'clone', '--branch', 'main', str(remote), str(other))
            other_rules = other/'dev/content/rules.lua'
            other_rules.write_text(original_rules.replace('local distance = 12', 'local distance = 15'))
            remote_revision = commit(other, 'Remote Lua step 15')
            started = time.monotonic()
            git(other, 'push', 'origin', 'main')
            remote_status = eventually(lambda: active(remote_revision))
            timings['remote_lua_commit_seconds_at_300ms_fetch'] = round(time.monotonic()-started, 3)
            assert remote_status['host_pid'] == native_status['host_pid']
            assert git(repo, 'rev-parse', 'HEAD') == native_revision
            assert client('b')['player'] == b['player']
            timings['checks'] = ['two Lightyear UDP sessions', 'Lua rules affect movement', 'dirty edits isolated',
                'Lua module deploy without restart', 'bad Lua retains healthy host', 'repaired commit activates',
                'native restart preserves world', 'remote commit deploys without editing checkout']
            print(json.dumps(timings, indent=2))
            if args.output:
                args.output.parent.mkdir(parents=True, exist_ok=True)
                args.output.write_text(json.dumps(timings, indent=2)+'\n')
        except BaseException:
            print(logs.read_text()[-6000:], file=sys.stderr)
            for path in sorted((deployed/'logs').glob('*.log')) if (deployed/'logs').exists() else []:
                print(f'{path.name}:\n{path.read_text()[-1500:]}', file=sys.stderr)
            raise
        finally:
            process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            # Snapshots are read-only by design; make only this disposable test tree removable.
            for directory, _, files in os.walk(root):
                Path(directory).chmod(0o700)
                for name in files:
                    path = Path(directory)/name
                    if not path.is_symlink():
                        path.chmod(0o600)


if __name__ == '__main__':
    main()
