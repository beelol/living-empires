"""Deployment safety tests use real Git repositories and an HTTP subprocess."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock
import urllib.error
import urllib.request

RUNNER = Path(__file__).with_name('runner.py')
FAKE_HOST = r'''#!PYTHON
import argparse,json,os,sys
from pathlib import Path
from http.server import BaseHTTPRequestHandler,HTTPServer
p=argparse.ArgumentParser();p.add_argument('command');p.add_argument('--content');p.add_argument('--state');p.add_argument('--bind');p.add_argument('--activate-file');a=p.parse_args()
def content():
    value=json.loads((Path(a.content)/'manifest.json').read_text())
    if value.get('invalid'): raise ValueError('invalid content')
    return value
if a.command=='check-content':
    try: print(json.dumps([] if content().get('validation_nonobject') else {'ok':True,'content_version':content()['version']}))
    except Exception as e: print(str(e),file=sys.stderr);sys.exit(1)
    sys.exit(0)
if content().get('probe_fail'): sys.exit(3)
if content().get('live_fail') and a.bind.endswith(':'+os.environ.get('FAKE_LIVE_PORT','x')):
    if content().get('corrupt_state'): Path(a.state).write_text('{\"world\":0}')
    sys.exit(4)
state=Path(a.state);state.parent.mkdir(parents=True,exist_ok=True)
if not state.exists(): state.write_text('{"world":42}')
def accepting(): return not a.activate_file or Path(a.activate_file).exists()
class H(BaseHTTPRequestHandler):
    def do_POST(self):
        if not accepting(): self.send_response(503);self.end_headers();return
        value=json.loads(state.read_text());value['world']+=1;state.write_text(json.dumps(value))
        self.send_response(200);self.end_headers();self.wfile.write(b'{}')
    def do_GET(self):
        payload={'ok':True,'content_version':content()['version'],'release':os.environ['ARGON_RELEASE'],'pid':os.getpid(),'accepting_commands':accepting(),'env_file':os.environ.get('ARGON_ENV_FILE')}
        if content().get('health_nonobject'): payload=[]
        data=json.dumps(payload).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    def log_message(self,*args): pass
host,port=a.bind.rsplit(':',1);HTTPServer((host,int(port)),H).serve_forever()
'''.replace('PYTHON', sys.executable)


class DeploymentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if RUNNER.exists():
            spec = importlib.util.spec_from_file_location('argon_runner', RUNNER)
            cls.module = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = cls.module
            spec.loader.exec_module(cls.module)

    def setUp(self):
        self.assertTrue(RUNNER.exists(), 'A deployment runner must implement the safety contract')
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / 'repo'
        self.repo.mkdir()
        self.git('init', '-b', 'main')
        self.git('config', 'user.email', 'deploy-test@example.invalid')
        self.git('config', 'user.name', 'Deployment Test')
        self.remote = self.root / 'origin.git'
        subprocess.run(['git', 'init', '--bare', str(self.remote)], check=True, capture_output=True)
        self.git('remote', 'add', 'origin', str(self.remote))
        (self.repo / 'dev/content').mkdir(parents=True)
        (self.repo / 'dev/runtime').mkdir(parents=True)
        (self.repo / 'dev/runtime/host.py').write_text(FAKE_HOST)
        (self.repo / 'Cargo.toml').write_text('[workspace]\n')
        self.first = self.commit('v1')
        self.git('push', 'origin', 'main')
        cargo = self.root / 'cargo'
        cargo.write_text('#!' + sys.executable + '\n' + '''import os,pathlib,shutil,sys,time
p=pathlib.Path.cwd()
if (p/'BUILD_SLEEP').exists(): time.sleep(float((p/'BUILD_SLEEP').read_text()))
if (p/'BUILD_FAIL').exists(): sys.exit(9)
assert sys.argv[1:] == ['build','--locked','-p','living-empires','--bin','empires','--no-default-features'], sys.argv
t=pathlib.Path(os.environ['CARGO_TARGET_DIR'])/'debug'/'empires';t.parent.mkdir(parents=True,exist_ok=True)
shutil.copyfile(p/'dev/runtime/host.py',t);t.chmod(0o755)
''')
        cargo.chmod(0o755)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            self.port = sock.getsockname()[1]
        self.config = self.module.Config(repo=self.repo, deploy_dir=self.root/'deploy',
            bind=f'127.0.0.1:{self.port}', cargo=str(cargo), health_timeout=3, build_timeout=10)
        self.runner = self.module.Runner(self.config)
        self.runner.acquire()
        self.addCleanup(self.runner.close)
        self.old_env = os.environ.get('FAKE_LIVE_PORT')
        os.environ['FAKE_LIVE_PORT'] = str(self.port)
        self.addCleanup(self.restore_env)

    def restore_env(self):
        if self.old_env is None: os.environ.pop('FAKE_LIVE_PORT', None)
        else: os.environ['FAKE_LIVE_PORT'] = self.old_env

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.repo), *args], check=True,
            capture_output=True, text=True).stdout.strip()

    def commit(self, version, **flags):
        (self.repo/'dev/content/manifest.json').write_text(json.dumps({'version': version, **flags}))
        self.git('add', '.'); self.git('commit', '-m', version)
        return self.git('rev-parse', 'HEAD')

    def deploy_first(self):
        self.runner.tick(fetch=True)
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        return self.runner.host.pid

    def test_missing_main_does_not_fall_back_to_master(self):
        self.git('branch', '-m', 'master')
        with self.assertRaisesRegex(self.module.DeployError, 'main'):
            self.runner.select_revision()

    def test_host_loads_env_from_repo_even_when_deployed_from_snapshot(self):
        with mock.patch.dict(os.environ, {}, clear=False):
            os.environ.pop('ARGON_ENV_FILE', None)
            self.deploy_first()
        self.assertEqual(Path(self.runner.health()['env_file']).resolve(), (self.repo/'.env.local').resolve())

    def test_host_preserves_explicit_env_file(self):
        with mock.patch.dict(os.environ, {'ARGON_ENV_FILE':str(self.root/'custom.env')}):
            self.deploy_first()
        self.assertEqual(self.runner.health()['env_file'], str(self.root/'custom.env'))

    def test_ipv6_wildcard_health_uses_ipv6_loopback(self):
        response = mock.MagicMock()
        response.__enter__.return_value.read.return_value = b'{"ok":true}'
        opener = mock.Mock()
        opener.open.return_value = response
        with mock.patch.object(self.module.urllib.request, 'build_opener', return_value=opener):
            self.assertEqual(self.runner.health('[::]:4000'), {'ok':True})
        opener.open.assert_called_once_with('http://[::1]:4000/health', timeout=0.5)

    def test_fetch_and_selection_never_modify_checkout(self):
        self.runner.fetch()
        second = self.commit('v2')
        (self.repo/'dev/content/manifest.json').write_text('uncommitted')
        self.assertEqual(self.runner.select_revision(), second)
        self.assertEqual((self.repo/'dev/content/manifest.json').read_text(), 'uncommitted')
        self.assertEqual(self.git('rev-parse', 'HEAD'), second)

    def test_remote_only_commit_is_selected_without_checkout(self):
        second = self.commit('v2')
        self.git('push', 'origin', 'main')
        self.git('checkout', '--detach', self.first)
        self.git('branch', '-f', 'main', self.first)
        self.runner.fetch()
        self.assertEqual(self.runner.select_revision(), second)
        self.assertEqual(self.git('rev-parse', 'HEAD'), self.first)

    def test_divergent_local_and_remote_heads_are_rejected(self):
        self.runner.fetch()
        remote = self.commit('remote')
        self.git('push', 'origin', 'main')
        self.git('checkout', '-B', 'main', self.first)
        self.commit('local')
        self.runner.fetch()
        with self.assertRaisesRegex(self.module.DeployError, 'diverg'):
            self.runner.select_revision()
        self.assertNotEqual(self.git('rev-parse', 'HEAD'), remote)

    def test_only_one_runner_can_own_a_deployment(self):
        other = self.module.Runner(self.config)
        with self.assertRaisesRegex(self.module.DeployError, 'already running'):
            other.acquire()
        other.close()

    def test_content_reload_keeps_host_pid_and_binary_release(self):
        pid = self.deploy_first()
        second = self.commit('v2')
        self.runner.tick()
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.active.commit, second)
        self.assertEqual(self.runner.health()['content_version'], 'v2')
        self.assertEqual(self.runner.health()['release'], self.first)

    def test_invalid_content_keeps_previous_host_and_version(self):
        pid = self.deploy_first()
        bad = self.commit('bad', invalid=True)
        self.runner.tick()
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.active.commit, self.first)
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        self.assertEqual(self.runner.failed_commit, bad)
        self.assertIn('Content validation failed', self.runner.last_error)

    def test_build_failure_keeps_previous_host_running(self):
        pid = self.deploy_first()
        (self.repo/'BUILD_FAIL').write_text('fail')
        self.commit('v2')
        self.runner.tick()
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')

    def test_malformed_validation_response_keeps_previous_host(self):
        pid = self.deploy_first()
        self.commit('bad-output', validation_nonobject=True)
        self.runner.tick()
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')

    def test_malformed_probe_health_keeps_previous_host(self):
        pid = self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        self.commit('bad-health', health_nonobject=True)
        self.runner.tick()
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')

    def test_failed_candidate_probe_never_stops_live_host(self):
        pid = self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        self.commit('v2', probe_fail=True)
        self.runner.tick()
        self.assertIn('Host exited with code 3', self.runner.last_error)
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')

    def test_failed_live_start_rolls_back_previous_binary_and_content(self):
        self.deploy_first()
        (self.config.deploy_dir/'world.json').write_text('{"world":99}')
        (self.repo/'dev/runtime/change').write_text('new runtime')
        self.commit('v2', live_fail=True, corrupt_state=True)
        self.runner.tick()
        self.assertIn('Live startup failed', self.runner.last_error)
        self.assertEqual(self.runner.active.commit, self.first)
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':99})

    def test_restart_can_retry_a_previously_built_candidate(self):
        self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        second = self.commit('v2', live_fail=True)
        self.runner.tick()
        self.assertEqual(self.runner.active.commit, self.first)
        self.runner.close()
        os.environ['FAKE_LIVE_PORT'] = '0'
        self.runner = self.module.Runner(self.config)
        self.runner.acquire()
        self.addCleanup(self.runner.close)
        self.runner.restore()
        self.runner.tick()
        self.assertEqual(self.runner.active.commit, second)
        self.assertEqual(self.runner.health()['content_version'], 'v2')

    def test_live_candidate_cannot_accept_actions_before_health_promotion(self):
        self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        second = self.commit('v2')
        original = self.runner.wait_healthy
        statuses = []
        def inject_request(process, release, bind=None, **kwargs):
            original(process, release, bind, **kwargs)
            if release.commit == second and bind is None:
                request = urllib.request.Request(f'http://127.0.0.1:{self.port}/mutate', data=b'{}')
                try:
                    with urllib.request.urlopen(request, timeout=2) as response:
                        statuses.append(response.status)
                except urllib.error.HTTPError as error:
                    statuses.append(error.code)
                raise self.module.DeployError('Injected failure after candidate health')
        self.runner.wait_healthy = inject_request
        self.runner.tick()
        self.assertEqual(statuses, [503], 'No action may be acknowledged before promotion')
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':42})
        self.assertEqual(self.runner.active.commit, self.first)

    def test_activation_marker_failure_restores_release_metadata_and_world(self):
        self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        second = self.commit('v2')
        original_touch = Path.touch
        failed = []
        def touch(path, *args, **kwargs):
            if path.parent.name == 'activation' and not failed:
                metadata = json.loads((self.config.deploy_dir/'active.json').read_text())
                if metadata['commit'] == second:
                    failed.append(True)
                    raise OSError('Injected marker creation failure')
            return original_touch(path, *args, **kwargs)
        with mock.patch.object(Path, 'touch', new=touch):
            self.runner.tick()
        self.assertEqual(failed, [True])
        self.assertEqual(self.runner.active.commit, self.first)
        self.assertEqual(json.loads((self.config.deploy_dir/'active.json').read_text())['commit'], self.first)
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        self.assertTrue(self.runner.health()['accepting_commands'])
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':42})

    def test_moving_native_file_to_content_is_not_a_content_only_change(self):
        self.deploy_first()
        self.git('mv', 'dev/runtime/host.py', 'dev/content/host.py')
        second = self.commit('v2')
        self.assertFalse(self.runner.content_only(second), 'Native-source removal must require a build')

    def test_snapshot_uses_committed_content_despite_dirty_checkout(self):
        (self.repo/'dev/content/manifest.json').write_text('dirty invalid JSON')
        self.deploy_first()
        self.assertEqual((self.repo/'dev/content/manifest.json').read_text(), 'dirty invalid JSON')

    def test_active_revision_cannot_rewind(self):
        self.runner.tick(fetch=True)
        self.commit('v2')
        self.runner.tick()
        self.git('checkout', '--detach', self.first)
        self.git('branch', '-f', 'main', self.first)
        self.runner.tick()
        self.assertEqual(self.runner.health()['content_version'], 'v2')
        self.assertIn('rewind', self.runner.last_error)

    def test_crashed_host_restarts_the_last_healthy_release(self):
        pid = self.deploy_first()
        self.runner.host.kill()
        self.runner.host.wait(timeout=3)
        self.runner.tick()
        self.assertNotEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        self.assertEqual(self.runner.active.commit, self.first)

    def test_crashed_host_recovers_before_building_a_new_candidate(self):
        self.deploy_first()
        (self.config.deploy_dir/'world.json').write_text('{"world":99}')
        (self.repo/'BUILD_FAIL').write_text('fail')
        second = self.commit('v2')
        self.runner.host.kill()
        self.runner.host.wait(timeout=3)
        self.runner.tick()
        self.assertIsNone(self.runner.host.poll(), 'Recover the healthy world before preparing newer work')
        self.assertTrue(self.runner.health()['accepting_commands'])
        self.assertEqual(self.runner.active.commit, self.first)
        self.assertIsNone(self.runner.failed_commit, 'The new candidate must remain pending during recovery')
        self.assertEqual(self.runner.select_revision(), second)
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':99})
        recovered_pid = self.runner.host.pid
        self.runner.tick()
        self.assertEqual(self.runner.failed_commit, second)
        self.assertIn('Build failed', self.runner.last_error)
        self.assertEqual(self.runner.host.pid, recovered_pid)
        self.assertTrue(self.runner.health()['accepting_commands'])
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':99})

    def test_new_candidate_can_restore_service_when_old_release_recovery_fails(self):
        self.deploy_first()
        (self.repo/'dev/runtime/change').write_text('new runtime')
        second = self.commit('v2')
        self.runner.host.kill()
        self.runner.host.wait(timeout=3)
        original_touch = Path.touch
        failures = []
        def touch(path, *args, **kwargs):
            if path.parent.name == 'activation' and not failures:
                metadata = json.loads((self.config.deploy_dir/'active.json').read_text())
                if metadata['commit'] == self.first:
                    failures.append(True)
                    raise OSError('Injected old-release recovery failure')
            return original_touch(path, *args, **kwargs)
        with mock.patch.object(Path, 'touch', new=touch):
            self.runner.tick()
        self.assertEqual(failures, [True], 'Try restoring the old release before preparing new work')
        self.assertEqual(self.runner.active.commit, second)
        self.assertTrue(self.runner.health()['accepting_commands'])
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':42})

    def test_recovery_marker_failure_stops_gated_host_and_retries(self):
        self.deploy_first()
        self.runner.host.kill()
        self.runner.host.wait(timeout=3)
        original_touch = Path.touch
        failed_hosts = []
        def touch(path, *args, **kwargs):
            if path.parent.name == 'activation' and not failed_hosts:
                failed_hosts.append(self.runner.host)
                raise OSError('Injected recovery marker failure')
            return original_touch(path, *args, **kwargs)
        with mock.patch.object(Path, 'touch', new=touch):
            self.runner.tick()
        self.assertEqual(len(failed_hosts), 1)
        self.assertIsNone(self.runner.host, 'A failed recovery must not strand a gated live process')
        self.assertIsNotNone(failed_hosts[0].poll(), 'Failed recovery process must be stopped')
        self.assertEqual(self.runner.phase, 'blocked')
        self.assertIn('Injected recovery marker failure', self.runner.last_error)
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':42})
        deadline = time.monotonic() + 4
        while self.runner.host is None and time.monotonic() < deadline:
            self.runner.tick()
            time.sleep(.05)
        self.assertIsNotNone(self.runner.host, 'The next bounded recovery attempt must run')
        self.assertTrue(self.runner.health()['accepting_commands'])
        self.assertEqual(self.runner.active.commit, self.first)
        request = urllib.request.Request(f'http://127.0.0.1:{self.port}/mutate', data=b'{}')
        with urllib.request.urlopen(request, timeout=2) as response:
            self.assertEqual(response.status, 200)
        self.assertEqual(json.loads((self.config.deploy_dir/'world.json').read_text()), {'world':43})

    def test_crashed_host_recovers_even_when_heads_diverge(self):
        self.deploy_first()
        self.commit('remote')
        self.git('push', 'origin', 'main')
        self.git('checkout', '-B', 'main', self.first)
        self.commit('local')
        self.runner.fetch()
        self.runner.host.kill()
        self.runner.host.wait(timeout=3)
        self.runner.tick()
        self.assertIsNone(self.runner.host.poll(), 'A bad candidate must not prevent recovery of the healthy release')
        self.assertEqual(self.runner.health()['content_version'], 'v1')
        self.assertIn('diverg', self.runner.last_error)

    def test_worktrees_share_deployment_directory(self):
        worktree = self.root/'agent-worktree'
        self.git('worktree', 'add', '-b', 'agent-one', str(worktree))
        self.assertEqual(self.module.default_deploy_dir(self.repo), self.module.default_deploy_dir(worktree))

    def test_stop_request_cancels_build_without_replacing_live_host(self):
        pid = self.deploy_first()
        (self.repo/'BUILD_SLEEP').write_text('30')
        self.commit('v2')
        thread = threading.Thread(target=self.runner.tick)
        thread.start()
        deadline = time.monotonic()+3
        while self.runner.phase != 'building' and time.monotonic() < deadline:
            time.sleep(.02)
        (self.config.deploy_dir/'stop').touch()
        thread.join(timeout=3)
        self.assertFalse(thread.is_alive(), 'Stop must interrupt an in-progress build')
        self.assertEqual(self.runner.host.pid, pid)
        self.assertEqual(self.runner.health()['content_version'], 'v1')

    def test_hook_install_preserves_existing_hook(self):
        hooks = self.repo/'.git/hooks'; hooks.mkdir(exist_ok=True)
        hook = hooks/'post-commit'; hook.write_text('#!/bin/sh\necho existing\n')
        with self.assertRaisesRegex(self.module.DeployError, 'existing'):
            self.module.install_hook(self.repo)
        self.assertEqual(hook.read_text(), '#!/bin/sh\necho existing\n')

    def test_hook_install_is_idempotent_and_wakes_shared_directory(self):
        self.module.install_hook(self.repo)
        self.module.install_hook(self.repo)
        self.commit('v2')
        self.assertTrue((self.repo/'.local-deploy/wake').exists())


if __name__ == '__main__':
    unittest.main()
