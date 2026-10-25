import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {THEMES} from '../public/themes.mjs';
import {injectDefect} from '../public/core.mjs';

function call(input,ok=true){
  const result=spawnSync(process.env.RUST_CORE_BIN||'target/debug/world-check',[],{input:JSON.stringify(input),encoding:'utf8',maxBuffer:2_000_000});
  const value=JSON.parse(result.stdout||'{}');
  if(ok)assert.equal(result.status,0,value.error||result.stderr||String(result.error));else assert.notEqual(result.status,0);
  return value;
}
const evidence=hash=>({candidateHash:hash,imageHashes:['1'.repeat(64),'2'.repeat(64),'3'.repeat(64)]});
test('Rust CLI owns generation, evidenced repair, approval and versioned rehearsal export',()=>{
  let pipeline=call({op:'create',theme:THEMES.argonaut,prompt:'Build a ceramic sensor outpost',now:1000,provenance:{rehearsal:true,provider:'fixture',model:'fixture',agent:null,sourceRevision:'test',sourceFingerprint:'test'}}).pipeline;
  const original=call({op:'fixture',theme:THEMES.argonaut}).recipe;
  let result=call({op:'submit',pipeline,recipe:injectDefect(original),now:1001});pipeline=result.pipeline;
  result=call({op:'decide',pipeline,review:{themeStatus:'pass',renderStatus:'pass',findings:[],summary:'Rendered views are consistent.'},evidence:evidence(result.value.candidateHash),now:1002});pipeline=result.pipeline;
  assert.equal(result.value,'repair');
  const repaired=call({op:'fixture',theme:THEMES.argonaut,previous:injectDefect(original)}).recipe;
  result=call({op:'submit',pipeline,recipe:repaired,now:1003});pipeline=result.pipeline;
  const acceptedHash=result.value.candidateHash;
  result=call({op:'decide',pipeline,review:{themeStatus:'pass',renderStatus:'pass',findings:[],summary:'All observable checks passed.'},evidence:evidence(acceptedHash),now:1004});pipeline=result.pipeline;
  assert.equal(result.value,'accept');
  const asset=call({op:'package',pipeline,now:9_000_000}).value;
  assert.equal(asset.format,'living-empires.asset');assert.equal(asset.schemaVersion,1);
  assert.equal(asset.manifest.contentSha256,acceptedHash);assert.equal(asset.manifest.rehearsal,true);
  assert.equal(asset.manifest.provider,'fixture');assert.equal(asset.manifest.sourceRevision,'test');
  assert.deepEqual(asset.theme,THEMES.argonaut);assert.equal(asset.manifest.evidence.length,3);
});
test('Rust CLI rejects contract tampering and cancelled late candidates',()=>{
  let pipeline=call({op:'create',theme:THEMES.ember,prompt:'Build a volcanic sensor outpost',now:1000}).pipeline;
  const tampered=structuredClone(pipeline);tampered.contract.limits.maxAttempts=99;
  assert.match(call({op:'continue',pipeline:tampered,now:1001},false).error,/integrity|tamper|limit|ceiling/i);
  pipeline=call({op:'cancel',pipeline,now:1001}).pipeline;
  const recipe=call({op:'fixture',theme:THEMES.ember}).recipe;
  assert.match(call({op:'submit',pipeline,recipe,now:1002},false).error,/cancel/i);
  assert.match(call({op:'package',pipeline,now:1002},false).error,/approved/i);
});
