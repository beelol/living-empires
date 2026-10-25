import assert from 'node:assert/strict';

const base = (process.env.BASE_URL || 'http://127.0.0.1:4317').replace(/\/$/, '');
const terminal = new Set(['accepted', 'active', 'failed', 'blocked', 'cancelled', 'exhausted']);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function request(path, {method = 'GET', body} = {}) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: body === undefined ? {} : {'content-type': 'application/json'},
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  const contentType = response.headers.get('content-type') || '';
  const value = contentType.includes('application/json') ? await response.json() : new Uint8Array(await response.arrayBuffer());
  return {response, value};
}

async function createFixtureJob(themeId, injectDefect) {
  const {response, value} = await request('/api/jobs', {
    method: 'POST',
    body: {
      themeId,
      provider: 'fixture',
      prompt: `Local smoke rehearsal for ${themeId} world generation.`,
      injectDefect
    }
  });
  if (response.status === 409) {
    console.log('SKIP: a run is already in progress; leaving the existing job untouched.');
    process.exitCode = 2;
    return null;
  }
  assert.equal(response.status, 201, `Job creation failed (${response.status}): ${value.error || JSON.stringify(value)}`);
  return value;
}

async function waitForTerminal(id, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const {response, value: job} = await request(`/api/jobs/${id}`);
    assert.equal(response.status, 200, `Could not read job ${id}`);
    if (terminal.has(job.status)) return job;
    await sleep(400);
  }
  throw new Error(`Job ${id} did not reach a terminal state within ${timeoutMs / 1000}s`);
}

async function expectFixtureSuccess(job, expectedAttempts) {
  const result = await waitForTerminal(job.id);
  assert.equal(result.provider, 'fixture', 'Smoke job must use offline fixture mode');
  assert.equal(result.status, 'accepted', `Expected accepted, got ${result.status}: ${result.stopReason || ''}`);
  assert.equal(result.attempts.length, expectedAttempts, `Expected ${expectedAttempts} candidate(s)`);
  for (const attempt of result.attempts) {
    assert.equal(attempt.images.length, 3, `Candidate ${attempt.number} should have three rendered PNGs`);
    for (const imageUrl of attempt.images) {
      assert.match(imageUrl, /^\/evidence\/[\da-f-]+\/\d+-[0-2]\.png$/, `Unexpected evidence URL: ${imageUrl}`);
      const {response, value: bytes} = await request(imageUrl);
      assert.equal(response.status, 200, `Evidence did not load: ${imageUrl}`);
      assert.match(response.headers.get('content-type') || '', /^image\/png\b/, `Evidence is not served as PNG: ${imageUrl}`);
      assert.deepEqual(Array.from(bytes.slice(0, 8)), [137, 80, 78, 71, 13, 10, 26, 10], `Invalid PNG signature: ${imageUrl}`);
    }
  }
  return result;
}

async function main() {
  console.log(`OFFLINE REHEARSAL: ${base}; fixture mode is selected explicitly, so this smoke test makes no live API calls.`);

  const firstCreated = await createFixtureJob('argonaut', true);
  if (!firstCreated) return;
  const first = await expectFixtureSuccess(firstCreated, 2);
  assert.equal(first.attempts[0].checks.find(check => check.id === 'GEN-05.access')?.status, 'fail', 'Injected first candidate should fail access');
  assert.equal(first.attempts[1].checks.find(check => check.id === 'GEN-05.access')?.status, 'pass', 'Repaired candidate should pass access');
  const exported=await request(`/api/jobs/${first.id}/package`);
  assert.equal(exported.response.status,200,'Accepted asset package should export');
  assert.equal(exported.value.format,'living-empires.asset');
  assert.equal(exported.value.schemaVersion,1);
  assert.equal(exported.value.manifest.rehearsal,true,'Fixture package must remain visibly marked');
  assert.equal(exported.value.manifest.contentSha256,first.acceptedHash);
  assert.equal(exported.value.manifest.evidence.length,3);
  assert.equal(exported.value.themeId,'argonaut');
  const staleRender=await request('/api/render-task',{method:'POST',body:{token:'stale',jobId:first.id,attemptNumber:1,candidateHash:first.attempts[0].hash}});
  assert.equal(staleRender.response.status,409,'Stale rendering evidence must be rejected');

  const {response: activationResponse, value: release} = await request(`/api/jobs/${first.id}/activate`, {method: 'POST', body: {}});
  assert.equal(activationResponse.status, 200, `Accepted candidate activation failed: ${JSON.stringify(release)}`);
  assert.equal(release.hash, first.acceptedHash, 'Activated release must match the accepted candidate hash');
  const duplicate = await request(`/api/jobs/${first.id}/activate`, {method: 'POST', body: {}});
  assert.equal(duplicate.response.status, 409, 'A second activation of the same job must be rejected');
  console.log(`PASS: argonaut defect was repaired in two candidates; all six rendered PNGs load; accepted release ${release.hash.slice(0, 10)} activated once.`);

  const secondCreated = await createFixtureJob('sylvan', false);
  if (!secondCreated) return;
  const second = await expectFixtureSuccess(secondCreated, 1);
  const secondActivation = await request(`/api/jobs/${second.id}/activate`, {method: 'POST', body: {}});
  assert.equal(secondActivation.response.status, 200, `Second theme activation failed: ${JSON.stringify(secondActivation.value)}`);
  assert.equal(secondActivation.value.hash, second.acceptedHash, 'Second release must match its accepted candidate');
  const rolledBack = await request('/api/rollback', {method: 'POST', body: {}});
  assert.equal(rolledBack.response.status, 200, `Rollback failed: ${JSON.stringify(rolledBack.value)}`);
  assert.equal(rolledBack.value.hash, release.hash, 'Rollback should restore the first activated release');
  console.log(`PASS: sylvan fixture candidate activated and rollback restored ${release.hash.slice(0, 10)}.`);

  const cancelCreated = await createFixtureJob('ember', true);
  if (!cancelCreated) return;
  const activeBeforeCancel = await request('/api/release');
  assert.equal(activeBeforeCancel.response.status, 200);
  const cancelled = await request(`/api/jobs/${cancelCreated.id}/cancel`, {method: 'POST', body: {}});
  assert.equal(cancelled.response.status, 200, `Immediate cancellation failed: ${JSON.stringify(cancelled.value)}`);
  await sleep(500);
  const afterCancel = await request(`/api/jobs/${cancelCreated.id}`);
  assert.equal(afterCancel.response.status, 200);
  assert.equal(afterCancel.value.status, 'cancelled', `Cancelled job ended as ${afterCancel.value.status}`);
  assert.notEqual(afterCancel.value.status, 'accepted', 'Cancelled job must not become accepted');
  assert.equal(afterCancel.value.cancelled, true, 'Cancellation flag must remain set');
  const cancelledActivation = await request(`/api/jobs/${cancelCreated.id}/activate`, {method: 'POST', body: {}});
  assert.equal(cancelledActivation.response.status, 409, 'Cancelled job must remain ineligible for activation');
  const cancelledExport=await request(`/api/jobs/${cancelCreated.id}/package`);
  assert.equal(cancelledExport.response.status,409,'Cancelled candidates cannot export approved asset packages');
  const activeAfterCancel = await request('/api/release');
  assert.equal(activeAfterCancel.response.status, 200);
  assert.equal(activeAfterCancel.value?.hash, activeBeforeCancel.value?.hash, 'Cancelled work must leave the active release unchanged');
  console.log('PASS: immediate cancellation left the third job cancelled and ineligible for activation.');
  console.log('All offline rehearsal smoke checks passed.');
}

main().catch(error => {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
});
