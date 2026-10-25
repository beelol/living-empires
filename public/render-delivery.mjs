const TRANSIENT_CLIENT_ERRORS = new Set([408, 425, 429]);

/** Keep one captured result per token until delivery is acknowledged or rejected. */
export function createRenderDelivery(post, { maxAttempts = 5 } = {}) {
  const pending = new Map();
  const submitted = new Set();
  const limit = Number.isFinite(maxAttempts) ? Math.max(1, Math.floor(maxAttempts)) : 5;

  function isTerminal(error) {
    const status = Number(error?.status);
    if (!Number.isInteger(status) || status < 400 || status >= 500) return false;
    return status >= 400 && status < 500 && !TRANSIENT_CLIENT_ERRORS.has(status);
  }

  async function send(key, payload) {
    if (submitted.has(key)) return true;

    let entry = pending.get(key);
    if (!entry) {
      if (payload === undefined) return false;
      entry = { payload, attempts: 0, exhausted: false, running: null };
      pending.set(key, entry);
    }
    if (entry.running) return entry.running;
    if (entry.exhausted || entry.attempts >= limit) {
      entry.exhausted = true;
      return false;
    }

    entry.attempts += 1;
    entry.running = (async () => {
      try {
        await post(entry.payload);
        submitted.add(key);
        pending.delete(key);
        return true;
      } catch (error) {
        if (isTerminal(error)) {
          submitted.add(key);
          pending.delete(key);
        } else if (entry.attempts >= limit) {
          entry.exhausted = true;
        }
        return false;
      } finally {
        entry.running = null;
      }
    })();
    return entry.running;
  }

  return {
    has: key => submitted.has(key),
    pending: key => pending.has(key),
    exhausted: key => pending.get(key)?.exhausted === true,
    send,
  };
}
