"""Durable automatic transport from permitted GM needs to the actual Asset Foundry.

This worker owns content jobs, not faction decisions, world activation or payment.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

try:
    from .bridge import Journal, canonical, fingerprint, require, is_hash
except ImportError:
    from bridge import Journal, canonical, fingerprint, require, is_hash


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Busy(Exception):
    pass


def same_json(left, right):
    """JSON numbers survive Node's 12.0 -> 12 encoding; booleans remain distinct."""
    if type(left) in (int, float) and type(right) in (int, float):
        return left == right
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return set(left) == set(right) and all(same_json(v, right[k]) for k, v in left.items())
    if isinstance(left, list):
        return len(left) == len(right) and all(same_json(a, b) for a, b in zip(left, right))
    return left == right


class FoundryAPI:
    def __init__(self, base):
        parsed = urllib.parse.urlsplit(base)
        require(parsed.scheme == 'http' and parsed.hostname in ('127.0.0.1', 'localhost')
                and not parsed.username and not parsed.password and not parsed.query
                and not parsed.fragment and parsed.path in ('', '/'),
                'Foundry must be an explicitly configured loopback HTTP service')
        self.base = base.rstrip('/')
        self.opener = urllib.request.build_opener(NoRedirect(), urllib.request.ProxyHandler({}))

    def request(self, path, body=None, binary=False):
        require(path.startswith('/') and not path.startswith('//'), 'invalid Foundry path')
        request = urllib.request.Request(self.base + path,
            data=canonical(body) if body is not None else None,
            headers={'Content-Type': 'application/json'} if body is not None else {})
        try:
            with self.opener.open(request, timeout=15) as response:
                limit = 2_900_000 if binary else 12_000_000
                data = response.read(limit + 1)
                require(len(data) <= limit, 'Foundry response exceeds limit')
                if binary:
                    require(data.startswith(b'\x89PNG\r\n\x1a\n'), 'render evidence is not PNG')
                    return data
                return json.loads(data)
        except urllib.error.HTTPError as error:
            if error.code == 409 and path == '/api/jobs' and body is not None:
                raise Busy('Foundry already has an active job') from None
            raise ValueError('Foundry HTTP ' + str(error.code)) from None

    def list_jobs(self):
        return self.request('/api/jobs')

    def create(self, body):
        return self.request('/api/jobs', body)

    def job(self, job_id):
        require(re.fullmatch(r'[a-z0-9-]{1,128}', job_id) is not None, 'invalid job ID')
        return self.request('/api/jobs/' + job_id)


def requests_from_gm(ledger, policies):
    events = {e['id']: e for e in ledger.get('events', [])}
    for receipt in ledger.get('decisions', []):
        proposal = receipt.get('proposal', {})
        if not (receipt.get('accepted') is True and proposal.get('action') == 'request_content'
                and proposal.get('target') == 'studio'):
            continue
        ids = proposal.get('event_ids', [])
        require(isinstance(ids, list) and bool(ids) and len(set(ids)) == len(ids),
                'content request must cite distinct source facts')
        require(all(i in events for i in ids), 'content request cites unavailable facts')
        facts = [events[i] for i in ids]
        needs = [e for e in facts if e.get('source') == 'faction-brain' and e.get('kind') in
                 ('unit_need', 'building_need', 'blocked_need', 'blocked_building_need')]
        for event in needs:
            need = event['payload']
            if (need.get('proposal') or {}).get('action', {}).get('kind') != 'request_design':
                continue
            faction = need.get('faction_id')
            require(faction in policies and all(faction in e['allowed_recipients'] for e in facts),
                    'content brief would reveal a fact outside faction knowledge')
            require(need.get('category') in ('unit', 'building') and
                    isinstance(need.get('need_id'), str) and isinstance(need.get('role'), str),
                    'need lacks an authoritative category/role/identity')
            # Full permitted facts stay local. A creator needs the fictional asset
            # role and appearance contract, not inventories, diplomacy or orders.
            brief = dict(faction_id=faction, category=need['category'], role=need['role'],
                         need_id=need['need_id'], facts=[{k: e[k] for k in
                         ('id', 'source', 'at', 'kind', 'payload')} for e in facts])
            identity = {k: brief[k] for k in ('faction_id', 'category', 'role', 'need_id')}
            request_id = 'need-' + fingerprint(identity)
            require(re.fullmatch(r'[A-Za-z0-9_.:-]{1,96}', need['role']),
                    'asset role must be a bounded authority identifier')
            creator_brief = dict(category=need['category'], role=need['role'],
                instruction='Create this functional game asset in the supplied frozen visual theme. '
                            'Return only the supported recipe; do not invent ownership or inventory.')
            prompt = '[empire-request:' + request_id + ']\n' + canonical(creator_brief).decode()
            require(len(prompt) <= 3000, 'permitted brief exceeds Foundry prompt limit')
            profile = {k: v for k, v in policies[faction].items()
                       if k in ('themeId', 'theme', 'injectDefect')}
            yield dict(id=request_id, creator_brief_version=2, local_brief=brief,
                receipt_id=receipt['id'], receipt_hash=fingerprint(
                dict(receipt=receipt, facts=facts)), faction=faction, category=need['category'],
                role=need['role'], allowed_recipients=[faction],
                body=dict(profile, prompt=prompt))


