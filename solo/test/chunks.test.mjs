import test from 'node:test';
import assert from 'node:assert/strict';
import {CHUNK_SIZE,groundHeight,groundNormal,chunkData,chunkKeys,ChunkWindow,detailVisible} from '../public/chunks.mjs';
const coast=x=>-23+Math.sin(x*.065)*5+Math.sin(x*.16)*1.6;
test('shared edges agree in world space, including negative chunks and normals',()=>{
 for(const [x,z] of [[0,0],[-2,-1],[312,-7]]){const a=chunkData(x,z,coast),b=chunkData(x+1,z,coast);for(let i=0;i<=a.segments;i++){const left=a.vertices[i*(a.segments+1)+a.segments],right=b.vertices[i*(b.segments+1)];assert.deepEqual(left,right);assert.deepEqual(groundNormal(left.x,left.z,coast),groundNormal(right.x,right.z,coast));}}
});
test('details regenerate identically after visiting arbitrary other chunks',()=>{
 const a=chunkData(-9,14,coast);chunkData(1250,-300,coast);assert.deepEqual(chunkData(-9,14,coast),a);assert.notDeepEqual(chunkData(-8,14,coast).details,a.details);assert.ok(a.details.some(p=>p.kind==='rock'));assert.ok(a.details.some(p=>p.kind==='foliage'));for(const p of a.details){assert.ok(p.x>=-9*CHUNK_SIZE&&p.x<-8*CHUNK_SIZE);assert.equal(p.y,groundHeight(p.x,p.z,coast));assert.ok(p.z>coast(p.x)+2);}
});
test('height matches existing saved geography',()=>{
 for(const x of [-1000,-26,0,26,1234])for(const d of [-10,0,2,2.01,50]){const z=coast(x)+d;assert.equal(groundHeight(x,z,coast),d>2?.2+Math.sin(x*.18)*Math.sin(z*.17)*.035:Math.max(-2,d*.42-.65));}
});
test('camera rectangles include every corner across positive and negative boundaries',()=>{
 for(const b of [{minX:-.1,maxX:64.1,minZ:-64.1,maxZ:.1},{minX:10000,maxX:10200,minZ:500,maxZ:670}]){const keys=new Set(chunkKeys(b));for(let x=b.minX;x<=b.maxX;x+=.5)for(const z of [b.minZ,b.maxZ])assert.ok(keys.has(`${Math.floor(x/CHUNK_SIZE)},${Math.floor(z/CHUNK_SIZE)}`));}
});
test('streaming bounds resident chunks, retains overlap and disposes distant meshes exactly once',()=>{
 const created=[],disposed=[];const w=new ChunkWindow({create:(x,z)=>{const id=`${x},${z}`;created.push(id);return {id};},dispose:c=>disposed.push(c.id)});
 const at=x=>({minX:x-90,maxX:x+90,minZ:-90,maxZ:90});w.update(at(0));const original=[...w.chunks.keys()];w.update(at(1));assert.equal(created.length,original.length);
 for(let x=128;x<20000;x+=128){w.update(at(x));assert.ok(w.chunks.size<=25);}w.update(at(0));assert.deepEqual([...w.chunks.keys()].sort(),original.sort());assert.ok(disposed.includes('0,0'));w.clear();assert.equal(w.chunks.size,0);assert.equal(disposed.length,created.length);
});
test('decorations clear the full size of each rock around foundations and access',()=>{
 const buildings=[{x:100,z:50}];assert.equal(detailVisible({x:105,z:50,size:2},buildings),false);assert.equal(detailVisible({x:100,z:57,size:.7},buildings),false);assert.equal(detailVisible({x:120,z:70,size:1},buildings),true);assert.equal(detailVisible({x:0,z:8,size:.5},[]),false);
});
