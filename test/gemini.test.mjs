import test from 'node:test';
import assert from 'node:assert/strict';
import { createCandidate, extractManagedText, parseManagedRecipe, parseReviewResponse, reviewCandidate } from '../lib/gemini.mjs';

const schema = { type: 'object', required: ['name'], properties: { name: { type: 'string' } } };
const ok = body => ({ ok: true, status: 200, json: async () => body });

test('managed final text is parsed and interaction/environment state is retained', async () => {
  let request;
  const result = await createCandidate({
    provider: 'managed', apiKey: 'test-key', model: 'model', agent: 'agent', prompt: 'make it', recipeSchema: schema,
    fetchImpl: async (url, init) => { request = { url, init, body: JSON.parse(init.body) }; return ok({
      id: 'next-interaction', environment_id: 'sandbox-1', output_text: '```json\n{"name":"Outpost"}\n```', usage: { total_tokens: 8 },
    }); },
  });
  assert.deepEqual(result.recipe, { name: 'Outpost' });
  assert.equal(result.interactionId, 'next-interaction');
  assert.equal(result.environmentId, 'sandbox-1');
  assert.deepEqual(result.usage, { total_tokens: 8 });
  assert.equal(request.body.environment, 'remote');
  assert.equal(request.body.input[0].text, 'make it');
  assert.equal(request.init.headers['x-goog-api-key'], 'test-key');
});

test('managed continuation sends both independent state identifiers', async () => {
  let body;
  await createCandidate({
    provider: 'managed', apiKey: 'key', model: 'model', agent: 'agent', prompt: 'repair', recipeSchema: schema,
    interactionId: 'previous', environmentId: 'sandbox-1',
    fetchImpl: async (_url, init) => { body = JSON.parse(init.body); return ok({ output_text: '{"name":"Repaired"}' }); },
  });
  assert.equal(body.previous_interaction_id, 'previous');
  assert.equal(body.environment, 'sandbox-1');
});

test('managed text extraction accepts final output blocks and refuses missing final text', () => {
  assert.equal(extractManagedText({ outputs: [{ type: 'reasoning', text: 'ignore' }, { type: 'text', text: 'final' }] }), 'final');
  assert.deepEqual(parseManagedRecipe({ outputs: [{ type: 'text', content: [{ type: 'text', text: '{"name":"Block"}' }] }] }), { name: 'Block' });
  assert.throws(() => extractManagedText({ steps: [{ type: 'reasoning', text: 'not the answer' }] }), /no final text/);
});

test('Gemini creator requests structured JSON and parses the returned candidate', async () => {
  let request;
  const result = await createCandidate({
    provider: 'gemini', apiKey: 'key', model: 'gemini-test', prompt: 'brief', recipeSchema: schema,
    fetchImpl: async (url, init) => { request = { url, body: JSON.parse(init.body) }; return ok({
      candidates: [{ content: { parts: [{ text: '{"name":"Tower"}' }] } }], usageMetadata: { totalTokenCount: 12 },
    }); },
  });
  assert.deepEqual(result.recipe, { name: 'Tower' });
  assert.equal(request.body.generationConfig.responseMimeType, 'application/json');
  assert.deepEqual(request.body.generationConfig.responseJsonSchema, schema);
  assert.equal(result.usage.totalTokenCount, 12);
});

test('provider errors are sanitized and not retried', async () => {
  let calls = 0;
  await assert.rejects(createCandidate({
    provider: 'gemini', apiKey: 'private-key', model: 'm', prompt: 'brief', recipeSchema: schema,
    fetchImpl: async () => { calls++; return { ok: false, status: 403, text: async () => 'contains private-key' }; },
  }), /Gemini request failed \(403\)/);
  assert.equal(calls, 1);
});

test('review validates verdicts and sends all three actual PNG buffers', async () => {
  const images = [Buffer.from('png one'), Buffer.from('png two'), Buffer.from('png three')];
  let body;
  const result = await reviewCandidate({
    apiKey: 'key', model: 'review-model', theme: { name: 'Frozen', palette: ['blue'] }, prompt: 'an outpost', images,
    fetchImpl: async (_url, init) => { body = JSON.parse(init.body); return ok({
      candidates: [{ content: { parts: [{ text: '```json\n{"themeStatus":"pass","renderStatus":"fail","findings":"View 2: roof floats."}\n```' }] } }],
      usageMetadata: { totalTokenCount: 20 },
    }); },
  });
  assert.equal(result.themeStatus, 'pass');
  assert.equal(result.renderStatus, 'fail');
  assert.match(result.findings, /View 2/);
  assert.deepEqual(body.contents[0].parts.slice(1).map(part => Buffer.from(part.inlineData.data, 'base64')), images);
  assert.equal(body.generationConfig.responseMimeType, 'application/json');
});

test('reviewer rejects malformed JSON, invalid verdicts, and missing evidence', async () => {
  assert.throws(() => parseReviewResponse('{broken'), /malformed JSON/);
  assert.throws(() => parseReviewResponse('{"themeStatus":"maybe","renderStatus":"pass","findings":""}'), /invalid verdict/);
  await assert.rejects(reviewCandidate({ apiKey: 'key', model: 'm', theme: {}, prompt: 'p', images: [Buffer.from('one')], fetchImpl: async () => ok({}) }), /exactly three/);
});