class Controller:
    def __init__(self, path, api, policies, provider='fixture', live_authorized=False):
        require(provider in ('fixture', 'gemini', 'managed'), 'unknown Foundry provider')
        require(provider == 'fixture' or live_authorized is True,
                'automatic paid provider requires explicit operator authorization')
        self.store = Journal(path)
        self.api, self.policies, self.provider = api, policies, provider
        with self.store._transaction() as db:
            db.execute('CREATE TABLE IF NOT EXISTS foundry_receipts '
                       '(id TEXT PRIMARY KEY, hash TEXT)')
            db.execute('CREATE TABLE IF NOT EXISTS foundry_requests '
                       '(id TEXT PRIMARY KEY, body TEXT, state TEXT, job_id TEXT, reason TEXT)')
            db.execute('CREATE TABLE IF NOT EXISTS foundry_audit '
                       '(seq INTEGER PRIMARY KEY, at_ms INTEGER, request_id TEXT, state TEXT, detail TEXT)')
            db.execute('CREATE TABLE IF NOT EXISTS foundry_artifacts '
                       '(request_id TEXT, name TEXT, hash TEXT, bytes BLOB, '
                       'PRIMARY KEY(request_id,name))')

    def sync(self, ledger):
        requests = list(requests_from_gm(ledger, self.policies))
        with self.store._transaction() as db:
            for request in requests:
                old = db.execute('SELECT hash FROM foundry_receipts WHERE id=?',
                                 (request['receipt_id'],)).fetchone()
                require(old is None or old[0] == request['receipt_hash'],
                        'GM receipt changed after dispatch identity was frozen')
                db.execute('INSERT OR IGNORE INTO foundry_receipts VALUES (?,?)',
                           (request['receipt_id'], request['receipt_hash']))
                request['body']['provider'] = self.provider
                state, reason = ('queued', '') if request['category'] == 'building' else (
                    'blocked', 'Foundry has no unit actor recipe/behavior contract yet')
                db.execute('INSERT OR IGNORE INTO foundry_requests VALUES (?,?,?,?,?)',
                           (request['id'], canonical(request).decode(), state, None, reason))

    def status(self):
        with self.store._transaction() as db:
            return [dict(id=i, request=json.loads(body), state=state, job_id=job_id, reason=reason)
                    for i, body, state, job_id, reason in db.execute(
                        'SELECT id,body,state,job_id,reason FROM foundry_requests ORDER BY rowid')]

    def _set(self, request_id, state, reason='', job_id=None):
        with self.store._transaction() as db:
            db.execute('UPDATE foundry_requests SET state=?,reason=?,job_id=COALESCE(?,job_id) '
                       'WHERE id=?', (state, reason, job_id, request_id))
            db.execute('INSERT INTO foundry_audit(at_ms,request_id,state,detail) VALUES (?,?,?,?)',
                       (int(time.time() * 1000), request_id, state, reason))

    def artifact(self, request_id, filename, data):
        return self.cache_files(request_id, {filename: data})[filename]

    def cache_files(self, request_id, files):
        hashes = {}
        with self.store._transaction() as db:
            for filename, data in files.items():
                require(re.fullmatch(r'[A-Za-z0-9_.-]{1,140}', filename) and
                        isinstance(data, bytes), 'invalid artifact file')
                digest = hashlib.sha256(data).hexdigest()
                old = db.execute('SELECT hash FROM foundry_artifacts WHERE request_id=? AND name=?',
                                 (request_id, filename)).fetchone()
                require(old is None or old[0] == digest, 'Foundry changed immutable artifact bytes')
                db.execute('INSERT OR IGNORE INTO foundry_artifacts VALUES (?,?,?,?)',
                           (request_id, filename, digest, data))
                hashes[filename] = digest
        return hashes

    def artifacts(self, request_id):
        with self.store._transaction() as db:
            files = {}
            for filename, digest, data in db.execute(
                    'SELECT name,hash,bytes FROM foundry_artifacts WHERE request_id=?', (request_id,)):
                require(re.fullmatch(r'[A-Za-z0-9_.-]{1,140}', filename) and
                        hashlib.sha256(data).hexdigest() == digest, 'corrupt artifact cache')
                files[filename] = data
            return files

    def step(self):
        for record in self.status():
            request_id, state = record['id'], record['state']
            if state in ('blocked', 'ready', 'failed', 'cancelled', 'exhausted'):
                continue
            body = record['request']['body']
            if state in ('dispatching', 'unknown'):
                matches = [j for j in self.api.list_jobs() if j.get('prompt') == body['prompt']]
                if not matches:
                    self._set(request_id, 'unknown', 'Uncertain creation; no automatic paid retry')
                    continue
                require(len(matches) == 1 and matches[0].get('provider') == body['provider']
                        and matches[0].get('themeId') == body.get('themeId', body.get('theme', {}).get('id')),
                        'Foundry reconciliation is ambiguous or mismatched')
                self._set(request_id, 'running', job_id=matches[0]['id'])
                return
            if state == 'queued':
                if record['request'].get('creator_brief_version') != 2:
                    self._set(request_id, 'blocked', 'Old creator payload requires privacy migration')
                    continue
                # Compare-and-set before network dispatch prevents concurrent writers spending twice.
                with self.store._transaction() as db:
                    changed = db.execute("UPDATE foundry_requests SET state='dispatching' "
                                         "WHERE id=? AND state='queued'", (request_id,)).rowcount
                    if changed:
                        db.execute('INSERT INTO foundry_audit(at_ms,request_id,state,detail) '
                                   "VALUES (?,?,'dispatching','Creation persisted before RPC')",
                                   (int(time.time() * 1000), request_id))
                if not changed:
                    continue
                try:
                    job = self.api.create(body)
                    require(job.get('prompt') == body['prompt'] and job.get('id'),
                            'Foundry created a different request')
                    self._set(request_id, 'running', job_id=job['id'])
                except Busy as error:
                    self._set(request_id, 'queued', str(error))
                except Exception:
                    self._set(request_id, 'unknown', 'Creation outcome unknown; reconcile before retry')
                return
            if state == 'running':
                job = self.api.job(record['job_id'])
                require(job.get('prompt') == body['prompt'], 'Foundry job identity changed')
                if job['status'] in ('blocked', 'failed', 'cancelled', 'exhausted'):
                    self.artifact(request_id, 'terminal-job.json', canonical(job))
                    self._set(request_id, job['status'], job.get('stopReason', job['status']))
                elif job['status'] in ('accepted', 'active'):
                    self._set(request_id, 'accepted', 'Awaiting exact package/image verification')
                return


