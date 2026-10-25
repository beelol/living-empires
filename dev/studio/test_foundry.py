"""Transport/controller fixtures; these tests make no model calls."""
from contextlib import closing
import json
from pathlib import Path
import tempfile
import unittest

import foundry


def ledger(category='building', faction='helios'):
    return dict(events=[dict(id='need-event-1', source='faction-brain', at=100,
        kind='building_need' if category == 'building' else 'unit_need',
        allowed_recipients=[faction], payload=dict(schema_version=1,
            faction_id=faction, observed_revision=2, need_id='relay-need',
            role='sensor_relay', category=category, status='proposed',
            proposal={'action': {'kind': 'request_design'}}))],
        decisions=[dict(id='gm-request-1', accepted=True, observed_revision=3,
            proposal=dict(action='request_content', target='studio',
                event_ids=['need-event-1'], summary='DO NOT SEND PRIVATE OPERATOR TEXT'))],
        attempts=[])


class Backend:
    """External Studio service substitute; all durable controller logic is real."""
    def __init__(self):
        self.jobs = []
        self.fail_after_create = False

    def list_jobs(self):
        return self.jobs

    def create(self, body):
        job = dict(body, id='job-fixture-1', status='generating')
        self.jobs.append(job)
        if self.fail_after_create:
            raise TimeoutError('response lost after server accepted')
        return job

    def job(self, job_id):
        return next(j for j in self.jobs if j['id'] == job_id)


class FoundryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.api = Backend()
        self.policy = {'helios': {'themeId': 'argonaut'}}
        self.path = self.root / 'controller.sqlite'

    def controller(self):
        return foundry.Controller(self.path, self.api, self.policy, provider='fixture')

    def test_exact_gm_need_dispatches_once_and_excludes_operator_summary(self):
        controller = self.controller()
        controller.sync(ledger())
        controller.step()
        self.assertEqual(len(self.api.jobs), 1)
        prompt = self.api.jobs[0]['prompt']
        self.assertIn('sensor_relay', prompt)
        self.assertNotIn('helios', prompt)
        self.assertNotIn('relay-need', prompt)
        self.assertNotIn('PRIVATE OPERATOR', prompt)
        reopened = self.controller()
        reopened.sync(ledger())
        reopened.step()
        self.assertEqual(len(self.api.jobs), 1)
        self.assertEqual(reopened.status()[0]['job_id'], 'job-fixture-1')

    def test_unknown_post_outcome_reconciles_without_second_creation(self):
        self.api.fail_after_create = True
        controller = self.controller()
        controller.sync(ledger())
        controller.step()
        self.assertEqual(controller.status()[0]['state'], 'unknown')
        reopened = self.controller()
        reopened.step()
        self.assertEqual(len(self.api.jobs), 1)
        self.assertEqual(reopened.status()[0]['state'], 'running')

    def test_private_or_missing_fact_cannot_dispatch(self):
        private = ledger()
        private['events'][0]['allowed_recipients'] = ['cinder']
        with self.assertRaises(ValueError):
            self.controller().sync(private)
        missing = ledger()
        missing['events'] = []
        with self.assertRaises(ValueError):
            self.controller().sync(missing)
        self.assertEqual(self.api.jobs, [])

    def test_unit_need_is_preserved_as_blocked_until_real_producer_support(self):
        controller = self.controller()
        controller.sync(ledger(category='unit'))
        controller.step()
        self.assertEqual(controller.status()[0]['state'], 'blocked')
        self.assertIn('unit', controller.status()[0]['reason'])
        self.assertEqual(self.api.jobs, [])

    def test_changed_receipt_cannot_reuse_dispatch_identity(self):
        controller = self.controller()
        controller.sync(ledger())
        changed = ledger()
        changed['events'][0]['payload']['role'] = 'factory'
        with self.assertRaises(ValueError):
            controller.sync(changed)

    def test_live_provider_without_explicit_allowance_never_dispatches(self):
        with self.assertRaises(ValueError):
            foundry.Controller(self.path, self.api, self.policy, provider='managed')
        self.assertEqual(self.api.jobs, [])

    def test_persisted_provider_controls_package_provenance_after_restart(self):
        record = {'request': {'body': {'provider': 'managed'}}}
        foundry.validate_provenance(record, {'manifest': {'rehearsal': False}})
        with self.assertRaises(ValueError):
            foundry.validate_provenance(record, {'manifest': {'rehearsal': True}})

    def test_ready_delivery_recovers_from_published_directory_without_refetch(self):
        controller = self.controller()
        controller.sync(ledger())
        controller.step()
        record = controller.status()[0]
        notice = {'id': 'fixed-notice', 'at': 100, 'kind': 'content.candidate_ready'}
        controller.artifact(record['id'], 'package.json', b'{"fixed":true}')
        controller.artifact(record['id'], 'ready.json', foundry.canonical(notice))
        target = self.root / 'deliveries' / record['id']
        target.mkdir(parents=True)
        (target / 'package.json').write_bytes(b'{"fixed":true}')
        (target / 'ready.json').write_bytes(foundry.canonical(notice))
        # Remote job can mutate after initial verified delivery; recovery uses its frozen bytes.
        self.api.jobs.clear()
        recovered = foundry.publish_ready(controller, record, target.parent, Path('/unused'))
        self.assertEqual(recovered, notice)
        self.assertEqual(controller.status()[0]['state'], 'ready')
        self.assertEqual((target / 'package.json').read_bytes(), b'{"fixed":true}')

    def test_affordable_build_proposal_does_not_request_another_design(self):
        source = ledger()
        source['events'][0]['payload']['proposal']['action'] = {'kind': 'build_structure'}
        controller = self.controller()
        controller.sync(source)
        controller.step()
        self.assertEqual(controller.status(), [])
        self.assertEqual(self.api.jobs, [])

    def test_http_numeric_json_keeps_rust_meaning_without_boolean_coercion(self):
        self.assertTrue(foundry.same_json({'sensorRadius': 12.0, 'position': [0.0]},
                                         {'sensorRadius': 12, 'position': [0]}))
        self.assertFalse(foundry.same_json({'sensorRadius': 1.0}, {'sensorRadius': True}))
        self.assertFalse(foundry.same_json({'x': 1}, {'x': 2}))
        self.assertFalse(foundry.same_json({'x': 1}, {'x': 1, 'extra': 0}))

    def test_creator_receives_role_not_private_world_facts(self):
        source = ledger()
        source['events'][0]['payload'].update(available_metal=987654,
            private_treaty='SECRET MINING TERMS', current_orders=['SECRET ATTACK TARGET'])
        controller = self.controller()
        controller.sync(source)
        controller.step()
        exported = json.dumps(self.api.jobs[0])
        for secret in ('987654', 'SECRET MINING TERMS', 'SECRET ATTACK TARGET',
                       'need-event-1', 'helios', 'relay-need'):
            self.assertNotIn(secret, exported)
        self.assertIn('sensor_relay', exported)


if __name__ == '__main__':
    unittest.main()
