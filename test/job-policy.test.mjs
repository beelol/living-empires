import test from 'node:test';
import assert from 'node:assert/strict';
import {fastLimits, retryLimits, remainingTime, armDeadline} from '../lib/job-policy.mjs';

test('new completion jobs freeze 30 attempts, 60 provider calls and no overall deadline', () => {
  assert.deepEqual(fastLimits(), {maxAttempts: 30, maxProviderCalls: 60, timeBudgetMs: 0, requiredImageHashes: 3});
  for (const value of [-1, 300001, 1.5, NaN, '300000']) assert.throws(() => fastLimits(value));
});

test('retry groups inherit the original finite or no-deadline profile and remaining counters', () => {
  const legacy = {deadline: 1_000_000, pipeline: {contract: {limits: {maxAttempts: 3, maxProviderCalls: 6, timeBudgetMs: 1_200_000}}}};
  assert.deepEqual(retryLimits(legacy, 2, 3, 900_000), {maxAttempts: 1, maxProviderCalls: 3, timeBudgetMs: 100_000, requiredImageHashes: 3});
  assert.throws(() => retryLimits(legacy, 3, 3, 900_000), /exhausted/);
  assert.throws(() => retryLimits(legacy, 2, 6, 900_000), /exhausted/);
  assert.throws(() => retryLimits(legacy, 1, 1, 1_000_000), /exhausted/);
  const fast = {deadline: null, pipeline: {contract: {limits: {maxAttempts: 30, maxProviderCalls: 60, timeBudgetMs: 0}}}};
  assert.deepEqual(retryLimits(fast, 12, 24, Number.MAX_SAFE_INTEGER), {maxAttempts: 18, maxProviderCalls: 36, timeBudgetMs: 0, requiredImageHashes: 3});
});

test('per-operation timeouts stay bounded without extending a frozen finite deadline', () => {
  assert.equal(remainingTime(1_100, 120_000, 1_000), 100);
  assert.equal(remainingTime(null, 120_000, 1_000), 120_000);
  assert.throws(() => remainingTime(1_000, 120_000, 1_000), /deadline/i);
});

test('a null overall deadline does not arm an abort timer, while a finite legacy deadline still does', async () => {
  const noDeadline = new AbortController();
  let expired = 0;
  armDeadline(null, noDeadline, () => expired++)();
  await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(noDeadline.signal.aborted, false);
  assert.equal(expired, 0);

  const finite = new AbortController();
  const stop = armDeadline(Date.now() + 15, finite, () => expired++);
  await new Promise(resolve => setTimeout(resolve, 30));
  stop();
  assert.equal(finite.signal.aborted, true);
  assert.equal(expired, 1);
});
