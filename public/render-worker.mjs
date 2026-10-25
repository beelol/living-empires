import {createWorld} from './scene.mjs';

const taskUrl='/api/render-task';
const frame=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));

export function startRenderWorker({interval=700}={}){
  let stopped=false,inFlight=false;
  const submitted=new Set();
  async function post(payload){
    const response=await fetch(taskUrl,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)});
    if(!response.ok)throw new Error(`Render result rejected (${response.status})`);
    return response;
  }
  async function processTask(task){
    const {token,jobId,attemptNumber,candidateHash,recipe,theme}=task;
    const key=String(token);
    if(submitted.has(key))return;
    let host,world;
    try{
      host=document.createElement('div');
      Object.assign(host.style,{position:'fixed',left:'-20000px',top:'0',width:'1000px',height:'740px',overflow:'hidden',pointerEvents:'none'});
      document.body.append(host);
      world=createWorld(host,{audit:true});
      world.load(recipe,theme);
      await frame();
      const images=[];
      for(let view=0;view<3;view++){
        world.view(view);
        await frame();
        images.push(host.querySelector('canvas')?.toDataURL('image/png'));
      }
      if(images.some(image=>typeof image!=='string'||!image.startsWith('data:image/png;base64,')))throw new Error('Could not capture all three PNG views');
      const movement=world.traverse();
      submitted.add(key);
      await post({token,jobId,attemptNumber,candidateHash,images,movement});
    }catch(error){
      const message=String(error?.message||'Browser renderer failed').replace(/[\r\n\t]+/g,' ').slice(0,400);
      submitted.add(key);
      try{await post({token,jobId,attemptNumber,candidateHash,error:message});}catch{}
    }finally{
      try{world?.dispose();}catch{}
      host?.remove();
    }
  }
  async function poll(){
    if(stopped||inFlight)return;
    inFlight=true;
    try{
      const response=await fetch(taskUrl,{cache:'no-store'});
      if(!response.ok)return;
      const task=await response.json();
      if(task&&task.token&&!submitted.has(String(task.token)))await processTask(task);
    }catch{}
    finally{inFlight=false;}
  }
  const timer=setInterval(poll,interval);
  void poll();
  return ()=>{stopped=true;clearInterval(timer);};
}
