// Subprocess transport fixtures, not claims of native rendering or asset approval.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {captureNative} from '../lib/native-capture.mjs';

function setup(t, mismatch=false){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'foundry-native-adapter-'));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const binary=path.join(root,'fixture-renderer.mjs');
  fs.writeFileSync(binary,`#!/usr/bin/env node
import fs from 'node:fs'; import path from 'node:path';
const p=JSON.parse(fs.readFileSync(process.argv[2],'utf8')),out=process.argv[3];
fs.mkdirSync(out,{recursive:true});
const names=['front','three-quarter','rear'];
const views=names.map((n,i)=>{const file='view-'+(i+1)+'-'+n+'.png';fs.writeFileSync(path.join(out,file),Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),Buffer.from('fixture-'+i)]));return {file};});
fs.writeFileSync(path.join(out,'capture-manifest.json'),JSON.stringify({package:{candidateHash:${mismatch?"'0'.repeat(64)":"p.manifest.contentSha256"}},views,renderer:'Bevy 0.20 native'}));
`);
  fs.chmodSync(binary,0o755);
  const job={id:'job-test-1',provider:'fixture',model:'fixture',themeId:'argonaut',theme:{id:'argonaut'},sourceRevision:'fixture'};
  const attempt={number:1,hash:'a'.repeat(64),recipe:{themeId:'argonaut',parts:[]}};
  return {root,binary,job,attempt,movement:{pass:true,trace:[[0,0],[1,0]]}};
}

test('native adapter binds exact candidate and all three PNGs without inventing native movement',async t=>{
  const input=setup(t);
  const result=await captureNative(input);
  assert.deepEqual(result.images,['/evidence/job-test-1/1-0.png','/evidence/job-test-1/1-1.png','/evidence/job-test-1/1-2.png']);
  assert.equal(result.nativeEvidence.candidateSha256,'a'.repeat(64));
  assert.equal(result.functionalHarness,'three-browser');
  assert.deepEqual(result.movement,input.movement);
  assert.equal(new Set(result.nativeEvidence.views.map(v=>v.sha256)).size,3);
  assert.match(result.nativeEvidence.rendererBinarySha256,/^[a-f0-9]{64}$/);
  for(const v of result.nativeEvidence.views)assert.ok(fs.existsSync(path.join(input.root,'.data',input.job.id,v.publicFile)));
});

test('wrong native candidate identity never replaces public evidence',async t=>{
  const input=setup(t,true);
  await assert.rejects(captureNative(input),/candidate/);
  assert.equal(fs.existsSync(path.join(input.root,'.data',input.job.id,'1-0.png')),false);
});
