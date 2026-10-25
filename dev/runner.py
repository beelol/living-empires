#!/usr/bin/env python3
"""Deploy committed revisions without touching an agent's checkout (Python stdlib)."""
from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import fcntl
import io
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid


class DeployError(RuntimeError):
    pass


def git(repo: Path, *args: str, check: bool = True) -> str:
    result = subprocess.run(['git', '-C', str(repo), *args], capture_output=True,
                            text=True, timeout=20)
    if check and result.returncode:
        raise DeployError(result.stderr.strip() or 'Git command failed')
    return result.stdout.strip() if result.returncode == 0 else ''


def common_dir(repo: Path) -> Path:
    return Path(git(repo, 'rev-parse', '--path-format=absolute', '--git-common-dir'))


def default_deploy_dir(repo: Path) -> Path:
    return common_dir(repo).parent / '.local-deploy'


def atomic_json(path: Path, value: dict) -> None:
    temp = path.with_suffix('.tmp')
    temp.write_text(json.dumps(value, indent=2) + '\n')
    os.replace(temp, path)


@dataclass
class Config:
    repo: Path
    deploy_dir: Path
    branch: str = 'main'
    remote: str = 'origin'
    bind: str = '0.0.0.0:4000'
    cargo: str = 'cargo'
    poll_interval: float = 0.25
    fetch_interval: float = 3.0
    health_timeout: float = 10.0
    build_timeout: float = 600.0


@dataclass
class Release:
    commit: str
    binary: str
    content: str
    content_version: str
    binary_revision: str


