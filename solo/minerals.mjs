/** Mineral geography v1. Keep IDs/hash stable: existing prospecting saves depend on these bytes. */
export const MINERAL_TILE_SIZE=32;
export function proceduralDeposit(tileX,tileZ){
 const hash=(Math.imul(tileX+9821,73856093)^Math.imul(tileZ+127,19349663))>>>0;
 return {id:`seam-${tileX}-${tileZ}`,x:tileX*32+((hash%9)-4)*2,z:tileZ*32+(((hash>>>7)%9)-4)*2,remaining:600+hash%900};
}
/** Camera discovery is read-only. A persisted zero overrides the original finite quantity. */
export function depositsInBounds(saved,bounds,isLand){
 const inBounds=p=>p.x>=bounds.minX&&p.x<=bounds.maxX&&p.z>=bounds.minZ&&p.z<=bounds.maxZ;
 const overrides=new Map(saved.map(p=>[p.id,p])),result=new Map();
 for(const p of saved)if(inBounds(p)&&isLand(p.x,p.z))result.set(p.id,p);
 // Positions jitter up to 8 m either side of their tile origin, including across tile edges.
 for(let z=Math.ceil((bounds.minZ-8)/32);z<=Math.floor((bounds.maxZ+8)/32);z++)for(let x=Math.ceil((bounds.minX-8)/32);x<=Math.floor((bounds.maxX+8)/32);x++){
 const generated=proceduralDeposit(x,z),p=overrides.get(generated.id)||generated;
 if(inBounds(p)&&isLand(p.x,p.z))result.set(p.id,p);
 }
 return [...result.values()];
}
export function depositNear(saved,x,z,isLand){return depositsInBounds(saved,{minX:x-4,maxX:x+4,minZ:z-4,maxZ:z+4},isLand).find(p=>p.remaining>0&&Math.hypot(p.x-x,p.z-z)<=4)||null;}