def verify_package(api, job, binary):
    """Re-run first-party Rust integrity validation; retain exact reviewed image bytes."""
    result = subprocess.run([str(binary)], input=canonical(dict(op='package',
        pipeline=job['pipeline'], now=int(time.time() * 1000))), capture_output=True, timeout=20)
    require(result.returncode == 0, 'Foundry Rust pipeline rejected package integrity')
    approved = json.loads(result.stdout)['value']
    package = api.request('/api/jobs/' + job['id'] + '/package')
    comparable = json.loads(canonical(package))
    comparable.get('manifest', {}).pop('adapter', None)
    require(same_json(approved, comparable), 'HTTP package differs from approved pipeline')
    # Reparse the HTTP object through Rust's actual typed schema and revalidate
    # its frozen hash, rather than treating formatting-normalized numbers as proof.
    received_pipeline = dict(job['pipeline'], approved=comparable)
    received = subprocess.run([str(binary)], input=canonical(dict(op='package',
        pipeline=received_pipeline, now=int(time.time() * 1000))), capture_output=True, timeout=20)
    require(received.returncode == 0, 'HTTP package fails first-party typed integrity validation')
    require(package['assetId'] == job['id'] and package['manifest']['contentSha256'] ==
            job['acceptedHash'], 'accepted asset identity/hash mismatch')
    evidence = package['manifest']['evidence']
    require(len(evidence) == 3 and len({e['sha256'] for e in evidence}) == 3,
            'three distinct exact render images required')
    images = {}
    for item in evidence:
        url = item['url']
        require(re.fullmatch('/evidence/' + re.escape(job['id']) + r'/\d+-[0-2]\.png', url),
                'evidence URL outside selected job')
        data = api.request(url, binary=True)
        require(hashlib.sha256(data).hexdigest() == item['sha256'], 'reviewed image bytes changed')
        images[item['sha256'] + '.png'] = data
    return package, images


