import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createApp} from '../server.mjs';
test('local API persists accepted actions, rejects cross-origin writes and recovers once',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-test-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});const base=app.url;
 const request={id:'solar-one',type:'build',kind:'solar',x:12,z:8};
 const post=(body,origin)=>fetch(base+'/api/v1/command',{method:'POST',headers:{'Content-Type':'application/json',...(origin?{Origin:origin}:{})},body:JSON.stringify(body)});
 assert.equal((await post(request,'https://evil.example')).status,403);assert.equal((await post(request)).status,200);assert.equal((await post(request)).status,200);
 let state=await (await fetch(base+'/api/v1/world')).json();assert.equal(state.buildings.length,2);assert.equal(state.alloy,165);
 assert.equal((await fetch(base+'/../.env')).status,404);await app.close();app=await createApp({port:0,dataDir:dir,foundryEnabled:false});state=await (await fetch(app.url+'/api/v1/world')).json();assert.equal(state.buildings.length,2);assert.equal(state.alloy,165);
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});
test('damaged save never resets silently and a second writer cannot open it',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-test-'));let app;try{app=await createApp({port:0,dataDir:dir,foundryEnabled:false});await assert.rejects(()=>createApp({port:0,dataDir:dir}),/already open/);await app.close();app=null;fs.writeFileSync(path.join(dir,'world.json'),'{broken');await assert.rejects(()=>createApp({port:0,dataDir:dir}),/save|JSON/);}finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});
test('ordinary player commands preserve the in-flight Foundry request journal',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-test-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});const f=app.world.foundry,request={id:'pending',kind:'core',status:'dispatching'};f.requests.push(request);
 const response=await fetch(app.url+'/api/v1/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({id:'walk',type:'move',x:12,z:8})});assert.equal(response.status,200);
 assert.equal(app.world.foundry,f);request.jobId='job-late-response';request.status='running';const view=await(await fetch(app.url+'/api/v1/world')).json();assert.equal(view.foundry.requests[0].jobId,'job-late-response');
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});
