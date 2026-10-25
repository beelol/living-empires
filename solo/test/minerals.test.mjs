import test from 'node:test';
import assert from 'node:assert/strict';
import {proceduralDeposit,depositsInBounds,depositNear} from '../minerals.mjs';
import {createWorld,land,canBuild,command,advance} from '../model.mjs';

test('procedural seams preserve the original prospecting contract everywhere',()=>{
 for(const [x,z] of [[0,0],[20,10],[-20,11],[200000,700000]]){const hash=(Math.imul(x+9821,73856093)^Math.imul(z+127,19349663))>>>0;assert.deepEqual(proceduralDeposit(x,z),{id:`seam-${x}-${z}`,x:x*32+((hash%9)-4)*2,z:z*32+(((hash>>>7)%9)-4)*2,remaining:600+hash%900});}
});
test('camera mineral discovery is deterministic, bounded and read-only across far positive and negative terrain',()=>{
 const w=createWorld(),before=JSON.stringify(w);for(const x of [-20000,0,20000]){const bounds={minX:x-100,maxX:x+100,minZ:900,maxZ:1100},a=depositsInBounds(w.deposits,bounds,land);assert.ok(a.length>=25&&a.length<=64);assert.deepEqual(a,depositsInBounds(w.deposits,bounds,land));for(const d of a){assert.ok(d.x>=bounds.minX&&d.x<=bounds.maxX);assert.ok(d.z>=bounds.minZ&&d.z<=bounds.maxZ);assert.ok(land(d.x,d.z));}}assert.equal(JSON.stringify(w),before);
});
test('saved finite amounts and exhausted seams override generation after reload',()=>{
 const d=proceduralDeposit(-20,11),saved=JSON.parse(JSON.stringify([{...d,remaining:0}])),bounds={minX:d.x-3,maxX:d.x+3,minZ:d.z-3,maxZ:d.z+3};assert.equal(depositsInBounds(saved,bounds,land).find(p=>p.id===d.id).remaining,0);assert.equal(depositNear(saved,d.x,d.z,land),null);assert.equal(saved[0].remaining,0);
});
test('seams jittering across tile edges remain discoverable and oceans contain none',()=>{
 for(let x=-20;x<20;x++){const d=proceduralDeposit(x,10);assert.equal(depositNear([],d.x,d.z,land).id,d.id);}assert.deepEqual(depositsInBounds([],{minX:-100,maxX:100,minZ:-1000,maxZ:-800},land),[]);
});
test('a visible distant procedural seam can be paid for once, mined, and recovered without refill',()=>{
 const w=createWorld(),d=proceduralDeposit(20,10);w.robot={x:d.x,z:d.z+12,path:[]};const site=canBuild(w,'extractor',d.x,d.z);assert.equal(site.ok,true,site.reason);assert.equal(w.deposits.length,3);const before=w.alloy,input={id:'new-seam',type:'build',kind:'extractor',x:d.x,z:d.z};command(w,input);command(w,input);assert.equal(w.alloy,before-60);assert.equal(w.deposits.filter(p=>p.id===d.id).length,1);advance(w,20);const seam=w.deposits.find(p=>p.id===d.id);assert.ok(seam.remaining<d.remaining);const restored=JSON.parse(JSON.stringify(w));assert.equal(depositNear(restored.deposits,d.x,d.z,land).remaining,seam.remaining);restored.deposits.find(p=>p.id===d.id).remaining=0;advance(restored,10);assert.equal(restored.deposits.find(p=>p.id===d.id).remaining,0);
});
test('rejected distant extraction persists no deposit or cost',()=>{
 const w=createWorld(),d=proceduralDeposit(-20,11);w.robot={x:d.x,z:d.z+12,path:[]};w.alloy=0;const before=JSON.stringify(w);assert.throws(()=>command(w,{id:'poor',type:'build',kind:'extractor',x:d.x,z:d.z}),/alloy/);assert.equal(JSON.stringify(w),before);
});
