const MAX_FAST_TIME_MS = 300_000;

/** Frozen profile for new creations; zero explicitly means no overall deadline. */
export function fastLimits(timeBudgetMs = 0) {
  if (!Number.isSafeInteger(timeBudgetMs) || timeBudgetMs < 0 || timeBudgetMs > MAX_FAST_TIME_MS) {
    throw new Error('Fast generation requires 0 (no deadline) or 1–300000 ms.');
  }
  return {maxAttempts: 30, maxProviderCalls: 60, timeBudgetMs, requiredImageHashes: 3};
}

/** Retry only reconciled work, inheriting the root job's remaining group budget. */
export function retryLimits(root, attempts, calls, now = Date.now()) {
  const original = root?.pipeline?.contract?.limits;
  if (!original || !Number.isSafeInteger(attempts) || attempts < 0 || !Number.isSafeInteger(calls) || calls < 0) {
    throw new Error('The original job group budget is unavailable.');
  }
  const noDeadline = root.deadline === null && original.timeBudgetMs === 0;
  if (!noDeadline && !Number.isFinite(root.deadline)) throw new Error('The original job deadline is unavailable.');
  const remaining = noDeadline ? 0 : root.deadline - now;
  if (calls >= original.maxProviderCalls || attempts >= original.maxAttempts || (!noDeadline && remaining < 1_000)) {
    throw new Error('The original job group call, candidate, or time limit is exhausted.');
  }
  return {
    maxAttempts: original.maxAttempts - attempts,
    maxProviderCalls: original.maxProviderCalls - calls,
    timeBudgetMs: noDeadline ? 0 : remaining,
    requiredImageHashes: 3,
  };
}

/** Cap each local operation without creating an overall timeout for no-deadline jobs. */
export function remainingTime(deadline, ceiling, now = Date.now()) {
  if (!Number.isSafeInteger(ceiling) || ceiling < 1) throw new Error('Operation timeout ceiling must be positive.');
  if (deadline === null) return ceiling;
  if (!Number.isFinite(deadline)) throw new Error('The frozen job deadline is unavailable.');
  const remaining = deadline - now;
  if (remaining <= 0) throw new Error('Job deadline exhausted.');
  return Math.min(ceiling, remaining);
}

export function armDeadline(deadline, controller, onExpire = () => {}) {
  if (deadline === null) return () => {};
  if (!Number.isFinite(deadline)) throw new Error('The frozen job deadline is unavailable.');
  const timer = setTimeout(() => {
    onExpire();
    controller.abort(new Error('Job deadline exhausted.'));
  }, Math.max(0, deadline - Date.now()));
  timer.unref?.();
  return () => clearTimeout(timer);
}
