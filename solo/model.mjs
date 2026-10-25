/** Single-player rules v1. Pure JSON state; UI and Foundry cannot mint world effects. */
export const DEFINITIONS=Object.freeze({
 core:{name:'Landing core',cost:0,seconds:0,power:8,demand:0,description:'Your colony anchor. Supplies 8 power.'},
 creation:{name:'Prompt creation',cost:25,seconds:6,power:0,demand:0,description:'A fixed-size foundation for anything you describe. 25 alloy.'},
 solar:{name:'Solar array',cost:35,seconds:8,power:12,demand:0,description:'Provides 12 power, even during a shortage.'},
 extractor:{name:'Mineral extractor',cost:60,seconds:12,power:0,demand:5,description:'On a mineral seam: extracts 2 alloy / second. Uses 5 power.'},
 habitat:{name:'Habitat',cost:40,seconds:10,power:0,demand:3,description:'Makes room for 8 settlers. Uses 3 power.'},
 workshop:{name:'Fabrication lab',cost:70,seconds:14,power:0,demand:4,description:'Recycles 0.5 alloy / second. Uses 4 power.'},
});
export const GRID=2,HALF=4.3,ROBOT_RADIUS=.55;
export const snap=n=>Math.round(n/GRID)*GRID;
export const coast=x=>-23+Math.sin(x*.065)*5+Math.sin(x*.16)*1.6;
export function land(x,z){return Number.isFinite(x)&&Number.isFinite(z)&&Math.abs(x)<=1e6&&Math.abs(z)<=1e6&&z>=coast(x)+2;}
export function creationRequest(input,world){
 if(typeof input.prompt!=='string'||input.prompt.trim().length<5||input.prompt.length>2000)throw Error('Describe the creation in 5–2000 characters.');
 if(!['fixture','gemini','managed'].includes(input.provider))throw Error('Choose a Foundry provider.');
 return {id:'creation-'+input.id,kind:'creation',targetId:input.id,round:world.foundry.round,status:'queued',provider:input.provider,prompt:`[colony:${world.id};creation:${input.id}] ${input.prompt.trim()}\nCreate the requested subject inside a fixed 8 metre-wide plot. Preserve a traversable front-to-centre corridor and grounded supports required by the Foundry contract. Express the description visually; do not substitute a standard relay unless requested. scene.components MUST be an array of objects with id, name, role, partIds. Example: {"id":"main-body","name":"Main body","role":"structure","partIds":["part-one"]}. IDs use lowercase letters and hyphens only; every part id belongs to exactly one component. Elevated fins and ornaments must use role detail, not grounded support.`,userPrompt:input.prompt.trim()};
}
export function createWorld(){return {schemaVersion:1,id:'colony-'+globalThis.crypto.randomUUID(),revision:0,time:0,paused:false,alloy:200,robot:{x:0,z:18,path:[]},buildings:[{id:'landing-core',kind:'core',x:0,z:8,progress:1,created:0}],deposits:[{id:'west-seam',x:-18,z:8,remaining:600},{id:'east-seam',x:26,z:-4,remaining:900},{id:'north-seam',x:-26,z:32,remaining:1200}],assets:{},assetHistory:[],receipts:{},events:[{time:0,text:'Touchdown. Build a solar array, then extract alloy from a mineral seam.'}],foundry:{enabled:false,mode:'fixture',round:1,requests:[],error:null}};}
function event(w,text){w.events.push({time:w.time,text});if(w.events.length>80)w.events.shift();}
export function power(w){return w.buildings.reduce((a,b)=>{const d=DEFINITIONS[b.kind];if(b.progress===1)a.supply+=d.power;a.demand+=d.demand;return a;},{supply:0,demand:0});}
export function canBuild(w,kind,x,z){
 const d=typeof kind==='string'&&Object.hasOwn(DEFINITIONS,kind)?DEFINITIONS[kind]:null;if(!d||kind==='core')return {ok:false,reason:'Unknown building type.'};
 if(typeof x!=='number'||typeof z!=='number'||!Number.isFinite(x)||!Number.isFinite(z))return {ok:false,reason:'Choose finite map coordinates.'};
 x=snap(x);z=snap(z);
 if(![[-HALF,-HALF],[HALF,-HALF],[-HALF,HALF+2],[HALF,HALF+2]].every(([a,b])=>land(x+a,z+b)))return {ok:false,reason:'Choose dry land; this footprint crosses the coast.'};
 if(w.buildings.length>=100)return {ok:false,reason:'This colony supports 100 buildings.'};
 if(w.buildings.some(b=>Math.abs(x-b.x)<HALF*2+1.4&&Math.abs(z-b.z)<HALF*2+2.2))return {ok:false,reason:'Building or access overlap. Leave room between foundations.'};
 if(Math.abs(x-w.robot.x)<HALF+ROBOT_RADIUS&&Math.abs(z-w.robot.z)<HALF+ROBOT_RADIUS)return {ok:false,reason:'Move your robot off the construction footprint.'};
 if(!findPath(w,w.robot,{x,z:z+HALF+2},{extra:{x,z}}))return {ok:false,reason:'No accessible route to this site.'};
 const deposit=w.deposits.find(p=>Math.hypot(p.x-x,p.z-z)<=4&&p.remaining>0);
 if(kind==='extractor'&&!deposit)return {ok:false,reason:'Place the extractor directly on a mineral seam.'};
 if(w.alloy<d.cost)return {ok:false,reason:`Need ${d.cost} alloy; you have ${Math.floor(w.alloy)}.`};
 return {ok:true,x,z,depositId:deposit?.id};
}
export function blocked(w,x,z,extra){return !land(x,z)||[...w.buildings,...(extra?[extra]:[])].some(b=>Math.abs(x-b.x)<HALF+ROBOT_RADIUS&&Math.abs(z-b.z)<HALF+ROBOT_RADIUS);}
export function findPath(w,from,to,{extra}={}){
 if(!land(to.x,to.z)||blocked(w,to.x,to.z,extra))return null;
 const start={x:snap(from.x),z:snap(from.z)},end={x:snap(to.x),z:snap(to.z)};if(blocked(w,end.x,end.z,extra))return null;
 const key=p=>`${p.x},${p.z}`,heuristic=p=>Math.abs(p.x-end.x)+Math.abs(p.z-end.z),open=[{...start,score:heuristic(start)}],came=new Map([[key(start),null]]),cost=new Map([[key(start),0]]);let steps=0;
 while(open.length&&steps++<10000){open.sort((a,b)=>b.score-a.score);const p=open.pop();if(p.x===end.x&&p.z===end.z){const route=[];let at=p;while(at){route.unshift({x:at.x,z:at.z});at=came.get(key(at));}route.shift();return route;}
 for(const [dx,dz] of [[2,0],[-2,0],[0,2],[0,-2]]){const next={x:p.x+dx,z:p.z+dz},nextCost=cost.get(key(p))+2;if(blocked(w,next.x,next.z,extra)||nextCost>=(cost.get(key(next))??Infinity))continue;cost.set(key(next),nextCost);came.set(key(next),p);open.push({...next,score:nextCost+heuristic(next)});}
 }return null;
}
export function mineralCandidates(w){const candidates=[...w.deposits],tileX=Math.floor(w.robot.x/32),tileZ=Math.floor(w.robot.z/32);for(let dx=-3;dx<=3;dx++)for(let dz=-3;dz<=3;dz++){const x=tileX+dx,z=tileZ+dz,hash=(Math.imul(x+9821,73856093)^Math.imul(z+127,19349663))>>>0,id=`seam-${x}-${z}`;if(w.deposits.some(d=>d.id===id))continue;const deposit={id,x:x*32+((hash%9)-4)*2,z:z*32+(((hash>>>7)%9)-4)*2,remaining:600+hash%900};if(land(deposit.x,deposit.z))candidates.push(deposit);}return candidates.sort((a,b)=>Math.hypot(a.x-w.robot.x,a.z-w.robot.z)-Math.hypot(b.x-w.robot.x,b.z-w.robot.z));}

