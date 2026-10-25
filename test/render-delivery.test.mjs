import test from 'node:test';
import assert from 'node:assert/strict';
import { createRenderDelivery } from '../public/render-delivery.mjs';

test('acknowledged captured evidence is submitted once', async () => {
  let calls = 0;
  const delivery = createRenderDelivery(async () => { calls += 1; });
  const payload = { token: 'token', candidateHash: 'hash', images: ['one', 'two', 'three'] };

  assert.equal(await delivery.send('token', payload), true);
  assert.equal(delivery.has('token'), true);
  assert.equal(delivery.pending('token'), false);
  assert.equal(await delivery.send('token', { ...payload, images: [] }), true);
  assert.equal(calls, 1);
});

test('transient failure retries the same captured payload and only marks it after acknowledgement', async () => {
  let calls = 0;
  const seen = [];
  const delivery = createRenderDelivery(async payload => {
    seen.push(payload);
    if (++calls === 1) throw new Error('connection reset');
  });
  const payload = { token: 'token', candidateHash: 'hash', images: ['overview', 'reverse', 'entrance'], movement: { pass: true } };

  assert.equal(await delivery.send('token', payload), false);
  assert.equal(delivery.has('token'), false);
  assert.equal(delivery.pending('token'), true);
  assert.equal(await delivery.send('token'), true);
  assert.equal(delivery.has('token'), true);
  assert.equal(delivery.pending('token'), false);
  assert.equal(seen.length, 2);
  assert.equal(seen[0], payload);
  assert.equal(seen[1], payload);
});

test('final transport failure stops at its retry bound and retains the exact pending payload', async () => {
  let calls = 0;
  const seen = [];
  const delivery = createRenderDelivery(async payload => {
    seen.push(payload);
    calls += 1;
    throw Object.assign(new Error('renderer endpoint unavailable'), { status: 503 });
  }, { maxAttempts: 3 });
  const payload = { token: 'token', candidateHash: 'hash', images: ['one', 'two', 'three'] };

  assert.equal(await delivery.send('token', payload), false);
  assert.equal(await delivery.send('token'), false);
  assert.equal(await delivery.send('token'), false);
  assert.equal(delivery.has('token'), false);
  assert.equal(delivery.pending('token'), true);
  assert.equal(delivery.exhausted('token'), true);
  assert.equal(await delivery.send('token'), false);
  assert.equal(calls, 3);
  assert.deepEqual(seen, [payload, payload, payload]);
  assert.ok(seen.every(value => value === payload));
});

test('stale and invalid submissions terminate instead of retrying', async () => {
  for (const status of [400, 409]) {
    let calls = 0;
    const delivery = createRenderDelivery(async () => {
      calls += 1;
      throw Object.assign(new Error('render task cannot accept this result'), { status });
    });

    assert.equal(await delivery.send(`terminal-${status}`, { token: `terminal-${status}`, error: 'capture failed' }), false);
    assert.equal(delivery.has(`terminal-${status}`), true);
    assert.equal(delivery.pending(`terminal-${status}`), false);
    assert.equal(await delivery.send(`terminal-${status}`), true);
    assert.equal(calls, 1);
  }
});
