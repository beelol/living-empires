/** Start a verified game/Foundry pair from this checkout using one local configuration. */
import {createHash} from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync,spawn} from 'node:child_process';
import {localConfiguration} from '../solo/config.mjs';
import {launchConfiguration,verifyPair} from '../solo/launch-config.mjs';

const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
let canonical=repo;
try{canonical=path.dirname(execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{cwd:repo,encoding:'utf8'}).trim());}catch{}
const local=localConfiguration({repo,canonical,environment:process.env});
const config=launchConfiguration({repo,canonical,environment:{...local.environment,SOLO_FOUNDRY_MODE:'gemini',THEME_INFERENCE_ENABLED:'1'}});
const {environment:env,appURL,foundryURL,runtimeDir}=config,data=env.SOLO_DATA_DIR;
config.appSourceHash=createHash('sha256').update(['server.mjs','foundry.mjs','model.mjs','store.mjs'].map(file=>fs.readFileSync(path.join(repo,'solo',file),'utf8')).join('')).digest('hex');
config.foundrySourceHash=createHash('sha256').update(JSON.stringify(['server.mjs',...fs.readdirSync(path.join(repo,'lib')).filter(name=>name.endsWith('.mjs')).sort().map(name=>'lib/'+name)].map(file=>[file,fs.readFileSync(path.join(repo,file),'utf8')]))).digest('hex');
config.sourceHash=createHash('sha256').update(['provider.rs','pipeline.rs','main.rs'].map(file=>fs.readFileSync(path.join(repo,'crates/living-world-core/src',file),'utf8')).join('')).digest('hex');
const defaultModel=env.GEMINI_MODEL||'gemini-3.8-flash';
config.models={creator:env.CREATOR_MODEL||defaultModel,repair:env.REPAIR_MODEL||defaultModel,reviewer:env.REVIEWER_MODEL||defaultModel};

async function check(url){
 try{const response=await fetch(url,{signal:AbortSignal.timeout(1500)});return response.ok?await response.json():null;}
 catch{return null;}
}

const binary=path.join(runtimeDir,'foundry-world-check');config.binaryPath=binary;
if(fs.existsSync(binary))config.binaryHash=createHash('sha256').update(fs.readFileSync(binary)).digest('hex');
let foundry=await check(foundryURL+'/api/config');
let existing=process.argv.includes('--studio')?null:await check(appURL+'/api/v1/health');
verifyPair(config,foundry,existing);
fs.mkdirSync(runtimeDir,{recursive:true,mode:0o700});

if(!foundry){
 execFileSync('cargo',['build','--manifest-path',path.join(repo,'Cargo.toml'),'-p','living-world-core','--features','live-provider'],{cwd:repo,stdio:'inherit'});
 const next=binary+'.next';
 fs.copyFileSync(path.join(repo,'target','debug','world-check'),next);
 fs.renameSync(next,binary);
 env.RUST_CORE_BIN=binary;
 config.binaryHash=createHash('sha256').update(fs.readFileSync(binary)).digest('hex');
 const descriptor=fs.openSync(path.join(runtimeDir,'foundry.log'),'a',0o600);
 const service=spawn(process.execPath,[path.join(repo,'server.mjs')],{cwd:repo,env,detached:true,stdio:['ignore',descriptor,descriptor]});
 service.unref();fs.closeSync(descriptor);
 for(let i=0;i<60&&!foundry;i++){
  await new Promise(resolve=>setTimeout(resolve,250));
  foundry=await check(foundryURL+'/api/config');
 }
 if(!foundry)throw Error('Foundry failed to start. Inspect this checkout’s .local/foundry.log.');
 verifyPair(config,foundry,null);
}else{
 if(!fs.existsSync(binary))throw Error('The matching private Foundry verifier binary is missing. Preserve the running service; repair it only when idle.');
 env.RUST_CORE_BIN=binary;
}

if(!foundry.keyConfigured)console.error('Foundry needs GEMINI_API_KEY in its server-only local environment.');
if(!foundry.spending?.sponsoredCredits&&!(foundry.spending?.remainingUsd>0))console.error('Foundry has no enabled generation credits.');
if(process.argv.includes('--studio')){
 console.log(`Live Asset Foundry: ${foundryURL}`);
 process.exit(0);
}
if(existing){
 console.log(`First Landing is running: ${appURL}\nLive Asset Foundry: ${foundryURL}`);
 process.exit(0);
}

const legacy=path.join(repo,'solo','.data');
if(!fs.existsSync(path.join(data,'world.json'))&&fs.existsSync(path.join(legacy,'world.json'))){
 if(fs.existsSync(path.join(legacy,'world.lock')))throw Error('Close the earlier colony before moving its save.');
 fs.mkdirSync(data,{recursive:true,mode:0o700});
 fs.cpSync(legacy,data,{recursive:true,errorOnExist:true,force:false});
}
const app=spawn(process.execPath,[path.join(repo,'solo','server.mjs')],{cwd:repo,env,stdio:'inherit'});
let exited=false;
app.on('exit',code=>{exited=true;process.exit(code||0);});
app.on('error',error=>{console.error(error.message);process.exit(1);});
for(const signal of ['SIGINT','SIGTERM'])process.once(signal,()=>app.kill(signal));
let ready;
for(let i=0;i<60&&!ready&&!exited;i++){
 await new Promise(resolve=>setTimeout(resolve,250));
 ready=await check(appURL+'/api/v1/health');
}
if(!ready){app.kill('SIGINT');throw Error('The game did not become ready. No colony was reset.');}
verifyPair(config,foundry,ready);
console.log(`First Landing: ${appURL}\nLive Asset Foundry: ${foundryURL}\nCredentials: ${local.localFile?'.env.local (server only)':'not configured'}\nSave: ${data}`);
