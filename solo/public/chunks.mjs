/** Visual terrain v1: absolute coordinates, no mutable world or resource state. */
export const CHUNK_SIZE=64,SEGMENTS=32,DETAIL_SEED=1337,TERRAIN_VERSION=1;
export function groundHeight(x,z,coast){const d=z-coast(x);return d>2?.2+Math.sin(x*.18)*Math.sin(z*.17)*.035:Math.max(-2,d*.42-.65);}
export function groundNormal(x,z,coast){const e=.25,dx=(groundHeight(x+e,z,coast)-groundHeight(x-e,z,coast))/(2*e),dz=(groundHeight(x,z+e,coast)-groundHeight(x,z-e,coast))/(2*e),length=Math.hypot(dx,1,dz);return [-dx/length,1/length,-dz/length];}
function randomFor(x,z){let seed=(Math.imul(x,73856093)^Math.imul(z,19349663)^DETAIL_SEED)>>>0;return ()=>{seed=(seed+0x6d2b79f5)>>>0;let n=seed;n=Math.imul(n^(n>>>15),n|1);n^=n+Math.imul(n^(n>>>7),n|61);return ((n^(n>>>14))>>>0)/4294967296;};}
export function chunkData(cx,cz,coast){
 const vertices=[],details=[],step=CHUNK_SIZE/SEGMENTS;
 for(let iz=0;iz<=SEGMENTS;iz++)for(let ix=0;ix<=SEGMENTS;ix++){const x=cx*CHUNK_SIZE+ix*step,z=cz*CHUNK_SIZE+iz*step;vertices.push({x,z,y:groundHeight(x,z,coast)});}
 const rand=randomFor(cx,cz);
 for(let i=0;i<180;i++){const x=(cx+rand())*CHUNK_SIZE,z=(cz+rand())*CHUNK_SIZE,kind=i<30?'rock':'foliage',size=kind==='rock'?.4+rand()*1.7:.45+rand()*.9,yaw=rand()*Math.PI*2,tilt=rand()*.3;
 if(z<coast(x)+(kind==='rock'?2:9))continue;details.push({kind,x,z,y:groundHeight(x,z,coast),size,yaw,tilt});}
 return {cx,cz,segments:SEGMENTS,vertices,details};
}
export function detailVisible(p,buildings){
 // Keep the original landing clearing; extra footprint includes each prop's radius.
 if(Math.abs(p.x)<42+p.size&&p.z>-18-p.size&&p.z<44+p.size)return false;
 return !buildings.some(b=>Math.abs(p.x-b.x)<5+p.size&&p.z>b.z-5-p.size&&p.z<b.z+8+p.size);
}
export function chunkKeys({minX,maxX,minZ,maxZ}){const keys=[];for(let z=Math.floor(minZ/CHUNK_SIZE);z<=Math.floor(maxZ/CHUNK_SIZE);z++)for(let x=Math.floor(minX/CHUNK_SIZE);x<=Math.floor(maxX/CHUNK_SIZE);x++)keys.push(`${x},${z}`);return keys;}
export class ChunkWindow{
 constructor({create,dispose}){this.create=create;this.dispose=dispose;this.chunks=new Map();}
 update(bounds){const wanted=new Set(chunkKeys(bounds));for(const [key,chunk] of this.chunks)if(!wanted.has(key)){this.dispose(chunk);this.chunks.delete(key);}for(const key of wanted)if(!this.chunks.has(key)){const [x,z]=key.split(',').map(Number);this.chunks.set(key,this.create(x,z));}}
 clear(){for(const chunk of this.chunks.values())this.dispose(chunk);this.chunks.clear();}
}