def validate_provenance(record, package):
    require(package['manifest']['rehearsal'] is (record['request']['body']['provider'] == 'fixture'),
            'rehearsal provenance does not match selected provider')


def publish_ready(controller, record, output, binary):
    request_id = record['id']
    files = controller.artifacts(request_id)
    if 'ready.json' in files:
        notice = json.loads(files['ready.json'])
    else:
        job = controller.api.job(record['job_id'])
        package, images = verify_package(controller.api, job, binary)
        validate_provenance(record, package)
        files = dict(images, **{'package.json': canonical(package), 'job.json': canonical(job),
                               'request.json': canonical(record['request'])})
        hashes = {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}
        notice = dict(id='foundry-' + fingerprint(dict(request=request_id, artifacts=hashes)),
            source='asset-foundry', at=int(time.time()), kind='content.candidate_ready',
            allowed_recipients=record['request']['allowed_recipients'],
            payload=dict(request_id=request_id, job_id=job['id'], faction_id=record['request']['faction'],
                category=record['request']['category'], role=record['request']['role'],
                content_sha256=package['manifest']['contentSha256'],
                package_sha256=hashes['package.json'], rehearsal=package['manifest']['rehearsal'],
                game_activation='pending_authority_checks'))
        files['ready.json'] = canonical(notice)
        # Seal the complete immutable delivery in one transaction before publication.
        controller.cache_files(request_id, files)
    target = Path(output) / request_id
    if target.exists():
        for name, data in files.items():
            require((target / name).is_file() and (target / name).read_bytes() == data,
                    'delivery differs from immutable published files')
    else:
        Path(output).mkdir(parents=True, exist_ok=True)
        temporary = Path(tempfile.mkdtemp(prefix='.foundry-', dir=output))
        try:
            for name, data in files.items():
                fd = os.open(temporary / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                with os.fdopen(fd, 'wb') as stream:
                    stream.write(data)
                    stream.flush()
                    os.fsync(stream.fileno())
            directory = os.open(temporary, os.O_RDONLY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
            os.rename(temporary, target)
            directory = os.open(target.parent, os.O_RDONLY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
        finally:
            if temporary.exists():
                for child in temporary.iterdir():
                    child.unlink()
                temporary.rmdir()
    controller._set(request_id, 'ready', 'Verified Foundry candidate; awaiting game authority activation')
    return notice


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database', required=True, type=Path)
    parser.add_argument('--gm-ledger', required=True, type=Path)
    parser.add_argument('--policies', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--foundry', default='http://127.0.0.1:4317')
    parser.add_argument('--core', type=Path, default=Path('target/debug/world-check'))
    parser.add_argument('--provider', choices=['fixture', 'gemini', 'managed'], default='fixture')
    parser.add_argument('--live-authorized', action='store_true')
    parser.add_argument('--once', action='store_true')
    args = parser.parse_args()
    controller = Controller(args.database, FoundryAPI(args.foundry),
        json.loads(args.policies.read_text()), args.provider, args.live_authorized)
    while True:
        try:
            controller.sync(json.loads(args.gm_ledger.read_text()))
            controller.step()
            for record in controller.status():
                if record['state'] == 'accepted':
                    publish_ready(controller, record, args.output, args.core)
        except (OSError, ValueError, TimeoutError) as error:
            print(json.dumps(dict(state='blocked', reason=str(error))), flush=True)
        if args.once:
            print(canonical(controller.status()).decode())
            return
        time.sleep(5)


if __name__ == '__main__':
    main()
