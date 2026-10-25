#!/usr/bin/env python3
"""Project-local delivery watcher; run from the integration checkout.

Ownership: minute-push-watch, scripts/push-watch.py only. AUD-03 operational
evidence, no game AC pass claimed. Depends on dev/agent.py integration.lock
and existing origin/main history. Acceptance: --once and two real minute ticks.
Run: python3 scripts/push-watch.py (four-hour session, no installed service).
Stop: python3 scripts/push-watch.py --stop
Status/log: .data/git-push-watch/{status.json,events.jsonl}
Never stages, commits, merges, or rewrites history; builders own checkpoints.
"""
import argparse
import datetime
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / '.data/git-push-watch'


def record(state, **fields):
    value = dict(time=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 pid=os.getpid(), state=state, **fields)
    STATE.mkdir(parents=True, exist_ok=True)
    temporary = STATE / ('status-%s.tmp' % os.getpid())
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(STATE / 'status.json')
    with (STATE / 'events.jsonl').open('a') as log:
        log.write(json.dumps(value) + '\n')
    print(json.dumps(value), flush=True)


def git(*args):
    env = dict(os.environ, GIT_TERMINAL_PROMPT='0')
    result = subprocess.run(['git', '-C', str(ROOT), *args], env=env,
                            text=True, capture_output=True, timeout=40)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-2000:] or 'Git command failed')
    return result.stdout.strip()


def tick():
    # The caller holds dev/agent.py's lock for this entire operation.
    branch = git('symbolic-ref', '--quiet', '--short', 'HEAD')
    if branch != 'main':
        record('skipped', reason='Integration checkout is not on main')
        return
    candidate = git('rev-parse', 'HEAD')
    # Never follow a changed destination silently or expose URL credentials.
    remote = git('remote', 'get-url', 'origin')
    if remote not in ('https://github.com/beelol/living-empires.git',
                      'https://github.com/beelol/living-empires',
                      'git@github.com:beelol/living-empires.git'):
        raise RuntimeError('Origin destination differs from authorized beelol/living-empires')
    git('fetch', '--quiet', 'origin', 'main')
    upstream = git('rev-parse', 'refs/remotes/origin/main')
    if upstream == candidate:
        record('synced', commit=candidate, pushed=False)
        return
    relation = subprocess.run(['git', '-C', str(ROOT), 'merge-base',
                               '--is-ancestor', upstream, candidate])
    if relation.returncode:
        record('blocked', commit=candidate, remote=upstream,
               reason='Remote advanced or diverged; integration owner must reconcile. No merge attempted.')
        return
    if git('rev-parse', 'HEAD') != candidate:
        record('skipped', reason='HEAD changed during tick; another writer ignored integration lock')
        return
    git('push', 'origin', candidate + ':refs/heads/main')
    verified = git('ls-remote', 'origin', 'refs/heads/main').split()[0]
    if verified != candidate:
        raise RuntimeError('Push returned success but remote advanced before exact verification; reconcile on next tick')
    git('config', 'codex.lastPushedCommit', candidate)
    record('synced', commit=candidate, pushed=True)


def once():
    result = subprocess.run([sys.executable, str(ROOT / 'dev/agent.py'), 'lock',
                             '--', sys.executable, str(Path(__file__).resolve()),
                             '--tick'], text=True, capture_output=True)
    if result.returncode:
        detail = result.stderr.strip()
        busy = 'lock' in detail.lower() and any(word in detail.lower() for word in ('held', 'holds', 'busy', 'temporarily unavailable'))
        record('skipped' if busy else 'blocked', reason=detail[-2000:] or 'Integration command failed')
    elif result.stdout:
        print(result.stdout.strip(), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stop', action='store_true')
    parser.add_argument('--once', action='store_true')
    parser.add_argument('--tick', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True)
    if args.stop:
        (STATE / 'stop').touch()
        print('Stop requested; watcher exits after its current bounded Git operation.')
        return
    if args.tick:
        try:
            tick()
        except Exception as error:
            record('blocked', reason=str(error))
        return
    with (STATE / 'watcher.lock').open('a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            sys.exit('A watcher already owns this checkout; inspect status.json.')
        if args.once:
            once()
            return
        (STATE / 'stop').unlink(missing_ok=True)
        record('started', intervalSeconds=60, maximumSessionHours=4)
        end = time.monotonic() + 4 * 3600
        due = time.monotonic()
        try:
            while time.monotonic() < end and not (STATE / 'stop').exists():
                if time.monotonic() >= due:
                    once()
                    due += 60
                    if due < time.monotonic():
                        due = time.monotonic() + 60
                time.sleep(1)
        except KeyboardInterrupt:
            pass
        finally:
            record('stopped', reason='Stop requested, session interrupted, or four-hour session ended')


if __name__ == '__main__':
    main()
