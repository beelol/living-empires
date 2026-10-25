"""Trusted local operator bridge. Never expose this database to faction clients.

Writers must be the simulation/release authority; this module records their
outcomes, it does not approve content or execute world actions. Python 3.9+.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import tempfile
import zipfile


def canonical(value):
    try:
        return json.dumps(value, sort_keys=True, separators=(',', ':'),
                          ensure_ascii=True, allow_nan=False).encode('ascii')
    except (TypeError, ValueError) as error:
        raise ValueError('expected finite JSON data') from error


def fingerprint(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def is_hash(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def name(value):
    return isinstance(value, str) and 0 < len(value) <= 256 and '*' not in value


class Journal:
    def __init__(self, path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        # Creates with owner-only permissions; SQLite sidecars inherit DB mode.
        fd = os.open(self.path, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            require(os.fstat(fd).st_mode & 0o077 == 0,
                    'history database must have owner-only permissions (0600)')
        finally:
            os.close(fd)
        with self._transaction() as db:
            db.execute('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)')
            db.execute('CREATE TABLE IF NOT EXISTS events '
                       '(seq INTEGER PRIMARY KEY, source TEXT, key TEXT, hash TEXT UNIQUE, '
                       'record TEXT, UNIQUE(source,key))')
            db.execute('CREATE TABLE IF NOT EXISTS blobs '
                       '(hash TEXT PRIMARY KEY, data BLOB, metadata TEXT)')
            db.execute('CREATE TABLE IF NOT EXISTS acks '
                       '(recipient TEXT, decision TEXT, body TEXT, '
                       'PRIMARY KEY(recipient,decision))')
            db.execute("INSERT OR IGNORE INTO meta VALUES ('version','1')")
            db.execute("INSERT OR IGNORE INTO meta VALUES ('head','')")
            require(dict(db.execute('SELECT key,value FROM meta'))['version'] == '1',
                    'unsupported history version')

    @contextlib.contextmanager
    def _transaction(self):
        db = sqlite3.connect(self.path, timeout=10)
        try:
            db.execute('PRAGMA synchronous=FULL')
            db.execute('BEGIN IMMEDIATE')
            yield db
            db.commit()
        except BaseException:
            db.rollback()
            raise
        finally:
            db.close()

    def _records(self, db):
        previous = ''
        result = []
        try:
            for seq, source, key, digest, body in db.execute(
                    'SELECT seq,source,key,hash,record FROM events ORDER BY seq'):
                record = json.loads(body)
                require(seq == len(result) + 1 and record['seq'] == seq,
                        'history sequence gap')
                require(record['parent'] == previous and fingerprint(record) == digest,
                        'history fingerprint/parent mismatch')
                event = record['event']
                self._validate(event)
                require(event['source'] == source and event['key'] == key,
                        'history index mismatch')
                require(record['state_hash'] == fingerprint(event['state']),
                        'history state mismatch')
                self._check_evidence(db, event)
                if result:
                    self._check_successor(result[-1]['event'], event)
                result.append(dict(record, hash=digest))
                previous = digest
            head = dict(db.execute('SELECT key,value FROM meta'))['head']
            require(previous == head, 'history tail missing')
        except (KeyError, TypeError, json.JSONDecodeError) as error:
            raise ValueError('invalid history record') from error
        return result

    def records(self):
        with self._transaction() as db:
            return self._records(db)

    def verify(self):
        return len(self.records())

    def _validate(self, event):
        canonical(event)
        require(isinstance(event, dict) and set(event) == {
            'key', 'source', 'at', 'tick', 'kind', 'payload', 'allowed_recipients',
            'identity', 'action', 'state', 'evidence'}, 'invalid v1 event fields')
        for field in ('key', 'source', 'kind'):
            require(name(event[field]), 'invalid ' + field)
        for field in ('at', 'tick'):
            require(type(event[field]) is int and 0 <= event[field] < 2**63,
                    field + ' must be a nonnegative integer')
        acl = event['allowed_recipients']
        require(isinstance(acl, list) and all(name(x) for x in acl) and
                len(set(acl)) == len(acl), 'invalid recipients; no wildcard')
        identity = event['identity']
        require(isinstance(identity, dict) and set(identity) == {
            'world', 'seed', 'release', 'rules', 'assets'}, 'invalid replay identity')
        require(name(identity['world']) and name(identity['release']) and
                type(identity['seed']) is int and 0 <= identity['seed'] < 2**64 and
                is_hash(identity['rules']) and is_hash(identity['assets']),
                'invalid replay fingerprints/seed')
        require(isinstance(event['payload'], dict) and isinstance(event['state'], dict),
                'payload and state must be objects')
        require(isinstance(event['evidence'], list) and
                all(is_hash(x) for x in event['evidence']), 'invalid evidence references')
        if event['kind'] == 'content.activated':
            require(is_hash(event['payload'].get('content_hash')) and
                    name(event['payload'].get('activation_id')),
                    'activation requires authority activation ID and exact fingerprint')

    def _check_successor(self, old, new):
        require(old['identity']['world'] == new['identity']['world'] and
                old['identity']['seed'] == new['identity']['seed'],
                'world/seed changed; start another journal')
        require(new['tick'] >= old['tick'], 'simulation time moved backward')

    def _blob(self, db, digest):
        row = db.execute('SELECT data,metadata FROM blobs WHERE hash=?', (digest,)).fetchone()
        require(row is not None, 'missing evidence blob')
        data, metadata = row
        metadata = json.loads(metadata)
        require(fingerprint({'bytes_hash': hashlib.sha256(data).hexdigest(),
                             'metadata': metadata}) == digest, 'evidence fingerprint mismatch')
        return dict(bytes=data, metadata=metadata)

    def _check_evidence(self, db, event):
        for digest in event['evidence']:
            meta = self._blob(db, digest)['metadata']
            require(meta['assets'] == event['identity']['assets'] and
                    meta['tick'] == event['tick'] and meta['at'] == event['at'],
                    'render does not match event time/content')

    def store_evidence(self, data, metadata):
        require(isinstance(data, bytes) and 0 < len(data) <= 20 * 1024 * 1024,
                'evidence must be 1..20MiB')
        require(isinstance(metadata, dict) and set(metadata) == {
            'media_type', 'view', 'client', 'assets', 'at', 'tick'},
            'missing render provenance')
        require(metadata['media_type'] in ('image/png', 'image/jpeg', 'image/webp') and
                name(metadata['view']) and name(metadata['client']) and
                is_hash(metadata['assets']) and all(type(metadata[k]) is int and
                metadata[k] >= 0 for k in ('at', 'tick')), 'invalid render provenance')
        digest = fingerprint({'bytes_hash': hashlib.sha256(data).hexdigest(),
                              'metadata': metadata})
        with self._transaction() as db:
            db.execute('INSERT OR IGNORE INTO blobs VALUES (?,?,?)',
                       (digest, data, canonical(metadata).decode()))
            self._blob(db, digest)
        return digest

    def read_evidence(self, digest):
        with self._transaction() as db:
            return self._blob(db, digest)

    def append(self, event):
        self._validate(event)
        with self._transaction() as db:
            records = self._records(db)
            for record in records:
                old = record['event']
                if (old['source'], old['key']) == (event['source'], event['key']):
                    require(canonical(old) == canonical(event),
                            'source event ID reused with different data')
                    return record['hash']
                if old['kind'] == event['kind'] == 'content.activated' and \
                        old['payload']['activation_id'] == event['payload']['activation_id']:
                    require(canonical({k: v for k, v in old.items() if k != 'key'}) ==
                            canonical({k: v for k, v in event.items() if k != 'key'}),
                            'activation ID reused with different authority event')
                    return record['hash']
            if records:
                self._check_successor(records[-1]['event'], event)
            self._check_evidence(db, event)
            record = dict(version=1, seq=len(records) + 1,
                          parent=records[-1]['hash'] if records else '',
                          event=event, state_hash=fingerprint(event['state']))
            digest = fingerprint(record)
            db.execute('INSERT INTO events VALUES (?,?,?,?,?)',
                       (record['seq'], event['source'], event['key'], digest,
                        canonical(record).decode()))
            db.execute("UPDATE meta SET value=? WHERE key='head'", (digest,))
        return digest

    def reconstruct(self, event_hash=None):
        records = self.records()
        require(bool(records), 'no starting snapshot')
        for record in reversed(records):
            if event_hash is None or event_hash == record['hash']:
                return record['event']['state']
        raise ValueError('unknown history hash')

    def export_studio(self, destination):
        """Operator-only archive, including private history and exact render bytes."""
        destination = Path(destination)
        with self._transaction() as db:
            records = self._records(db)
            previous = {}
            views = []
            blobs = {}
            for record in records:
                state = record['event']['state']
                diff = dict(set={k: v for k, v in state.items()
                                 if k not in previous or canonical(previous[k]) != canonical(v)},
                            remove=sorted(set(previous) - set(state)))
                views.append(dict(record, diff=diff))
                previous = state
                for digest in record['event']['evidence']:
                    blobs[digest] = self._blob(db, digest)
            manifest = dict(version=1, audience='trusted-operator-only', records=views,
                evidence={h: b['metadata'] for h, b in blobs.items()})
        # Publish a complete archive without replacing an existing delivery.
        fd, temporary = tempfile.mkstemp(prefix='.studio-', dir=destination.parent)
        try:
            with os.fdopen(fd, 'w+b') as output:
                with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_STORED) as archive:
                    def put(filename, data):
                        info = zipfile.ZipInfo(filename, date_time=(1980, 1, 1, 0, 0, 0))
                        info.external_attr = 0o600 << 16
                        archive.writestr(info, data)
                    put('manifest.json', canonical(manifest))
                    for digest in sorted(blobs):
                        put('evidence/' + digest, blobs[digest]['bytes'])
                output.flush()
                os.fsync(output.fileno())
                output.seek(0)
                digest = hashlib.file_digest(output, 'sha256').hexdigest() if hasattr(
                    hashlib, 'file_digest') else hashlib.sha256(output.read()).hexdigest()
            os.link(temporary, destination)
            directory = os.open(destination.parent, os.O_RDONLY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
            return digest
        finally:
            os.unlink(temporary)

    def replay(self, reducer, identity):
        records = self.records()
        require(bool(records) and records[0]['event']['kind'] == 'snapshot',
                'rules replay needs explicit starting snapshot')
        state = records[0]['event']['state']
        for index, record in enumerate(records):
            event = record['event']
            require(event['identity'] == identity, 'pinned rules/content/release unavailable')
            if index:
                state = reducer(state, event['action'], identity)
                require(fingerprint(state) == record['state_hash'], 'rules replay diverged')
        return state

    @staticmethod
    def _envelope(record):
        event = record['event']
        return dict(id=record['hash'], **{key: event[key] for key in
            ('source', 'at', 'kind', 'payload', 'allowed_recipients')})

    def export_gm(self, recipient):
        require(name(recipient), 'explicit recipient required')
        return [self._envelope(r) for r in self.records()
                if recipient in r['event']['allowed_recipients']]

    def _pending(self, db, recipient, records):
        seen = set()
        for body, in db.execute('SELECT body FROM acks WHERE recipient=?', (recipient,)):
            seen.update(json.loads(body)['ids'])
        return [self._envelope(r) for r in records if
                r['event']['kind'] == 'content.activated' and
                recipient in r['event']['allowed_recipients'] and r['hash'] not in seen]

    def pending(self, recipient):
        require(name(recipient), 'explicit recipient required')
        with self._transaction() as db:
            return self._pending(db, recipient, self._records(db))

    def acknowledge(self, recipient, ids, decision, state_hash):
        require(name(recipient) and name(decision) and is_hash(state_hash),
                'decision and observed current state fingerprint required')
        body = canonical(dict(ids=ids, state_hash=state_hash)).decode()
        with self._transaction() as db:
            records = self._records(db)
            old = db.execute('SELECT body FROM acks WHERE recipient=? AND decision=?',
                             (recipient, decision)).fetchone()
            if old:
                require(old[0] == body, 'decision ID reused with different acknowledgment')
                return
            expected = [e['id'] for e in self._pending(db, recipient, records)]
            require(bool(expected) and ids == expected, 'stale/incomplete content notice set')
            db.execute('INSERT INTO acks VALUES (?,?,?)', (recipient, decision, body))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('database', type=Path)
    parser.add_argument('command', choices=['verify', 'export-gm', 'operator',
                                          'reconstruct', 'export-studio', 'append'])
    parser.add_argument('--recipient')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--input', type=Path)
    args = parser.parse_args()
    journal = Journal(args.database)
    if args.command == 'verify':
        result = dict(version=1, verified_events=journal.verify())
    elif args.command == 'export-gm':
        result = journal.export_gm(args.recipient)
    elif args.command == 'operator':
        result = dict(version=1, records=journal.records())
    elif args.command == 'export-studio':
        require(args.output is not None, '--output archive.zip required')
        result = dict(sha256=journal.export_studio(args.output))
    elif args.command == 'append':
        require(args.input is not None, '--input authority-event.json required')
        result = dict(id=journal.append(json.loads(args.input.read_text())))
    else:
        result = journal.reconstruct()
    print(canonical(result).decode())


if __name__ == '__main__':
    main()