class Runner:
    def __init__(self, config: Config):
        self.config = config
        self.config.repo = self.config.repo.resolve()
        self.config.deploy_dir = self.config.deploy_dir.resolve()
        self.root = self.config.deploy_dir
        self.lock = None
        self.active: Release | None = None
        self.host: subprocess.Popen | None = None
        self.failed_commit: str | None = None
        self.last_error: str | None = None
        self.fetch_error: str | None = None
        self.phase = 'starting'
        self.started_at = time.time()
        self.stopping = threading.Event()
        self.fetch_ready = threading.Event()
        self.activation_files: dict[int, Path] = {}
        self.remote_ref = f'refs/argon-deploy/{config.remote}/{config.branch}'
        self.last_good_commit = None
        self.restart_attempts = 0
        self.next_restart_at = 0.0
        self.healthy_since = time.monotonic()
        self._validate_config()

    def _validate_config(self):
        result = subprocess.run(['git', 'check-ref-format', f'refs/heads/{self.config.branch}'], capture_output=True)
        if result.returncode:
            raise DeployError('Invalid branch name')
        if not self.config.remote or self.config.remote.startswith('-') or '/' in self.config.remote:
            raise DeployError('Remote must be a configured Git remote name')
        if self.config.poll_interval <= 0 or self.config.fetch_interval <= 0:
            raise DeployError('Poll and fetch intervals must be positive')
        try:
            hostname, port = self.config.bind.rsplit(':', 1)
            if not hostname or not (0 < int(port) < 65536):
                raise ValueError()
        except ValueError as exc:
            raise DeployError('--bind must be HOST:PORT with a nonzero port') from exc

    def acquire(self):
        self.root.mkdir(parents=True, exist_ok=True)
        handle = (self.root/'runner.lock').open('a+')
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            handle.close()
            raise DeployError(f'A deployment runner is already running in {self.root}') from exc
        self.lock = handle
        (self.root/'stop').unlink(missing_ok=True)
        self.write_status()

    def log(self, message):
        print(time.strftime('%Y-%m-%d %H:%M:%S') + ' ' + message, flush=True)

    def write_status(self):
        if self.lock is None:
            return
        atomic_json(self.root/'status.json', {
            'pid': os.getpid(), 'host_pid': self.host.pid if self.host and self.host.poll() is None else None,
            'phase': self.phase, 'branch': self.config.branch, 'remote': self.config.remote,
            'bind': self.config.bind, 'active': asdict(self.active) if self.active else None,
            'failed_commit': self.failed_commit, 'last_error': self.last_error,
            'fetch_error': self.fetch_error, 'restart_attempts': self.restart_attempts, 'started_at': self.started_at, 'updated_at': time.time(),
        })

    def fetch(self):
        """Fetch only into our private ref; never update a checkout or local branch."""
        result = subprocess.run(['git', '-C', str(self.config.repo), 'fetch', '--no-tags',
            '--no-write-fetch-head', self.config.remote,
            f'+refs/heads/{self.config.branch}:{self.remote_ref}'],
            capture_output=True, text=True, timeout=20)
        if result.returncode:
            raise DeployError(result.stderr.strip() or f'Cannot fetch {self.config.remote}/{self.config.branch}')

    def ancestor(self, older: str, newer: str) -> bool:
        result = subprocess.run(['git', '-C', str(self.config.repo), 'merge-base', '--is-ancestor', older, newer],
                                capture_output=True)
        if result.returncode not in (0, 1):
            raise DeployError(result.stderr.decode().strip() or 'Cannot compare revision ancestry')
        return result.returncode == 0

    def select_revision(self) -> str:
        local = git(self.config.repo, 'rev-parse', '--verify', f'refs/heads/{self.config.branch}^{{commit}}', check=False)
        remote = git(self.config.repo, 'rev-parse', '--verify', f'{self.remote_ref}^{{commit}}', check=False)
        if not local and not remote:
            raise DeployError(f'No local or fetched {self.config.remote}/{self.config.branch}; create/push {self.config.branch} or pass --branch explicitly')
        if not local or not remote or local == remote:
            selected = local or remote
        elif self.ancestor(local, remote):
            selected = remote
        elif self.ancestor(remote, local):
            selected = local
        else:
            raise DeployError(f'Local {self.config.branch} and {self.config.remote}/{self.config.branch} diverge; merge or fast-forward them before deployment')
        previous = self.active.commit if self.active else self.last_good_commit
        if previous and not self.ancestor(previous, selected):
            raise DeployError('Refusing to rewind or diverge from the last healthy deployed commit')
        return selected

    def snapshot(self, revision: str) -> Path:
        parent = self.root/'snapshots'
        parent.mkdir(exist_ok=True)
        destination = parent/revision
        if destination.exists():
            return destination
        temporary = Path(tempfile.mkdtemp(prefix=revision+'.', dir=parent))
        result = subprocess.run(['git', '-C', str(self.config.repo), 'archive', '--format=tar', revision],
                                capture_output=True, timeout=30)
        if result.returncode:
            raise DeployError(result.stderr.decode().strip() or 'Snapshot failed')
        # Refuse links and special files so an archive cannot escape its private directory.
        with tarfile.open(fileobj=io.BytesIO(result.stdout)) as archive:
            for member in archive.getmembers():
                if member.name.startswith('/') or '..' in Path(member.name).parts or not (member.isdir() or member.isfile()):
                    raise DeployError(f'Unsupported snapshot entry: {member.name}')
                archive.extract(member, temporary, set_attrs=False)
                if member.isfile():
                    (temporary/member.name).chmod(0o555 if member.mode & 0o111 else 0o444)
        for directory, _, _ in os.walk(temporary, topdown=False):
            Path(directory).chmod(0o555)
        os.rename(temporary, destination)
        return destination

    def content_only(self, revision: str) -> bool:
        if not self.active:
            return False
        paths = git(self.config.repo, 'diff', '--name-only', '--no-renames', self.active.commit, revision, '--').splitlines()
        return bool(paths) and all(path.startswith('dev/content/') for path in paths)

    def build(self, revision: str, snapshot: Path) -> Path:
        destination = self.root/'releases'/revision/'empires'
        if destination.is_file():
            return destination
        self.phase = 'building'
        self.write_status()
        logs = self.root/'logs'
        logs.mkdir(exist_ok=True)
        target = self.root/'target'
        with (logs/f'build-{revision}.log').open('ab') as output:
            env = dict(os.environ, CARGO_TARGET_DIR=str(target))
            process = subprocess.Popen([self.config.cargo, 'build', '--locked', '-p', 'living-empires', '--bin', 'empires', '--no-default-features'],
                cwd=snapshot, env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            deadline = time.monotonic() + self.config.build_timeout
            try:
                while process.poll() is None:
                    if self.stopping.is_set() or (self.root/'stop').exists():
                        raise DeployError('Build cancelled by stop request')
                    if time.monotonic() >= deadline:
                        raise DeployError(f'Build timed out; see {logs/f"build-{revision}.log"}')
                    self.stopping.wait(0.05)
            finally:
                self.terminate(process)
        if process.returncode:
            raise DeployError(f'Build failed; see {logs/f"build-{revision}.log"}')
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_suffix('.next')
        temporary.unlink(missing_ok=True)
        shutil.copy2(target/'debug/empires', temporary)
        temporary.chmod(0o555)
        os.replace(temporary, destination)
        return destination

    def validate(self, binary: Path, content: Path) -> str:
        try:
            result = subprocess.run([str(binary), 'check-content', '--content', str(content)],
                capture_output=True, text=True, timeout=self.config.health_timeout)
            value = json.loads(result.stdout) if result.returncode == 0 else {}
            if result.returncode or not isinstance(value, dict) or value.get('ok') is not True or not value.get('content_version'):
                raise DeployError('Content validation failed: ' + (result.stderr.strip() or result.stdout.strip()))
            return str(value['content_version'])
        except (subprocess.TimeoutExpired, json.JSONDecodeError) as exc:
            raise DeployError(f'Content validation did not return healthy JSON: {exc}') from exc

    def swap_content(self, content: str):
        temporary = self.root/'current-content.next'
        temporary.unlink(missing_ok=True)
        temporary.symlink_to(content, target_is_directory=True)
        os.replace(temporary, self.root/'current-content')

    def spawn(self, release: Release, bind: str, state: Path, content: str) -> subprocess.Popen:
        logs = self.root/'logs'
        logs.mkdir(exist_ok=True)
        gates = self.root/'activation'
        gates.mkdir(exist_ok=True)
        gate = gates/uuid.uuid4().hex
        with (logs/f'host-{release.binary_revision}.log').open('ab') as output:
            process = subprocess.Popen([release.binary, 'host', '--bind', bind, '--content', content,
                '--state', str(state), '--activate-file', str(gate)],
                env=dict(os.environ, ARGON_RELEASE=release.binary_revision,
                    ARGON_ENV_FILE=os.environ.get('ARGON_ENV_FILE', str(self.config.repo/'.env.local'))),
                stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        self.activation_files[process.pid] = gate
        return process

    def activate_host(self, process: subprocess.Popen, release: Release):
        # Persist the selected release before allowing any acknowledged player writes.
        atomic_json(self.root/'active.json', asdict(release))
        gate = self.activation_files[process.pid]
        try:
            gate.touch(mode=0o600, exist_ok=False)
        except OSError:
            if not gate.is_file():
                raise
            # The gate becoming visible is the commit point, even if touching it
            # reported an error afterward. Never restore a pre-activation save.
        self.active = release
        self.last_good_commit = release.commit

    def health(self, bind: str | None = None) -> dict:
        host, port = (bind or self.config.bind).rsplit(':', 1)
        if host == '0.0.0.0':
            host = '127.0.0.1'
        elif host in ('::', '[::]'):
            host = '[::1]'
        # Local health requests must never be routed through an HTTP proxy.
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(f'http://{host}:{port}/health', timeout=0.5) as response:
            return json.load(response)

    def wait_healthy(self, process: subprocess.Popen, release: Release, bind: str | None = None,
                     accepting_commands: bool | None = None):
        deadline = time.monotonic() + self.config.health_timeout
        error = 'health endpoint unavailable'
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise DeployError(f'Host exited with code {process.returncode}')
            try:
                value = self.health(bind)
                if (isinstance(value, dict) and value.get('ok') is True and value.get('content_version') == release.content_version
                        and value.get('release') == release.binary_revision and process.poll() is None
                        and (accepting_commands is None or value.get('accepting_commands') is accepting_commands)):
                    return
                error = 'host has not activated the expected content and binary version'
            except (OSError, urllib.error.URLError, json.JSONDecodeError) as exc:
                error = str(exc)
            time.sleep(0.05)
        raise DeployError(f'Health check timed out: {error}')

    @staticmethod
    def terminate(process: subprocess.Popen | None):
        if process is not None and process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            except ProcessLookupError:
                process.wait(timeout=5)

    def probe(self, release: Release):
        self.phase = 'probing'
        self.write_status()
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            bind = f'127.0.0.1:{sock.getsockname()[1]}'
        probe_dir = Path(tempfile.mkdtemp(prefix=release.commit[:12]+'-', dir=self.root))
        probe_state = probe_dir/'world.json'
        live_state = self.root/'world.json'
        if live_state.exists():
            shutil.copy2(live_state, probe_state)
        process = self.spawn(release, bind, probe_state, release.content)
        try:
            self.wait_healthy(process, release, bind, accepting_commands=False)
        finally:
            self.terminate(process)
            shutil.rmtree(probe_dir)

    def promote(self, release: Release, hot_reload: bool):
        previous = self.active
        if self.stopping.is_set() or (self.root/'stop').exists():
            raise DeployError('Promotion cancelled by stop request')
        if self.select_revision() != release.commit:
            raise DeployError('Candidate was superseded; preparing the newest commit next')
        if hot_reload:
            self.phase = 'reloading-content'
            self.write_status()
            self.swap_content(release.content)
            try:
                self.wait_healthy(self.host, release, accepting_commands=True)
                atomic_json(self.root/'active.json', asdict(release))
            except (DeployError, OSError):
                self.swap_content(previous.content)
                self.wait_healthy(self.host, previous, accepting_commands=True)
                raise
            self.active = release
            self.last_good_commit = release.commit
        else:
            self.probe(release)
            # Builds can take time: do not knowingly activate an obsolete candidate.
            if self.select_revision() != release.commit:
                raise DeployError('Candidate was superseded during build; preparing the newest commit next')
            self.phase = 'restarting-host'
            self.write_status()
            self.terminate(self.host)
            self.host = None
            state = self.root/'world.json'
            backup = self.root/'world-before-restart.json'
            had_state = state.exists()
            if had_state:
                shutil.copy2(state, backup)
            self.swap_content(release.content)
            try:
                self.host = self.spawn(release, self.config.bind, state, str(self.root/'current-content'))
                self.wait_healthy(self.host, release, accepting_commands=False)
                self.activate_host(self.host, release)
            except (DeployError, OSError) as exc:
                self.terminate(self.host)
                self.host = None
                # The failed host was gated throughout validation, so it could not
                # acknowledge commands whose durable state this restore would lose.
                if had_state:
                    os.replace(backup, state)
                else:
                    state.unlink(missing_ok=True)
                if previous:
                    self.swap_content(previous.content)
                    self.host = self.spawn(previous, self.config.bind, state, str(self.root/'current-content'))
                    self.wait_healthy(self.host, previous, accepting_commands=False)
                    self.activate_host(self.host, previous)
                else:
                    (self.root/'active.json').unlink(missing_ok=True)
                raise DeployError(f'Live startup failed; previous release restored when available: {exc}') from exc
            # Once activation succeeds, no later bookkeeping failure may restore
            # the backup: this host may already have acknowledged player commands.
            try:
                backup.unlink(missing_ok=True)
            except OSError as exc:
                self.log(f'Keeping unused restart backup: {exc}')
        self.restart_attempts = 0
        self.healthy_since = time.monotonic()
        self.failed_commit = None
        self.last_error = None
        self.phase = 'serving'
        self.write_status()

    def deploy(self, revision: str):
        started = time.monotonic()
        self.log(f'Preparing {revision[:12]}')
        snapshot = self.snapshot(revision)
        hot_reload = self.content_only(revision) and self.host is not None and self.host.poll() is None
        binary = Path(self.active.binary) if hot_reload else self.build(revision, snapshot)
        version = self.validate(binary, snapshot/'dev/content')
        release = Release(revision, str(binary), str(snapshot/'dev/content'), version,
                          self.active.binary_revision if hot_reload else revision)
        self.promote(release, hot_reload)
        action = 'Content activated without restart' if hot_reload else 'Host activated'
        self.log(f'{action}: {revision[:12]} ({time.monotonic()-started:.2f}s)')

    def tick(self, fetch=False):
        revision = None
        try:
            if fetch:
                try:
                    self.fetch()
                    self.fetch_error = None
                except (DeployError, subprocess.TimeoutExpired) as exc:
                    self.fetch_error = str(exc)
            revision = self.select_revision()
            if self.active and (self.host is None or self.host.poll() is not None):
                try:
                    self.recover_host()
                except (DeployError, OSError) as recovery_error:
                    if revision == self.active.commit or revision == self.failed_commit:
                        raise
                    self.log(f'Host recovery failed; trying the new candidate: {recovery_error}')
                else:
                    self.write_status()
                    return
            elif time.monotonic() - self.healthy_since >= 30:
                self.restart_attempts = 0
            if revision != (self.active.commit if self.active else None) and revision != self.failed_commit:
                self.deploy(revision)
            elif self.active:
                self.phase = 'serving-with-error' if self.last_error else 'serving'
        except (DeployError, OSError, subprocess.TimeoutExpired) as exc:
            message = str(exc)
            if message != self.last_error:
                self.log('Deployment held: ' + message)
            self.last_error = message
            # Invalid refs cannot strand a previously healthy world after a host crash.
            if revision is None and self.active and (self.host is None or self.host.poll() is not None):
                try:
                    self.recover_host()
                except (DeployError, OSError) as recovery_error:
                    self.last_error += '; host recovery: ' + str(recovery_error)
            if revision:
                self.failed_commit = revision
            self.phase = 'serving-with-error' if self.active and self.host and self.host.poll() is None else 'blocked'
        self.write_status()

    def recover_host(self):
        if self.restart_attempts >= 5:
            raise DeployError('Host repeatedly crashed; automatic restart limit reached (5). Inspect logs and restart the runner')
        if time.monotonic() < self.next_restart_at:
            self.phase = 'waiting-to-restart'
            return
        self.restart_attempts += 1
        self.next_restart_at = time.monotonic() + min(30, 2 ** (self.restart_attempts - 1))
        self.phase = 'recovering-host'
        self.write_status()
        self.swap_content(self.active.content)
        self.host = self.spawn(self.active, self.config.bind, self.root/'world.json', str(self.root/'current-content'))
        try:
            self.wait_healthy(self.host, self.active, accepting_commands=False)
            self.activate_host(self.host, self.active)
        except (DeployError, OSError):
            self.terminate(self.host)
            self.host = None
            raise
        self.healthy_since = time.monotonic()
        self.phase = 'serving-with-error' if self.last_error else 'serving'
        self.log(f'Restarted last healthy host (attempt {self.restart_attempts}/5)')

    def restore(self):
        path = self.root/'active.json'
        if not path.exists():
            return
        previous = Release(**json.loads(path.read_text()))
        self.last_good_commit = previous.commit
        self.validate(Path(previous.binary), Path(previous.content))
        self.swap_content(previous.content)
        self.host = self.spawn(previous, self.config.bind, self.root/'world.json', str(self.root/'current-content'))
        self.wait_healthy(self.host, previous, accepting_commands=False)
        self.activate_host(self.host, previous)
        self.phase = 'serving'
        self.write_status()

    def fetch_loop(self):
        while not self.stopping.is_set():
            try:
                self.fetch()
                self.fetch_error = None
            except (DeployError, OSError, subprocess.TimeoutExpired) as exc:
                self.fetch_error = str(exc)
            self.fetch_ready.set()
            self.stopping.wait(self.config.fetch_interval)

    def wait_for_change(self):
        def wake_stamp():
            try:
                return (self.root/'wake').stat().st_mtime_ns
            except FileNotFoundError:
                return None
        initial = wake_stamp()
        deadline = time.monotonic() + self.config.poll_interval
        while time.monotonic() < deadline and not self.stopping.is_set():
            if (self.root/'stop').exists() or wake_stamp() != initial:
                return
            self.stopping.wait(min(0.05, max(0, deadline-time.monotonic())))

    def run(self):
        self.acquire()
        try:
            self.restore()
            thread = threading.Thread(target=self.fetch_loop, name='git-fetch', daemon=True)
            thread.start()
            while not self.stopping.is_set() and not (self.root/'stop').exists():
                if self.fetch_ready.is_set():
                    self.tick()
                else:
                    self.phase = 'serving' if self.active else 'checking-remote'
                    self.write_status()
                self.wait_for_change()
        finally:
            self.close()

    def close(self):
        self.stopping.set()
        self.terminate(self.host)
        self.host = None
        if self.lock is not None:
            self.phase = 'stopped'
            self.write_status()
            fcntl.flock(self.lock, fcntl.LOCK_UN)
            self.lock.close()
            self.lock = None


def install_hook(repo: Path):
    configured = git(repo, 'config', '--get', 'core.hooksPath', check=False)
    if configured:
        raise DeployError(f'An existing core.hooksPath is configured ({configured}); keep it and use polling or chain dev/hooks/post-commit manually')
    directory = common_dir(repo)/'hooks'
    directory.mkdir(exist_ok=True)
    destination = directory/'post-commit'
    template = Path(__file__).with_name('hooks')/'post-commit'
    source = template.read_text()
    if destination.exists() and destination.read_text() != source:
        raise DeployError(f'Refusing to overwrite existing hook: {destination}; polling already works')
    destination.write_text(source)
    destination.chmod(0o755)
    return destination


def lock_held(root: Path) -> bool:
    path = root/'runner.lock'
    if not path.exists():
        return False
    with path.open('a+') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        fcntl.flock(handle, fcntl.LOCK_UN)
        return False


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['run', 'start', 'status', 'stop', 'install-hook'])
    parser.add_argument('--repo', type=Path, default=Path.cwd())
    parser.add_argument('--deploy-dir', type=Path)
    parser.add_argument('--branch', default='main')
    parser.add_argument('--remote', default='origin')
    parser.add_argument('--bind', default='0.0.0.0:4000')
    parser.add_argument('--cargo', default='cargo')
    parser.add_argument('--poll-interval', type=float, default=0.25)
    parser.add_argument('--fetch-interval', type=float, default=3)
    parser.add_argument('--health-timeout', type=float, default=10)
    parser.add_argument('--build-timeout', type=float, default=600)
    args = parser.parse_args(argv)
    try:
        root = args.deploy_dir or default_deploy_dir(args.repo)
        root = root.resolve()
        if args.command == 'install-hook':
            print(install_hook(args.repo))
            return 0
        if args.command == 'status':
            path = root/'status.json'
            status = json.loads(path.read_text()) if path.exists() else {'phase': 'not-started'}
            status['runner_running'] = lock_held(root)
            print(json.dumps(status, indent=2))
            return 0
        if args.command == 'stop':
            if not lock_held(root):
                print('Runner is already stopped')
                return 0
            (root/'stop').touch()
            deadline = time.monotonic()+15
            while lock_held(root) and time.monotonic() < deadline:
                time.sleep(0.1)
            if lock_held(root):
                raise DeployError('Stop requested; runner will stop after its current build/check finishes')
            print('Runner stopped')
            return 0
        if args.command == 'start':
            if lock_held(root):
                raise DeployError('Deployment runner is already running')
            root.mkdir(parents=True, exist_ok=True)
            forwarded = list(argv if argv is not None else sys.argv[1:])
            forwarded[0] = 'run'
            with (root/'runner.log').open('ab') as output:
                process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), *forwarded],
                    stdout=output, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
            deadline = time.monotonic()+5
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise DeployError(f'Runner exited; inspect {root/"runner.log"}')
                if lock_held(root):
                    print(f'Runner started (PID {process.pid}); status and logs: {root}')
                    return 0
                time.sleep(0.05)
            raise DeployError(f'Runner has not acquired its lock; inspect {root/"runner.log"}')
        runner = Runner(Config(repo=args.repo, deploy_dir=root, branch=args.branch, remote=args.remote,
            bind=args.bind, cargo=args.cargo, poll_interval=args.poll_interval, fetch_interval=args.fetch_interval,
            health_timeout=args.health_timeout, build_timeout=args.build_timeout))
        for signum in (signal.SIGINT, signal.SIGTERM):
            signal.signal(signum, lambda *_: runner.stopping.set())
        runner.run()
        return 0
    except (DeployError, OSError, ValueError, subprocess.TimeoutExpired) as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
