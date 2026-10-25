import fs from 'node:fs';
import path from 'node:path';
import {createWorld,validateWorld} from './model.mjs';
export function openStore(dir){
 fs.mkdirSync(dir,{recursive:true,mode:0o700});const file=path.join(dir,'world.json'),lock=path.join(dir,'world.lock');
 if(fs.existsSync(lock)){const pid=Number(fs.readFileSync(lock,'utf8'));if(!Number.isSafeInteger(pid)||pid<1)throw Error('Colony save lock is invalid; inspect it before recovery.');let alive=true;try{process.kill(pid,0);}catch(e){if(e.code==='ESRCH')alive=false;}if(alive)throw Error('This colony is already open in another process.');fs.unlinkSync(lock);}
 const fd=fs.openSync(lock,'wx',0o600);fs.writeFileSync(fd,String(process.pid));fs.fsyncSync(fd);fs.closeSync(fd);
 function save(world){const temporary=file+'.tmp';const fd=fs.openSync(temporary,'w',0o600);try{fs.writeFileSync(fd,JSON.stringify(world));fs.fsyncSync(fd);}finally{fs.closeSync(fd);}fs.renameSync(temporary,file);const folder=fs.openSync(dir,'r');try{fs.fsyncSync(folder);}finally{fs.closeSync(folder);}}
 try{const world=fs.existsSync(file)?validateWorld(JSON.parse(fs.readFileSync(file,'utf8'))):createWorld();save(world);let closed=false;return {world,save:()=>save(world),close(){if(closed)return;save(world);fs.unlinkSync(lock);closed=true;}};}catch(e){fs.unlinkSync(lock);throw e;}
}
