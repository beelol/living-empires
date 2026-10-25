import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {spawn} from 'node:child_process';

const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const png=Buffer.from([137,80,78,71,13,10,26,10]);
const viewNames=['front','three-quarter','rear'];
const maxLogBytes=32768,terminationGraceMs=250;
const ensure=(ok,message)=>{if(!ok)throw new Error(message);};

function boundedLog(...entries){
  const text=entries.filter(Boolean).join('\n');
  let result='',size=0;
  for(const character of text){
    const bytes=Buffer.byteLength(character);
    if(size+bytes>maxLogBytes)break;
    result+=character;size+=bytes;
  }
  return result;
}

class NativeRenderError extends Error{
  constructor(message,stderr=''){
    super(message);this.name='NativeRenderError';this.stderr=stderr;
  }
}

async function fileHash(file){
  const hash=createHash('sha256');
  for await(const chunk of fs.createReadStream(file))hash.update(chunk);
  return hash.digest('hex');
}

function render(binary,input,output,signal,timeout){
  return new Promise((resolve,reject)=>{
    if(signal?.aborted){reject(new NativeRenderError('Native capture cancelled'));return;}
    let finished=false;
    const child=spawn(binary,[input,output],{stdio:['ignore','ignore','pipe']});
    let stderr='',timer,killTimer,terminationReason=null;
    const done=(error)=>{
      if(finished)return;
      finished=true;clearTimeout(timer);clearTimeout(killTimer);signal?.removeEventListener('abort',abort);
      error?reject(new NativeRenderError(error,stderr)):resolve(stderr);
    };
    const terminate=(reason)=>{
      if(finished||terminationReason)return;
      terminationReason=reason;
      try{child.kill('SIGTERM');}catch{}
      killTimer=setTimeout(()=>{if(!finished)try{child.kill('SIGKILL');}catch{}},terminationGraceMs);
    };
    const abort=()=>terminate('Native capture cancelled');
    timer=setTimeout(()=>terminate('Native capture deadline exceeded'),timeout);
    child.stderr.on('data',chunk=>{if(Buffer.byteLength(stderr)<maxLogBytes)stderr=boundedLog(stderr+chunk.toString());});
    child.on('error',error=>done(`Native renderer could not start: ${error.message}`));
    child.on('close',code=>{
      if(terminationReason){done(terminationReason);return;}
      done(code===0?null:`Native renderer failed (${code}); evidence remains unapproved`);
    });
    signal?.addEventListener('abort',abort,{once:true});
  });
}

