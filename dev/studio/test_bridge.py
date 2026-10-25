"""Real persistence tests; all worlds, render bytes and releases here are fixtures."""
import concurrent.futures
from contextlib import closing
import copy
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import unittest
import zipfile
import hashlib

import verifier
from pathlib import Path

import bridge


class BridgeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'history.sqlite'
        self.journal = bridge.Journal(self.path)
        self.identity = dict(world='demo', seed=42, release='fixture-release',
                             rules='a' * 64, assets='b' * 64)

    def event(self, key='move-1', **changes):
        event = dict(key=key, source='authority', at=1792950000, tick=1,
                     kind='action.completed', payload={'unit': 'worker-1'},
                     allowed_recipients=['helios'], identity=self.identity,
                     action={'dx': 2}, state={'x': 2}, evidence=[])
        event.update(changes)
        return event

    def test_restart_duplicate_and_conflict(self):
        first = self.journal.append(self.event())
        reopened = bridge.Journal(self.path)
        self.assertEqual(first, reopened.append(self.event()))
        with self.assertRaises(ValueError):
            reopened.append(self.event(payload={'unit': 'different'}))
        self.assertEqual(len(reopened.records()), 1)

    def test_reconstruct_and_verify_rules_replay(self):
        start = self.journal.append(self.event('genesis', kind='snapshot',
                                             action=None, tick=0, state={'x': 0}))
        end = self.journal.append(self.event())
        self.assertEqual(self.journal.reconstruct(end), {'x': 2})
        reducer = lambda state, action, identity: {'x': state['x'] + action['dx']}
        self.assertEqual(self.journal.replay(reducer, self.identity), {'x': 2})
        with self.assertRaises(ValueError):
            self.journal.replay(lambda s, a, i: {'x': 99}, self.identity)
        wrong = dict(self.identity, rules='c' * 64)
        with self.assertRaises(ValueError):
            self.journal.replay(reducer, wrong)
        self.assertNotEqual(start, end)

    def test_evidence_is_immutable_and_tied_to_event_and_versions(self):
        evidence = self.journal.store_evidence(b'fixture-render',
            dict(media_type='image/png', view='front', client='fixture-client',
                 assets='b' * 64, at=1792950000, tick=1))
        event_id = self.journal.append(self.event(evidence=[evidence]))
        record = self.journal.records()[0]
        self.assertEqual(record['hash'], event_id)
        self.assertEqual(self.journal.read_evidence(evidence)['bytes'], b'fixture-render')
        self.assertEqual(record['event']['evidence'], [evidence])
        with self.assertRaises(ValueError):
            self.journal.append(self.event('bad-render', evidence=['0' * 64]))
        with closing(sqlite3.connect(self.path)) as db, db:
            db.execute('UPDATE blobs SET data=?', (b'changed',))
        with self.assertRaises(ValueError):
            self.journal.verify()

    def test_tampering_missing_parent_and_tail_deletion_are_detected(self):
        self.journal.append(self.event('genesis', kind='snapshot', action=None))
        self.journal.append(self.event())
        with closing(sqlite3.connect(self.path)) as db, db:
            db.execute('DELETE FROM events WHERE seq=2')
        with self.assertRaises(ValueError):
            self.journal.verify()

    def test_payload_tamper_detected(self):
        self.journal.append(self.event())
        with closing(sqlite3.connect(self.path)) as db, db:
            db.execute("UPDATE events SET record='{}'")
        with self.assertRaises(ValueError):
            self.journal.records()

    def test_concurrent_duplicate_has_single_commit(self):
        def send(_):
            return bridge.Journal(self.path).append(self.event())
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            results = list(pool.map(send, range(12)))
        self.assertEqual(len(set(results)), 1)
        self.assertEqual(len(self.journal.records()), 1)

    def test_faction_export_has_no_hidden_history_state_or_evidence(self):
        self.journal.append(self.event('secret', payload={'secret': 'hidden'},
                                      allowed_recipients=['cinder']))
        visible = self.journal.append(self.event(state={'secret_inventory': 900}))
        self.journal.append(self.event('operator', allowed_recipients=[]))
        exported = self.journal.export_gm('helios')
        self.assertEqual(len(exported), 1)
        self.assertEqual(exported[0], dict(id=visible, source='authority', at=1792950000,
            kind='action.completed', payload={'unit': 'worker-1'},
            allowed_recipients=['helios']))
        self.assertNotIn('hidden', json.dumps(exported))
        self.assertNotIn('secret_inventory', json.dumps(exported))
        self.assertEqual(self.journal.export_gm('unknown'), [])

    def test_content_notice_is_restart_safe_and_stale_ack_rejected(self):
        self.journal.append(self.event('proposal', kind='content.proposed'))
        self.assertEqual(self.journal.pending('helios'), [])
        first = self.journal.append(self.event('release', kind='content.activated',
            payload={'content_hash': 'c' * 64, 'activation_id': 'release-1'}))
        self.assertEqual([x['id'] for x in self.journal.pending('helios')], [first])
        self.assertEqual(self.journal.pending('cinder'), [])
        self.journal.append(self.event('release-2', kind='content.activated',
            payload={'content_hash': 'd' * 64, 'activation_id': 'release-2'}))
        with self.assertRaises(ValueError):
            self.journal.acknowledge('helios', [first], 'decision-1', 'e' * 64)
        ids = [x['id'] for x in self.journal.pending('helios')]
        self.journal.acknowledge('helios', ids, 'decision-2', 'e' * 64)
        self.assertEqual(bridge.Journal(self.path).pending('helios'), [])
        self.journal.acknowledge('helios', ids, 'decision-2', 'e' * 64)
        with self.assertRaises(ValueError):
            self.journal.acknowledge('helios', ids, 'decision-2', 'f' * 64)

    def test_invalid_input_leaves_history_unchanged(self):
        for changes in ({'at': -1}, {'tick': True}, {'state': {'x': float('nan')}},
                        {'allowed_recipients': ['*']}, {'at': '2026-10-25'},
                        {'kind': 'content.activated', 'payload': {}}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.journal.append(self.event(**changes))
        self.assertEqual(self.journal.records(), [])

    def test_cli_produces_consumable_gm_json(self):
        self.journal.append(self.event())
        result = subprocess.run([sys.executable, str(Path(bridge.__file__)),
            str(self.path), 'export-gm', '--recipient', 'helios'],
            check=True, capture_output=True, text=True)
        self.assertEqual(json.loads(result.stdout)[0]['payload'], {'unit': 'worker-1'})

    def test_authority_file_can_be_ingested_without_python_embedding(self):
        incoming = Path(self.tmp.name) / 'accepted-action.json'
        incoming.write_text(json.dumps(self.event()))
        command = [sys.executable, str(Path(bridge.__file__)), str(self.path),
                   'append', '--input', str(incoming)]
        first = subprocess.run(command, check=True, capture_output=True, text=True)
        retry = subprocess.run(command, check=True, capture_output=True, text=True)
        self.assertEqual(json.loads(first.stdout), json.loads(retry.stdout))
        self.assertEqual(self.journal.verify(), 1)

    def test_studio_archive_contains_exact_evidence_and_readable_diffs(self):
        self.journal.append(self.event('genesis', kind='snapshot', action=None,
                                      tick=0, state={'x': 0, 'obsolete': True}))
        image = self.journal.store_evidence(b'fixture-render',
            dict(media_type='image/png', view='front', client='fixture-client',
                 assets='b' * 64, at=1792950000, tick=1))
        self.journal.append(self.event(evidence=[image]))
        target = Path(self.tmp.name) / 'studio.zip'
        digest = self.journal.export_studio(target)
        self.assertEqual(len(digest), 64)
        with zipfile.ZipFile(target) as archive:
            manifest = json.loads(archive.read('manifest.json'))
            self.assertEqual(manifest['records'][1]['diff'],
                             {'set': {'x': 2}, 'remove': ['obsolete']})
            self.assertEqual(archive.read('evidence/' + image), b'fixture-render')
        with self.assertRaises(FileExistsError):
            self.journal.export_studio(target)

    def test_same_activation_cannot_conflict_or_notify_twice(self):
        event = self.event('release', kind='content.activated',
            payload={'content_hash': 'c' * 64, 'activation_id': 'release-1'})
        self.journal.append(event)
        self.journal.append(dict(event, key='retry-with-new-transport-id'))
        self.assertEqual(len(self.journal.pending('helios')), 1)
        changed = copy.deepcopy(event)
        changed['key'] = 'conflict'
        changed['payload']['content_hash'] = 'd' * 64
        with self.assertRaises(ValueError):
            self.journal.append(changed)

    def test_failed_append_does_not_acknowledge_or_change_head(self):
        self.journal.append(self.event('start', kind='snapshot', action=None))
        with closing(sqlite3.connect(self.path)) as db, db:
            db.execute("CREATE TRIGGER fail BEFORE INSERT ON events BEGIN "
                       "SELECT RAISE(ABORT, 'disk fixture unavailable'); END")
        with self.assertRaises(sqlite3.IntegrityError):
            self.journal.append(self.event())
        self.assertEqual(self.journal.verify(), 1)

    def test_process_crash_rolls_back_unacknowledged_transaction(self):
        self.journal.append(self.event())
        subprocess.run([sys.executable, '-c',
            "import sqlite3,os,sys; c=sqlite3.connect(sys.argv[1]); "
            "c.execute('BEGIN IMMEDIATE'); "
            "c.execute(\"UPDATE meta SET value='bad' WHERE key='head'\"); os._exit(7)",
            str(self.path)], check=False)
        self.assertEqual(bridge.Journal(self.path).verify(), 1)

    def test_activation_retry_cannot_hide_different_state_time_or_evidence(self):
        event = self.event('release', kind='content.activated',
            payload={'content_hash': 'c' * 64, 'activation_id': 'release-1'})
        self.journal.append(event)
        for field, value in [('tick', 2), ('at', 1792950001), ('state', {'x': 4}),
                             ('action', {'dx': 4}), ('evidence', ['0' * 64])]:
            changed = dict(event, key='conflict', **{field: value})
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.journal.append(changed)

    def test_boolean_and_integer_payloads_are_distinct_retries(self):
        self.journal.append(self.event(state={'x': 1}))
        with self.assertRaises(ValueError):
            self.journal.append(self.event(state={'x': True}))

    def test_existing_world_readable_database_is_refused(self):
        os.chmod(self.path, 0o644)
        with self.assertRaises(ValueError):
            bridge.Journal(self.path)

    def archive(self):
        self.journal.append(self.event('genesis', kind='snapshot', action=None,
                                      tick=0, state={'x': 0}))
        image = self.journal.store_evidence(b'fixture-render',
            dict(media_type='image/png', view='front', client='fixture-client',
                 assets='b' * 64, at=1792950000, tick=1))
        self.journal.append(self.event(evidence=[image]))
        target = Path(self.tmp.name) / 'delivery.zip'
        digest = self.journal.export_studio(target)
        return target, digest, image

    def test_receiver_verifies_complete_archive_without_creating_database(self):
        target, digest, image = self.archive()
        result = verifier.verify_archive(target, digest)
        self.assertEqual(result['events'], 2)
        self.assertEqual(result['images'], 1)
        self.assertEqual(result['head'], self.journal.records()[-1]['hash'])
        with self.assertRaises(ValueError):
            verifier.verify_archive(target, '0' * 64)

    def test_receiver_rejects_changed_image_manifest_diff_or_missing_member(self):
        target, digest, image = self.archive()
        with zipfile.ZipFile(target) as source:
            original = {n: source.read(n) for n in source.namelist()}
        for defect in ('image', 'diff', 'record', 'missing', 'extra'):
            entries = dict(original)
            if defect == 'image':
                entries['evidence/' + image] = b'changed'
            elif defect == 'missing':
                del entries['evidence/' + image]
            elif defect == 'extra':
                entries['../unexpected'] = b'not extracted'
            else:
                manifest = json.loads(entries['manifest.json'])
                if defect == 'diff':
                    manifest['records'][1]['diff']['set']['x'] = 999
                else:
                    manifest['records'][1]['event']['at'] += 1
                entries['manifest.json'] = json.dumps(manifest).encode()
            broken = Path(self.tmp.name) / (defect + '.zip')
            with zipfile.ZipFile(broken, 'w') as archive:
                for filename, data in entries.items():
                    archive.writestr(filename, data)
            with self.subTest(defect=defect), self.assertRaises(ValueError):
                verifier.verify_archive(broken, hashlib.sha256(broken.read_bytes()).hexdigest())

    def test_receiver_rejects_invalid_provenance_even_when_hashes_are_consistent(self):
        target, digest, image = self.archive()
        with zipfile.ZipFile(target) as source:
            original = json.loads(source.read('manifest.json'))
            data = source.read('evidence/' + image)
        for field, value in [('media_type', 'application/javascript'), ('view', None),
                             ('client', []), ('tick', True), ('at', True)]:
            manifest = copy.deepcopy(original)
            meta = manifest['evidence'].pop(image)
            meta[field] = value
            replacement = bridge.fingerprint(dict(
                bytes_hash=hashlib.sha256(data).hexdigest(), metadata=meta))
            manifest['evidence'][replacement] = meta
            last = manifest['records'][-1]
            last['event']['evidence'] = [replacement]
            last['hash'] = bridge.fingerprint({k: v for k, v in last.items()
                                               if k not in ('hash', 'diff')})
            broken = Path(self.tmp.name) / (field + '.zip')
            with zipfile.ZipFile(broken, 'w') as archive:
                archive.writestr('manifest.json', bridge.canonical(manifest))
                archive.writestr('evidence/' + replacement, data)
            with self.subTest(field=field), self.assertRaises(ValueError):
                verifier.verify_archive(broken, hashlib.sha256(broken.read_bytes()).hexdigest())

    def test_atomic_gm_inbox_unions_only_named_factions_and_survives_retry(self):
        self.journal.append(self.event('secret', allowed_recipients=['unknown'],
                                      payload={'secret': 'hidden'}))
        first = self.journal.append(self.event())
        self.journal.append(self.event('operator', allowed_recipients=[]))
        second = self.journal.append(self.event('shared', allowed_recipients=['helios', 'tide']))
        inbox = Path(self.tmp.name) / 'world.json.gm-inbox.json'
        self.journal.publish_gm(inbox, ['helios', 'tide'])
        events = json.loads(inbox.read_bytes())
        self.assertEqual([e['id'] for e in events], [first, second])
        self.assertEqual(events[1]['allowed_recipients'], ['helios', 'tide'])
        self.assertNotIn('hidden', inbox.read_text())
        self.assertEqual(inbox.stat().st_mode & 0o777, 0o600)
        self.journal.publish_gm(inbox, ['helios', 'tide'])
        self.assertEqual(json.loads(inbox.read_bytes()), events)
        with self.assertRaises(ValueError):
            self.journal.publish_gm(self.path, ['helios'])
        self.assertEqual(self.journal.verify(), 4)

    def test_cli_publishes_live_gm_intake_contract(self):
        self.journal.append(self.event())
        inbox = Path(self.tmp.name) / 'world.json.gm-inbox.json'
        result = subprocess.run([sys.executable, str(Path(bridge.__file__)), str(self.path),
            'export-gm', '--recipient', 'helios', '--recipient', 'tide',
            '--output', str(inbox)], check=True, capture_output=True, text=True)
        self.assertEqual(json.loads(result.stdout)['events'], 1)
        self.assertEqual(json.loads(inbox.read_bytes())[0]['kind'], 'action.completed')

    def test_gm_pages_obey_settled_importer_limit_and_resume_without_loss(self):
        envelopes = [dict(id=format(n, '064x'), source='authority', at=1,
            kind='action.completed', payload={}, allowed_recipients=['helios'])
            for n in range(1025)]
        page, more = bridge.gm_page(envelopes)
        self.assertEqual(len(page), 1024)
        self.assertTrue(more)
        tail, more = bridge.gm_page(envelopes, page[-1]['id'])
        self.assertEqual([e['id'] for e in tail], [format(1024, '064x')])
        self.assertFalse(more)
        with self.assertRaises(ValueError):
            bridge.gm_page(envelopes, 'f' * 64)

    def test_gm_pages_obey_byte_cap_and_reject_incompatible_source_data(self):
        event = dict(id='a' * 64, source='authority', at=1,
            kind='action.completed', payload={'text': 'x' * 16000},
            allowed_recipients=['helios'])
        envelopes = [dict(event, id=format(n, '064x')) for n in range(100)]
        page, more = bridge.gm_page(envelopes)
        self.assertLessEqual(len(bridge.canonical(page)), 1048576)
        self.assertTrue(more)
        for changed in (dict(event, source='x' * 161), dict(event, source='秘密'),
                        dict(event, payload={'text': 'x' * 16384}),
                        dict(event, allowed_recipients=['f-' + str(n) for n in range(65)])):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                bridge.gm_page([changed])


if __name__ == '__main__':
    unittest.main()
