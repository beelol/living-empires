import test from 'node:test';
import assert from 'node:assert/strict';
import {chmod, mkdir, mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const hook = path.join(root, '.githooks', 'post-commit');
const sha = 'abc123def456';

async function setup({branch = 'main', failPush = false} = {}) {
  const temp = await mkdtemp(path.join(os.tmpdir(), 'codex-autopush-'));
  const repo = path.join(temp, 'repo');
  const bin = path.join(temp, 'bin');
  await mkdir(repo);
  await mkdir(bin);
  const mockGit = path.join(bin, 'git');
  await writeFile(mockGit, `#!/bin/sh
printf '%s\\n' "$*" >> "$MOCK_GIT_LOG"
if [ "$1" = "-C" ]; then shift 2; fi
case "$*" in
  "rev-parse --show-toplevel") printf '%s\\n' "$MOCK_ROOT" ;;
  "symbolic-ref --quiet --short HEAD") printf '%s\\n' "$MOCK_BRANCH" ;;
  "rev-parse HEAD") printf '%s\\n' "$MOCK_SHA" ;;
  "push origin abc123def456:refs/heads/main")
    [ "$GIT_TERMINAL_PROMPT" = 0 ] || exit 3
    [ "$MOCK_FAIL_PUSH" = 1 ] && exit 1 || exit 0 ;;
  "config codex.lastPushedCommit "*) printf '%s\\n' "$3" > "$MOCK_MARKER" ;;
  *) exit 2 ;;
esac
`);
  await chmod(mockGit, 0o755);
  const log = path.join(temp, 'git.log');
  const marker = path.join(temp, 'last-pushed');
  const result = (extraEnv = {}) => spawnSync(hook, {
    cwd: repo,
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${bin}${path.delimiter}${process.env.PATH ?? ''}`,
      MOCK_ROOT: repo,
      MOCK_BRANCH: branch,
      MOCK_SHA: sha,
      MOCK_FAIL_PUSH: failPush ? '1' : '0',
      MOCK_GIT_LOG: log,
      MOCK_MARKER: marker,
      ...extraEnv,
    },
  });
  const cleanup = () => rm(temp, {recursive: true, force: true});
  return {result, log, marker, cleanup, repo};
}

async function lines(file) {
  try { return (await readFile(file, 'utf8')).trim().split('\n').filter(Boolean); }
  catch { return []; }
}

test('main pushes the captured SHA to origin/main and records it only after success', async () => {
  const testCase = await setup();
  try {
    const result = testCase.result();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stderr, /pushed abc123def456 to origin\/main/);
    const commands = await lines(testCase.log);
    const push = commands.find(line => line.endsWith(`push origin ${sha}:refs/heads/main`));
    assert.equal(push, `-C ${testCase.repo} push origin ${sha}:refs/heads/main`);
    assert.equal(push.includes('--force'), false);
    assert.equal((await readFile(testCase.marker, 'utf8')).trim(), sha);
  } finally { await testCase.cleanup(); }
});

test('non-main branches and controlled opt-out do not push or write a marker', async () => {
  const nonMain = await setup({branch: 'feature/test'});
  try {
    const result = nonMain.result();
    assert.equal(result.status, 0);
    assert.equal((await lines(nonMain.log)).some(line => line.includes(' push ')), false);
    assert.deepEqual(await lines(nonMain.marker), []);
  } finally { await nonMain.cleanup(); }

  const skipped = await setup();
  try {
    const result = skipped.result({CODEX_SKIP_AUTO_PUSH: '1'});
    assert.equal(result.status, 0);
    assert.deepEqual(await lines(skipped.log), []);
    assert.deepEqual(await lines(skipped.marker), []);
  } finally { await skipped.cleanup(); }
});

test('failed push warns, preserves commit success, and leaves the sync marker untouched', async () => {
  const testCase = await setup({failPush: true});
  try {
    const result = testCase.result();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stderr, /push to origin\/main failed/);
    assert.match(result.stderr, /local commit remains committed/);
    assert.deepEqual(await lines(testCase.marker), []);
    const commands = await lines(testCase.log);
    assert.equal(commands.filter(line => line.endsWith(`push origin ${sha}:refs/heads/main`)).length, 1);
    assert.equal(commands.some(line => line.includes('config codex.lastPushedCommit')), false);
  } finally { await testCase.cleanup(); }
});