/** Native visual evidence plus an explicitly separate existing browser movement result. */
export async function captureNative({root,dataDir=path.join(root,'.data'),binary,job,attempt,movement,signal,timeoutMs=90000}){
  ensure(/^[a-z0-9-]{1,128}$/.test(job.id)&&Number.isInteger(attempt.number)&&attempt.number>=0&&attempt.number<=3,'Invalid native capture job');
  ensure(/^[a-f0-9]{64}$/.test(attempt.hash),'Invalid candidate fingerprint');
  ensure(binary&&fs.existsSync(binary),'Build the native foundry_audit example before native review');
  const folder=path.join(dataDir,job.id);fs.mkdirSync(folder,{recursive:true});
  const attemptFolder=fs.mkdtempSync(path.join(folder,`native-${attempt.number}-`));
  const output=path.join(attemptFolder,'views'),input=path.join(attemptFolder,'candidate-preview.json');
  const reviewHalfExtent=job.sceneHalfExtent??job.pipeline?.contract.sceneHalfExtent??4;
  ensure(Number.isFinite(reviewHalfExtent)&&reviewHalfExtent>=4&&reviewHalfExtent<=16,'Invalid frozen native review extent');
  ensure(!attempt.recipe.scene||attempt.recipe.scene.halfExtent===reviewHalfExtent,'Candidate scene differs from frozen native review extent');
  const candidate={format:'living-empires.asset',schemaVersion:1,assetType:'building',assetId:job.id,
    themeId:job.themeId,theme:job.theme,recipe:attempt.recipe,manifest:{contentSha256:attempt.hash,
      reviewHalfExtent,rehearsal:job.provider==='fixture',provider:job.provider,model:job.model,renderOnly:true}};
  fs.writeFileSync(input,JSON.stringify(candidate),{mode:0o600});
  const rendererBinarySha256=await fileHash(binary);
  let log;
  try{log=await render(binary,input,output,signal,timeoutMs);}
  catch(error){
    fs.writeFileSync(path.join(attemptFolder,'renderer.log'),boundedLog(error.message,error.stderr),{mode:0o600});
    throw error;
  }
  fs.writeFileSync(path.join(attemptFolder,'renderer.log'),boundedLog(log),{mode:0o600});
  const manifest=JSON.parse(fs.readFileSync(path.join(output,'capture-manifest.json'),'utf8'));
  ensure(manifest.package?.candidateHash===attempt.hash,'Native capture candidate differs from requested candidate');
  ensure(Array.isArray(manifest.views)&&manifest.views.length===3,'Native capture must provide three views');
  const views=viewNames.map((name,i)=>{
    const file=`view-${i+1}-${name}.png`;
    ensure(manifest.views[i].file===file,'Unexpected native view path');
    const bytes=fs.readFileSync(path.join(output,file));
    // Three base64 frames plus theme/prompt must fit world-check's 12MB input ceiling.
    ensure(bytes.length<=2_900_000&&bytes.subarray(0,8).equals(png),'Native view is missing, invalid or too large');
    return {view:i+1,sha256:sha(bytes),atUnixSeconds:Math.floor(fs.statSync(path.join(output,file)).mtimeMs/1000),
      sourceFile:path.relative(folder,path.join(output,file)),publicFile:`${attempt.number}-${i}.png`,bytes};
  });
  ensure(new Set(views.map(v=>v.sha256)).size===3,'Native views must be distinct');
  const publication=views.map(view=>{
    const target=path.join(folder,view.publicFile),staged=path.join(attemptFolder,`staged-${view.view}.png`);
    const backup=path.join(attemptFolder,`prior-${view.view-1}.png`),existed=fs.existsSync(target);
    fs.writeFileSync(staged,view.bytes,{mode:0o600});
    if(existed){
      fs.copyFileSync(target,backup);
      fs.copyFileSync(target,path.join(attemptFolder,`browser-${view.view}.png`));
    }
    return {target,staged,backup:existed?backup:null};
  });
  const nativeEvidence={renderer:'Bevy 0.20 native',candidateSha256:attempt.hash,
    rendererBinarySha256,coordinateMapping:manifest.coordinateMapping||null,
    sourceRevision:job.sourceRevision,views:views.map(({bytes,...v})=>v),
    functionalHarness:'three-browser',nativeTraversal:'not_run'};
  const stagedEvidence=path.join(attemptFolder,'evidence.json.stage'),evidenceFile=path.join(attemptFolder,'evidence.json');
  fs.writeFileSync(stagedEvidence,JSON.stringify(nativeEvidence,null,2),{mode:0o600});

  // Publish only after all images, backups and the evidence manifest are staged.
  const published=[];
  try{
    for(const item of publication){fs.renameSync(item.staged,item.target);published.push(item);}
    fs.renameSync(stagedEvidence,evidenceFile);
  }catch(error){
    const rollbackFailures=[];
    for(const item of published.reverse()){
      try{
        if(item.backup)fs.renameSync(item.backup,item.target);
        else fs.rmSync(item.target,{force:true});
      }catch(rollbackError){
        const backup=item.backup?`; backup retained at ${item.backup}`:'';
        rollbackFailures.push(`${item.target}: ${rollbackError.message}${backup}`);
      }
    }
    if(rollbackFailures.length)throw new Error(`Native capture publication failed: ${error.message}; rollback incomplete: ${rollbackFailures.join('; ')}`);
    throw error;
  }

  return {images:views.map(v=>`/evidence/${job.id}/${v.publicFile}`),movement,
    renderSource:'bevy-native',functionalHarness:'three-browser',nativeEvidence};
}
