// Subprocess transport fixtures, not claims of native rendering or asset approval.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {captureNative} from '../lib/native-capture.mjs';

function setup(t, {mismatch=false, mode='success'}={}){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'foundry-native-adapter-'));
  t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
  const binary=path.join(root,'fixture-renderer.mjs');
  fs.writeFileSync(binary,`#!/usr/bin/env node
import fs from 'node:fs'; import path from 'node:path';
const p=JSON.parse(fs.readFileSync(process.argv[2],'utf8')),out=process.argv[3];
const attemptDir=path.dirname(process.argv[2]);
if(${JSON.stringify(mode)}==='fail'){process.stderr.write('fixture renderer diagnostic\\n'+'x'.repeat(40000),()=>process.exit(7));}
if(${JSON.stringify(mode)}==='ignore-term'){
  process.on('SIGTERM',()=>fs.writeFileSync(path.join(attemptDir,'term-received'),'yes'));
  process.stderr.write('renderer waiting for termination\\n');
  setInterval(()=>{},1000);
  setTimeout(()=>process.exit(0),15000);
}
fs.writeFileSync(path.join(attemptDir,'started'),String(process.pid));
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

function attemptFolder(input){
  return fs.readdirSync(path.join(input.root,'.data',input.job.id))
    .map(name=>path.join(input.root,'.data',input.job.id,name))
    .find(folder=>nameIsAttempt(folder,input.attempt.number));
}

function nameIsAttempt(folder,number){return path.basename(folder).startsWith(`native-${number}-`);}

async function waitForFile(file,timeoutMs=2000){
  const deadline=Date.now()+timeoutMs;
  while(Date.now()<deadline){if(fs.existsSync(file))return;await new Promise(resolve=>setTimeout(resolve,10));}
  assert.fail(`Timed out waiting for ${file}`);
}

function failOnce(t,method,shouldFail,message='injected filesystem failure'){
  const original=fs[method];let failed=false;
  fs[method]=(...args)=>{
    if(!failed&&shouldFail(...args)){failed=true;throw new Error(message);}
    return original(...args);
  };
  t.after(()=>{fs[method]=original;});
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
  const input=setup(t,{mismatch:true});
  await assert.rejects(captureNative(input),/candidate/);
  assert.equal(fs.existsSync(path.join(input.root,'.data',input.job.id,'1-0.png')),false);
});

test('renderer failure retains bounded stderr and the previous public evidence',async t=>{
  const input=setup(t,{mode:'fail'}),folder=path.join(input.root,'.data',input.job.id);
  fs.mkdirSync(folder,{recursive:true});fs.writeFileSync(path.join(folder,'1-0.png'),'prior evidence');
  await assert.rejects(captureNative(input),/Native renderer failed \(7\)/);
  const evidenceDir=attemptFolder(input),log=fs.readFileSync(path.join(evidenceDir,'renderer.log'),'utf8');
  assert.match(log,/fixture renderer diagnostic/);
  assert.match(log,/Native renderer failed \(7\)/);
  assert.ok(Buffer.byteLength(log)<=32768);
  assert.equal(fs.readFileSync(path.join(folder,'1-0.png'),'utf8'),'prior evidence');
});

test('timeout escalates termination and waits for renderer exit before rejecting',async t=>{
  const input=setup(t,{mode:'ignore-term'}),folder=path.join(input.root,'.data',input.job.id);
  fs.mkdirSync(folder,{recursive:true});fs.writeFileSync(path.join(folder,'1-0.png'),'prior evidence');
  await assert.rejects(captureNative({...input,timeoutMs:5000}),/Native capture deadline exceeded/);
  const evidenceDir=attemptFolder(input),pid=Number(fs.readFileSync(path.join(evidenceDir,'started'),'utf8'));
  assert.ok(fs.existsSync(path.join(evidenceDir,'term-received')));
  const log=fs.readFileSync(path.join(evidenceDir,'renderer.log'),'utf8');
  assert.match(log,/renderer waiting for termination/);
  assert.match(log,/Native capture deadline exceeded/);
  assert.throws(()=>process.kill(pid,0),{code:'ESRCH'});
  assert.equal(fs.readFileSync(path.join(folder,'1-0.png'),'utf8'),'prior evidence');
});

test('cancellation terminates and waits for a running renderer before rejecting',async t=>{
  const input=setup(t,{mode:'ignore-term'}),controller=new AbortController();
  const publicFolder=path.join(input.root,'.data',input.job.id);
  fs.mkdirSync(publicFolder,{recursive:true});fs.writeFileSync(path.join(publicFolder,'1-0.png'),'prior evidence');
  const capture=captureNative({...input,timeoutMs:5000,signal:controller.signal});
  const dir=publicFolder;
  await waitForFile(path.join(dir,fs.readdirSync(dir).find(name=>name.startsWith('native-1-')),'started'));
  controller.abort();
  await assert.rejects(capture,/Native capture cancelled/);
  const evidenceDir=attemptFolder(input),pid=Number(fs.readFileSync(path.join(evidenceDir,'started'),'utf8'));
  assert.ok(fs.existsSync(path.join(evidenceDir,'term-received')));
  const log=fs.readFileSync(path.join(evidenceDir,'renderer.log'),'utf8');
  assert.match(log,/renderer waiting for termination/);
  assert.match(log,/Native capture cancelled/);
  assert.throws(()=>process.kill(pid,0),{code:'ESRCH'});
  assert.equal(fs.readFileSync(path.join(publicFolder,'1-0.png'),'utf8'),'prior evidence');
});

test('an already-aborted capture never starts the renderer and retains the cancellation reason',async t=>{
  const input=setup(t),controller=new AbortController();controller.abort();
  await assert.rejects(captureNative({...input,signal:controller.signal}),/Native capture cancelled/);
  const evidenceDir=attemptFolder(input);
  assert.equal(fs.existsSync(path.join(evidenceDir,'started')),false);
  assert.match(fs.readFileSync(path.join(evidenceDir,'renderer.log'),'utf8'),/Native capture cancelled/);
});

test('spawn failure retains its diagnostic and leaves public evidence untouched',async t=>{
  const input=setup(t),folder=path.join(input.root,'.data',input.job.id);
  fs.mkdirSync(folder,{recursive:true});fs.writeFileSync(path.join(folder,'1-0.png'),'prior evidence');
  fs.chmodSync(input.binary,0o644);
  await assert.rejects(captureNative(input),/Native renderer could not start/);
  const evidenceDir=attemptFolder(input),log=fs.readFileSync(path.join(evidenceDir,'renderer.log'),'utf8');
  assert.match(log,/Native renderer could not start/);
  assert.ok(Buffer.byteLength(log)<=32768);
  assert.equal(fs.readFileSync(path.join(folder,'1-0.png'),'utf8'),'prior evidence');
});

test('evidence manifest staging failure preserves all public image bytes',async t=>{
  const input=setup(t),folder=path.join(input.root,'.data',input.job.id),prior=['prior-front','prior-side','prior-rear'];
  fs.mkdirSync(folder,{recursive:true});
  prior.forEach((bytes,index)=>fs.writeFileSync(path.join(folder,`1-${index}.png`),bytes));
  failOnce(t,'writeFileSync',file=>path.basename(String(file)).startsWith('evidence.json'),'injected evidence manifest staging failure');
  await assert.rejects(captureNative(input),/injected evidence manifest staging failure/);
  prior.forEach((bytes,index)=>assert.equal(fs.readFileSync(path.join(folder,`1-${index}.png`),'utf8'),bytes));
});

test('second image publication failure restores all prior public image bytes',async t=>{
  const input=setup(t),folder=path.join(input.root,'.data',input.job.id),prior=['prior-front','prior-side','prior-rear'];
  fs.mkdirSync(folder,{recursive:true});
  prior.forEach((bytes,index)=>fs.writeFileSync(path.join(folder,`1-${index}.png`),bytes));
  failOnce(t,'renameSync',(source,target)=>path.basename(String(target))==='1-1.png'&&path.basename(String(source))==='staged-2.png','injected second frame publication failure');
  await assert.rejects(captureNative(input),/injected second frame publication failure/);
  prior.forEach((bytes,index)=>assert.equal(fs.readFileSync(path.join(folder,`1-${index}.png`),'utf8'),bytes));
});

test('failed second image publication removes newly published frames when no prior images exist',async t=>{
  const input=setup(t),folder=path.join(input.root,'.data',input.job.id);
  failOnce(t,'renameSync',(source,target)=>path.basename(String(target))==='1-1.png'&&path.basename(String(source))==='staged-2.png','injected second frame publication failure');
  await assert.rejects(captureNative(input),/injected second frame publication failure/);
  for(let index=0;index<3;index++)assert.equal(fs.existsSync(path.join(folder,`1-${index}.png`)),false);
});

test('rollback failure is reported with the surviving backup location',async t=>{
  const input=setup(t),folder=path.join(input.root,'.data',input.job.id),original=fs.renameSync;
  fs.mkdirSync(folder,{recursive:true});fs.writeFileSync(path.join(folder,'1-0.png'),'prior-front');
  fs.writeFileSync(path.join(folder,'1-1.png'),'prior-side');fs.writeFileSync(path.join(folder,'1-2.png'),'prior-rear');
  let failedPublish=false,failedRestore=false;
  fs.renameSync=(source,target)=>{
    if(!failedPublish&&path.basename(String(target))==='1-1.png'&&path.basename(String(source))==='staged-2.png'){
      failedPublish=true;throw new Error('injected second frame publication failure');
    }
    if(failedPublish&&!failedRestore&&path.basename(String(source))==='prior-0.png'){
      failedRestore=true;throw new Error('injected rollback failure');
    }
    return original(source,target);
  };
  t.after(()=>{fs.renameSync=original;});
  await assert.rejects(captureNative(input),/rollback incomplete.*1-0\.png.*prior-0\.png/i);
  const evidenceDir=attemptFolder(input);
  assert.ok(fs.existsSync(path.join(evidenceDir,'prior-0.png')));
  assert.notEqual(fs.readFileSync(path.join(folder,'1-0.png'),'utf8'),'prior-front');
});

test('baseline capture uses attempt zero and the frozen wider review extent',async t=>{
 const input=setup(t);input.attempt.number=0;input.job.sceneHalfExtent=12;
 const result=await captureNative(input);assert.match(result.images[0],/0-0.png$/);
});
test('scene extent drift is rejected before native capture',async t=>{
 const input=setup(t);input.job.sceneHalfExtent=12;input.attempt.recipe.scene={halfExtent:8};
 await assert.rejects(captureNative(input),/frozen native review extent/);
});

test('configured data directory receives native evidence and preserves its browser frames',async t=>{
 const input=setup(t);input.dataDir=path.join(input.root,'custom-data');
 const folder=path.join(input.dataDir,input.job.id);fs.mkdirSync(folder,{recursive:true});
 fs.writeFileSync(path.join(folder,'1-0.png'),'browser-frame');
 const result=await captureNative(input);
 assert.equal(fs.existsSync(path.join(input.root,'.data',input.job.id,'1-0.png')),false);
 assert.equal(fs.readFileSync(path.join(folder,'1-0.png')).subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 const evidenceFolder=path.dirname(path.dirname(path.join(folder,result.nativeEvidence.views[0].sourceFile)));
 assert.equal(fs.readFileSync(path.join(evidenceFolder,'browser-1.png'),'utf8'),'browser-frame');
});