export function command(w,input){
 if(!input||typeof input!=='object'||typeof input.id!=='string'||!/^[a-zA-Z0-9_-]{1,100}$/.test(input.id)||['__proto__','constructor','prototype'].includes(input.id))throw Error('A stable operation ID is required.');
 const signature=JSON.stringify(Object.fromEntries(Object.keys(input).sort().map(k=>[k,input[k]])));if(Object.hasOwn(w.receipts,input.id)){if(w.receipts[input.id].signature!==signature)throw Error('Operation ID reused with different data.');return structuredClone(w.receipts[input.id].result);}
 if(Object.keys(w.receipts).length>=4096)throw Error('Command history is full; export this colony before starting a new one.');
 let result={ok:true,id:input.id};
 const creation=input.type==='build'&&input.kind==='creation'?creationRequest(input,w):null;
 if(input.type==='prospect'){
 if(w.alloy<DEFINITIONS.extractor.cost)throw Error('Need 60 alloy to place a mineral extractor.');
 const candidate=structuredClone(w),deposit=mineralCandidates(w).find(d=>d.remaining>0&&canBuild({...w,deposits:[...w.deposits,d]},'extractor',d.x,d.z).ok);if(!deposit)throw Error('No accessible mineral seam nearby. Move your robot to another area.');
 if(!candidate.deposits.some(d=>d.id===deposit.id))candidate.deposits.push(deposit);result=command(candidate,{id:input.id,type:'build',kind:'extractor',x:deposit.x,z:deposit.z});Object.assign(w,candidate);
 }else if(input.type==='build'){
 if(w.buildings.some(b=>b.id===input.id))throw Error('Building identity already exists.');
 const site=canBuild(w,input.kind,input.x,input.z);if(!site.ok)throw Error(site.reason);
 w.alloy-=DEFINITIONS[input.kind].cost;const b={id:input.id,kind:input.kind,x:site.x,z:site.z,progress:0,created:w.time,...(input.kind==='extractor'?{depositId:site.depositId}:{})};if(creation){b.prompt=creation.userPrompt;b.name=creation.userPrompt.slice(0,70);b.creationStatus='queued';w.foundry.requests.push(creation);}w.buildings.push(b);result.buildingId=b.id;event(w,`${DEFINITIONS[b.kind].name} construction started.`);
 // Recalculate the remaining route when a new foundation crosses it.
 if(w.robot.path.length){const destination=w.robot.path.at(-1);w.robot.path=findPath(w,w.robot,destination)||[];}
 }else if(input.type==='move'){
 if(typeof input.x!=='number'||typeof input.z!=='number'||!Number.isFinite(input.x)||!Number.isFinite(input.z))throw Error('Choose finite map coordinates.');
 const path=findPath(w,w.robot,{x:input.x,z:input.z});if(!path)throw Error('No dry-land route to that location.');w.robot.path=path;
 }else if(input.type==='pause'){if(typeof input.paused!=='boolean')throw Error('Pause must be true or false.');w.paused=input.paused;
 }else throw Error('Unknown colony command.');
 w.revision++;w.receipts[input.id]={signature,result};return structuredClone(result);
}
export function advance(w,seconds){
 if(w.paused||!Number.isFinite(seconds)||seconds<=0)return;
 let remaining=Math.min(seconds,60);while(remaining>0){const dt=Math.min(remaining,.25);remaining-=dt;w.time+=dt;const p=power(w),powered=p.supply>=p.demand;
 for(const b of w.buildings){const def=DEFINITIONS[b.kind];if(b.progress<1){if(powered||def.demand===0){b.progress=Math.min(1,b.progress+dt/def.seconds);if(b.progress>=1-1e-9){b.progress=1;event(w,`${def.name} is online.`);}}continue;}
 if(powered&&b.kind==='extractor'){const deposit=w.deposits.find(d=>d.id===b.depositId);if(deposit){const amount=Math.min(deposit.remaining,dt*2);deposit.remaining-=amount;w.alloy+=amount;}}
 if(powered&&b.kind==='workshop')w.alloy+=dt*.5;
 }
 const target=w.robot.path[0];if(target){const dx=target.x-w.robot.x,dz=target.z-w.robot.z,d=Math.hypot(dx,dz),s=Math.min(d,dt*6);const x=w.robot.x+(d?dx/d*s:0),z=w.robot.z+(d?dz/d*s:0);if(blocked(w,x,z)){w.robot.path=[];event(w,'Robot route blocked. Choose another destination.');}else{w.robot.x=x;w.robot.z=z;if(d<=s+.001)w.robot.path.shift();}}
 }w.revision++;
}
export function publicState(w){const p=power(w);return {schemaVersion:1,id:w.id,revision:w.revision,time:w.time,paused:w.paused,alloy:w.alloy,robot:w.robot,buildings:w.buildings,deposits:w.deposits,assets:w.assets,assetHistory:w.assetHistory.map(({kind,hash,rehearsal,at})=>({kind,hash,rehearsal,at})),power:p,population:w.buildings.filter(b=>b.kind==='habitat'&&b.progress===1).length*8,populationCap:1+w.buildings.filter(b=>b.kind==='habitat'&&b.progress===1).length*8,events:w.events,foundry:w.foundry,definitions:DEFINITIONS};}
export function validateWorld(w){
 if(w?.schemaVersion!==1||(typeof w.id!=='string'||!(/^(?:first-landing|colony-[a-f0-9-]{36})$/).test(w.id))||!Number.isFinite(w.alloy)||w.alloy<0||!Number.isFinite(w.time)||!Array.isArray(w.buildings)||w.buildings.length<1||w.buildings.length>100||!w.robot||!Array.isArray(w.robot.path)||!w.receipts||!w.foundry)throw Error('Unsupported or damaged colony save; preserve the file for recovery.');
 const ids=new Set();for(const b of w.buildings){if(ids.has(b.id)||!Object.hasOwn(DEFINITIONS,b.kind)||!Number.isFinite(b.x)||!Number.isFinite(b.z)||!Number.isFinite(b.progress)||b.progress<0||b.progress>1)throw Error('Damaged building record.');ids.add(b.id);}return w;
}
