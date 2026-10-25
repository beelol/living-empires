import * as THREE from '/vendor/three.js';
import {CHUNK_SIZE,SEGMENTS,TERRAIN_VERSION,DETAIL_SEED,chunkData,groundNormal,detailVisible,ChunkWindow} from './chunks.mjs';

/** Shared geometries/materials live for the view; only per-chunk buffers are released on travel. */
export function createTerrain(scene,coast){
 const groundMaterial=new THREE.MeshStandardMaterial({vertexColors:true,roughness:1});
 const waterMaterial=new THREE.MeshStandardMaterial({color:0x548e8d,roughness:.27,metalness:.24,transparent:true,opacity:.92});
 const rockGeometry=new THREE.DodecahedronGeometry(1,0),grassGeometry=new THREE.ConeGeometry(.22,1.1,4),waterGeometry=new THREE.PlaneGeometry(CHUNK_SIZE,CHUNK_SIZE);
 waterGeometry.rotateX(-Math.PI/2);
 const rockMaterial=new THREE.MeshStandardMaterial({color:0x949c8e,roughness:.83}),grassMaterial=new THREE.MeshStandardMaterial({color:0x6a8f72,roughness:.83}),surfMaterial=new THREE.LineBasicMaterial({color:0xc3d8c6,transparent:true,opacity:.8});
 const sand=new THREE.Color(0xc3bc9e),green=new THREE.Color(0x8eaa8c),dummy=new THREE.Object3D();let buildings=[],clearanceKey='',lastBounds='',visibleBounds=null;
 function fill(chunk){for(const [kind,instances] of [['rock',chunk.rocks],['foliage',chunk.grass]]){let count=0;for(const p of chunk.data.details){if(p.kind!==kind||!detailVisible(p,buildings))continue;const rock=kind==='rock';dummy.position.set(p.x-chunk.data.cx*CHUNK_SIZE,p.y+p.size*(rock?.15:.4),p.z-chunk.data.cz*CHUNK_SIZE);dummy.scale.set(p.size,p.size*(rock?.55:1),p.size*(rock?.85:1));dummy.rotation.set(rock?p.tilt:0,p.yaw,rock?p.tilt:.1);dummy.updateMatrix();instances.setMatrixAt(count++,dummy.matrix);}instances.count=count;instances.instanceMatrix.needsUpdate=true;instances.computeBoundingSphere();}}
 const window=new ChunkWindow({create(cx,cz){
 const data=chunkData(cx,cz,coast),group=new THREE.Group(),geometry=new THREE.BufferGeometry(),positions=[],normals=[],colors=[],indices=[];group.position.set(cx*CHUNK_SIZE,0,cz*CHUNK_SIZE);
 for(const p of data.vertices){positions.push(p.x-cx*CHUNK_SIZE,p.y,p.z-cz*CHUNK_SIZE);normals.push(...groundNormal(p.x,p.z,coast));const inland=THREE.MathUtils.smoothstep(p.z-coast(p.x),6,60),noise=(Math.sin(p.x*.4)*Math.cos(p.z*.5)+1)*.025,c=sand.clone().lerp(green,inland*.48+noise);colors.push(c.r,c.g,c.b);}
 for(let z=0;z<SEGMENTS;z++)for(let x=0;x<SEGMENTS;x++){const a=z*(SEGMENTS+1)+x,b=a+1,c=a+SEGMENTS+1,d=c+1;indices.push(a,c,b,b,c,d);}
 geometry.setAttribute('position',new THREE.Float32BufferAttribute(positions,3));geometry.setAttribute('normal',new THREE.Float32BufferAttribute(normals,3));geometry.setAttribute('color',new THREE.Float32BufferAttribute(colors,3));geometry.setIndex(indices);geometry.computeBoundingSphere();
 const ground=new THREE.Mesh(geometry,groundMaterial);ground.receiveShadow=true;group.add(ground);
 const water=new THREE.Mesh(waterGeometry,waterMaterial);water.position.set(CHUNK_SIZE/2,-.42,CHUNK_SIZE/2);water.receiveShadow=true;group.add(water);
 const rocks=new THREE.InstancedMesh(rockGeometry,rockMaterial,30),grass=new THREE.InstancedMesh(grassGeometry,grassMaterial,150);rocks.castShadow=true;rocks.receiveShadow=true;group.add(rocks,grass);
 const surfPoints=[];for(let i=0;i<=CHUNK_SIZE;i++){const x=cx*CHUNK_SIZE+i,z=coast(x)+.7;if(Math.floor(z/CHUNK_SIZE)===cz)surfPoints.push(new THREE.Vector3(i,-.2,z-cz*CHUNK_SIZE));}
 let surf;if(surfPoints.length>1){surf=new THREE.Line(new THREE.BufferGeometry().setFromPoints(surfPoints),surfMaterial);group.add(surf);}
 const chunk={group,data,geometry,rocks,grass,surf};fill(chunk);scene.add(group);return chunk;
 },dispose(chunk){chunk.group.removeFromParent();chunk.geometry.dispose();chunk.surf?.geometry.dispose();chunk.rocks.dispose();chunk.grass.dispose();}});
 const ray=new THREE.Raycaster(),plane=new THREE.Plane(new THREE.Vector3(0,1,0),2),point=new THREE.Vector3();
 return {
 update(camera,nextBuildings){const key=nextBuildings.map(b=>`${b.id}:${b.x}:${b.z}`).join('|');if(key!==clearanceKey){buildings=nextBuildings;clearanceKey=key;for(const c of window.chunks.values())fill(c);}
 camera.updateMatrixWorld();const bounds={minX:Infinity,maxX:-Infinity,minZ:Infinity,maxZ:-Infinity};for(const x of [-1,1])for(const y of [-1,1]){ray.setFromCamera(new THREE.Vector2(x,y),camera);if(ray.ray.intersectPlane(plane,point)){bounds.minX=Math.min(bounds.minX,point.x);bounds.maxX=Math.max(bounds.maxX,point.x);bounds.minZ=Math.min(bounds.minZ,point.z);bounds.maxZ=Math.max(bounds.maxZ,point.z);}}
 if(!Number.isFinite(bounds.minX))return;for(const k of ['minX','minZ'])bounds[k]-=16;for(const k of ['maxX','maxZ'])bounds[k]+=16;
 const signature=[Math.floor(bounds.minX/CHUNK_SIZE),Math.floor(bounds.maxX/CHUNK_SIZE),Math.floor(bounds.minZ/CHUNK_SIZE),Math.floor(bounds.maxZ/CHUNK_SIZE)].join(',');if(signature!==lastBounds){window.update(bounds);visibleBounds={minX:Math.floor(bounds.minX/CHUNK_SIZE)*CHUNK_SIZE,maxX:(Math.floor(bounds.maxX/CHUNK_SIZE)+1)*CHUNK_SIZE,minZ:Math.floor(bounds.minZ/CHUNK_SIZE)*CHUNK_SIZE,maxZ:(Math.floor(bounds.maxZ/CHUNK_SIZE)+1)*CHUNK_SIZE};lastBounds=signature;}
 },bounds(){return visibleBounds;},stats(){let rocks=0,foliage=0;for(const c of window.chunks.values()){rocks+=c.rocks.count;foliage+=c.grass.count;}return {version:TERRAIN_VERSION,seed:DETAIL_SEED,chunks:window.chunks.size,rocks,foliage,keys:[...window.chunks.keys()]};},
 dispose(){window.clear();for(const resource of [groundMaterial,waterMaterial,rockGeometry,grassGeometry,waterGeometry,rockMaterial,grassMaterial,surfMaterial])resource.dispose();}
 };
}
