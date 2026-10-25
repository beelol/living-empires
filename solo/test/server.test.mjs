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
test('faction commands persist scoped assignments and supersede in-flight references without enabling automatic work',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-factions-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});
 const post=body=>fetch(app.url+'/api/v1/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
 for(const input of [{id:'a',type:'build',kind:'solar',x:12,z:8},{id:'b',type:'build',kind:'solar',x:-12,z:8},{id:'create-east',type:'faction-create',factionId:'east',name:'East',artDirection:'Black basalt stepped architecture with copper ribs.'}])assert.equal((await post(input)).status,200);
 const assign={id:'assign-east',type:'faction-assign',factionId:'east',buildingIds:['a']};assert.equal((await post(assign)).status,200);assert.equal((await post(assign)).status,200);
 const f=app.world.foundry,prior=f.requests.find(r=>r.factionId==='east');prior.status='dispatching';
 assert.equal((await post({id:'style-east',type:'faction-style',factionId:'east',artDirection:'Black basalt stepped architecture with carved copper ribs and deep roof eaves.'})).status,200);
 assert.equal(app.world.foundry,f);assert.equal(prior.status,'cancelled');assert.equal(f.requests.find(r=>r.id===prior.id),prior);assert.equal(f.enabled,false);
 let state=await(await fetch(app.url+'/api/v1/world')).json();assert.equal(state.buildings.find(b=>b.id==='a').factionId,'east');assert.equal(state.buildings.find(b=>b.id==='b').factionId,'landing');assert.equal(state.factions.find(f=>f.id==='east').revision,2);assert.ok(state.foundry.requests.filter(r=>r.status==='queued').every(r=>r.targets.every(t=>t.id==='a')));assert.ok(state.foundryURL);
 const health=await(await fetch(app.url+'/api/v1/health')).json();assert.equal(health.apiVersion,1);assert.equal(health.saveVersion,2);assert.equal((await fetch(app.url+'/public/themes.mjs')).status,200);
 await app.close();app=await createApp({port:0,dataDir:dir,foundryEnabled:false});state=await(await fetch(app.url+'/api/v1/world')).json();assert.equal(state.buildings.find(b=>b.id==='a').factionId,'east');assert.equal(state.foundry.requests.find(r=>r.id===prior.id).status,'cancelled');assert.equal(state.factions.find(f=>f.id==='east').revision,2);
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});

test('restart retains uncertain theme work as blocked instead of silently retrying or hanging',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-theme-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});app.world.foundry.requests.push({id:'theme-unknown',kind:'core',status:'preparing-theme',factionId:'landing',factionRevision:1,targets:[{id:'landing-core',assignment:1}]});await app.close();app=await createApp({port:0,dataDir:dir,foundryEnabled:false});
 const request=app.world.foundry.requests[0];assert.equal(request.status,'blocked');assert.equal(request.reconciliationRequired,true);assert.match(request.reason,/no automatic retry/);
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});

test('saved faction areas govern later construction and survive API restart without cancelling current work',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-area-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});
 const post=body=>fetch(app.url+'/api/v1/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
 for(const input of [{id:'create-east',type:'faction-create',factionId:'east',name:'East',artDirection:'Basalt blocks with copper ribs and cyan recesses.'},{id:'save-east-area',type:'faction-assign',factionId:'east',buildingIds:[],area:{minX:8,maxX:30,minZ:0,maxZ:18},reuseDesign:true}]){const r=await post(input);assert.equal(r.status,200,await r.text());}
 const first={id:'new-east-a',type:'build',kind:'solar',factionId:'landing',x:12,z:8};assert.equal((await post(first)).status,200);assert.equal((await post(first)).status,200);
 const pending=app.world.foundry.requests.find(r=>r.targets?.some(t=>t.id==='new-east-a'));pending.status='running';pending.jobId='existing-bounded-job';
 assert.equal((await post({id:'new-east-b',type:'build',kind:'solar',factionId:'landing',x:24,z:8})).status,200);assert.equal(pending.status,'running');assert.equal(app.world.foundry.requests.find(r=>r.id===pending.id),pending);
 assert.equal(app.world.alloy,130);assert.equal(app.world.buildings.find(b=>b.id==='new-east-b').factionId,'east');assert.equal(app.world.foundry.requests.filter(r=>r.preferExistingDesign).length,2);assert.equal(app.world.foundry.enabled,false);
 const originalArea=structuredClone(app.world.factions.find(f=>f.id==='east').area);await app.close();app=await createApp({port:0,dataDir:dir,foundryEnabled:false});assert.deepEqual(app.world.factions.find(f=>f.id==='east').area,originalArea);assert.equal(app.world.buildings.find(b=>b.id==='new-east-b').factionId,'east');assert.equal(app.world.foundry.requests.find(r=>r.id===pending.id).jobId,'existing-bounded-job');
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});
test('cancelling an inherited installation never cancels its shared source job',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-reuse-'));let app;try{
 app=await createApp({port:0,dataDir:dir,foundryEnabled:false});const source={id:'source',kind:'core',status:'installed',jobId:'approved-source'};app.world.foundry.requests.push(source,{id:'inheritor',kind:'core',status:'running',jobId:'approved-source',reuseFromRequestId:'source'});
 const r=await fetch(app.url+'/api/v1/foundry',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'cancel',requestId:'inheritor'})});assert.equal(r.status,200);assert.equal(source.status,'installed');assert.equal(app.world.foundry.error,null);assert.equal(app.world.foundry.requests[1].status,'cancelled');
 }finally{await app?.close();fs.rmSync(dir,{recursive:true,force:true});}
});
