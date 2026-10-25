import test from 'node:test';
import assert from 'node:assert/strict';
import {createWorld,command,advance,canBuild,findPath,DEFINITIONS} from '../model.mjs';
const build=(w,id,kind,x,z)=>command(w,{id,type:'build',kind,x,z});
test('construction costs once, visibly progresses and survives serialization',()=>{
 const w=createWorld();const before=w.alloy;const a=build(w,'build-a','solar',12,8);assert.equal(w.alloy,before-DEFINITIONS.solar.cost);assert.equal(w.buildings.at(-1).progress,0);
 assert.deepEqual(build(w,'build-a','solar',12,8),a);assert.equal(w.buildings.length,2);assert.throws(()=>build(w,'build-a','habitat',12,8),/reused/);
 advance(w,4);assert.ok(w.buildings.at(-1).progress>0&&w.buildings.at(-1).progress<1);const recovered=JSON.parse(JSON.stringify(w));advance(recovered,20);assert.equal(recovered.buildings.at(-1).progress,1);assert.equal(recovered.alloy,w.alloy);
});
test('overlap, water, unknown kinds and unaffordable requests do not spend',()=>{
 const w=createWorld();const before=JSON.stringify(w);assert.throws(()=>build(w,'a','solar',0,8),/overlap|access/);assert.throws(()=>build(w,'b','solar',0,-40),/water|land/);assert.throws(()=>build(w,'c','nuke',12,8),/Unknown/);assert.equal(JSON.stringify(w),before);
 w.alloy=0;assert.throws(()=>build(w,'d','solar',12,8),/alloy/);assert.equal(w.buildings.length,1);
});
test('robot routes around a structure and cannot walk through water',()=>{
 const w=createWorld();const path=findPath(w,{x:-12,z:8},{x:12,z:8});assert.ok(path.length>8);for(const p of path)assert.ok(Math.abs(p.x)>4.8||Math.abs(p.z-8)>4.8);
 command(w,{id:'move-1',type:'move',x:12,z:8});for(let i=0;i<100;i++)advance(w,.2);assert.ok(Math.hypot(w.robot.x-12,w.robot.z-8)<.2);
 assert.throws(()=>command(w,{id:'move-2',type:'move',x:0,z:-40}),/land|route/);
});
test('finite extraction needs a seam and power; outages do not invent output',()=>{
 const w=createWorld();assert.equal(canBuild(w,'extractor',12,8).ok,false);build(w,'mine','extractor',-18,8);advance(w,20);const amount=w.alloy;advance(w,5);assert.ok(w.alloy>amount);assert.ok(w.deposits[0].remaining<600);
 w.deposits[0].remaining=1;advance(w,10);assert.equal(w.deposits[0].remaining,0);const end=w.alloy;advance(w,10);assert.equal(w.alloy,end);
});
test('power shortage pauses consumers and solar restores them',()=>{
 const w=createWorld();w.alloy=1000;build(w,'a','habitat',12,8);advance(w,20);build(w,'b','habitat',24,8);advance(w,20);build(w,'c','workshop',0,24);advance(w,20);assert.equal(w.buildings.at(-1).progress,0);build(w,'s','solar',-12,24);advance(w,30);assert.equal(w.buildings.find(b=>b.kind==='workshop').progress,1);
});
test('commands validate finite coordinates, operation IDs and pause',()=>{
 const w=createWorld();for(const x of [NaN,Infinity,'12',1000001])assert.throws(()=>build(w,'x','solar',x,8));assert.throws(()=>command(w,{id:'',type:'pause'}));command(w,{id:'pause',type:'pause',paused:true});const clock=w.time;advance(w,10);assert.equal(w.time,clock);
});
test('prototype-like kinds and existing entity IDs cannot corrupt a save',()=>{
 const w=createWorld(),before=JSON.stringify(w);for(const kind of ['constructor','toString','__proto__'])assert.throws(()=>build(w,'bad-'+kind,kind,12,8),/Unknown/);assert.throws(()=>build(w,'landing-core','solar',12,8),/identity|exists/);assert.equal(JSON.stringify(w),before);
});
test('operation retries tolerate JSON field order and reserve dangerous IDs',()=>{
 const w=createWorld();const first=command(w,{id:'same',type:'move',x:12,z:8});assert.deepEqual(command(w,{z:8,x:12,type:'move',id:'same'}),first);for(const id of ['__proto__','constructor'])assert.throws(()=>command(w,{id,type:'move',x:12,z:8}));
});
test('prompt creations preserve the free-form brief and queue one job for one paid plot',()=>{
 const w=createWorld(),input={id:'custom-1',type:'build',kind:'creation',x:12,z:8,prompt:'An enormous pink mushroom with a glass observatory',provider:'fixture'};command(w,input);assert.equal(w.alloy,175);assert.equal(w.buildings.at(-1).prompt,input.prompt);assert.equal(w.foundry.requests.length,1);assert.match(w.foundry.requests[0].prompt,/pink mushroom/);assert.equal(w.foundry.requests[0].targetId,'custom-1');command(w,input);assert.equal(w.foundry.requests.length,1);assert.throws(()=>command(w,{...input,id:'bad',x:24,prompt:''}),/Describe/);
});
test('dry land beyond the former survey edge is buildable and mineral prospecting is paid once',()=>{
 const w=createWorld();assert.equal(canBuild(w,'solar',64,8).ok,true);const result=command(w,{id:'prospect',type:'prospect'});assert.ok(result.buildingId);assert.equal(w.alloy,140);assert.equal(w.buildings.at(-1).kind,'extractor');assert.equal(w.buildings.at(-1).progress,0);command(w,{id:'prospect',type:'prospect'});assert.equal(w.alloy,140);
});
