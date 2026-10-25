import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {createApp} from '../server.mjs';

async function fixture(t){
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'colony-economy-api-'));
 const app=await createApp({port:0,dataDir:dir,foundryEnabled:false});
 t.after(async()=>{await app.close();fs.rmSync(dir,{recursive:true,force:true});});
 const post=(route,body)=>fetch(app.url+`/api/v1/${route}`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)});
 await post('command',{id:'freeze-fixture-time',type:'pause',paused:true});
 return {app,post};
}

test('blueprint preview returns canonical function, cost and power without changing colony state',async t=>{
 const {app,post}=await fixture(t);
 const before=JSON.stringify((await(await fetch(app.url+'/api/v1/world')).json()));
 const response=await post('blueprint',{prompt:'A brass windmill',power:9999,cost:0});
 assert.equal(response.status,200);
 const blueprint=await response.json();
 assert.equal(blueprint.function,'wind');
 assert.equal(blueprint.cost,35);
 assert.equal(blueprint.power,12);
 assert.equal(blueprint.demand,0);
 assert.equal(JSON.stringify(await(await fetch(app.url+'/api/v1/world')).json()),before);
});

test('blueprint preview rejects short or oversized prompts',async t=>{
 const {post}=await fixture(t);
 for(const prompt of ['wind', 'x'.repeat(2001)]){
  const response=await post('blueprint',{prompt});
  assert.equal(response.status,400);
 }
});

test('real build API binds a prompted generator and exact-once canonical receipt',async t=>{
 const {app,post}=await fixture(t);
 const input={id:'api-solar',type:'build',kind:'creation',prompt:'Copper solar panels',provider:'gemini',x:12,z:8,power:9999,cost:0};
 assert.equal((await post('command',input)).status,200);
 assert.equal((await post('command',input)).status,200);
 const state=await(await fetch(app.url+'/api/v1/world')).json();
 assert.equal(state.alloy,65);
 assert.equal(state.buildings.at(-1).function,'solar');
 assert.equal(state.power.supply,8);
 assert.equal(state.foundry.requests.length,1);
 assert.equal(state.alloyRate,1);
});

test('real build API rejects power oversubscription and missing alloy without changing state',async t=>{
 const {app,post}=await fixture(t);
 app.world.alloy=500;
 assert.equal((await post('command',{id:'home-a',type:'build',kind:'habitat',x:12,z:8})).status,200);
 assert.equal((await post('command',{id:'home-b',type:'build',kind:'habitat',x:24,z:8})).status,200);
 const beforePowerDenial=JSON.stringify(app.world);
 const denied=await post('command',{id:'no-power',type:'build',kind:'workshop',x:0,z:24});
 assert.equal(denied.status,400);
 assert.match((await denied.json()).error,/power/i);
 assert.equal(JSON.stringify(app.world),beforePowerDenial);
 app.world.alloy=0;
 const beforeAlloyDenial=JSON.stringify(app.world);
 const noFunds=await post('command',{id:'no-alloy',type:'build',kind:'creation',x:0,z:24,prompt:'Windmill generator',provider:'gemini'});
 assert.equal(noFunds.status,400);
 assert.match((await noFunds.json()).error,/alloy/i);
 assert.equal(JSON.stringify(app.world),beforeAlloyDenial);
});
