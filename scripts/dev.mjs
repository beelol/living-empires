/** Launch the default single-player game and its existing live Foundry service. */
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync,spawn} from 'node:child_process';
import {localConfiguration} from '../solo/config.mjs';
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
let canonical=repo;try{canonical=path.dirname(execFileSync('git',['rev-parse','--path-format=absolute','--git-common-dir'],{cwd:repo,encoding:'utf8'}).trim());}catch{}
const config=localConfiguration({repo,canonical,environment:process.env}),envFile=config.localFile;
const env={...config.environment,SOLO_FOUNDRY_MODE:'gemini',THEME_INFERENCE_ENABLED:'1',GOOGLE_SPONSORED_CREDITS:config.environment.GOOGLE_SPONSORED_CREDITS||'1'};
const data=process.env.SOLO_DATA_DIR||path.join(canonical,'.local','solo-colony');env.SOLO_DATA_DIR=data;
const appURL=`http://127.0.0.1:${env.SOLO_PORT||4320}`,foundryURL=env.SOLO_FOUNDRY_URL||'http://127.0.0.1:4317';env.SOLO_FOUNDRY_URL=foundryURL;
async function check(url){try{const res=await fetch(url,{signal:AbortSignal.timeout(1500)});return res.ok?await res.json():null;}catch{return null;}}
let foundry=await check(foundryURL+'/api/config');
if(!foundry){
 if(!['http://127.0.0.1:4317','http://127.0.0.1:4317/'].includes(foundryURL))throw Error('Configured Foundry is unavailable. Start that service, then run dev again.');
 fs.mkdirSync(path.join(canonical,'.local'),{recursive:true});const fd=fs.openSync(path.join(canonical,'.local','solo-foundry.log'),'a',0o600);
 const child=spawn(process.execPath,[...(envFile?[`--env-file=${envFile}`]:[]),path.join(canonical,'server.mjs')],{cwd:canonical,env,detached:true,stdio:['ignore',fd,fd]});child.unref();fs.closeSync(fd);
 for(let i=0;i<60&&!foundry;i++){await new Promise(r=>setTimeout(r,250));foundry=await check(foundryURL+'/api/config');}
 if(!foundry)throw Error('Foundry failed to start. Inspect .local/solo-foundry.log; no key is printed by this launcher.');
}
if(!foundry.keyConfigured)console.error('Foundry needs GEMINI_API_KEY in .env.local. Offline generation is disabled.');
if(!foundry.spending?.sponsoredCredits&&!(foundry.spending?.remainingUsd>0))console.error('Foundry is running without sponsored-credit permission. Restart it with GOOGLE_SPONSORED_CREDITS=1 and the same .env.local.');
if(process.argv.includes('--studio')){console.log(`Live Asset Foundry: ${foundryURL}`);process.exit(0);}
const existing=await check(appURL+'/api/v1/health');if(existing?.app==='first-landing'){console.log(`First Landing is running: ${appURL}\nLive Asset Foundry: ${foundryURL}`);process.exit(0);}
// Migrate the earlier isolated prototype once, without overwriting an existing colony.
const legacy=path.join(repo,'solo','.data');if(!fs.existsSync(path.join(data,'world.json'))&&fs.existsSync(path.join(legacy,'world.json'))){if(fs.existsSync(path.join(legacy,'world.lock')))throw Error('Close the earlier colony process before moving its save.');fs.mkdirSync(data,{recursive:true,mode:0o700});fs.cpSync(legacy,data,{recursive:true,errorOnExist:true,force:false});}
console.log(`First Landing: ${appURL}\nLive Asset Foundry: ${foundryURL}\nCredentials: ${envFile?'.env.local (server only)':'not configured'}\nSave: ${data}`);
const app=spawn(process.execPath,[path.join(repo,'solo','server.mjs')],{cwd:repo,env,stdio:'inherit'});
for(const signal of ['SIGINT','SIGTERM'])process.once(signal,()=>app.kill(signal));
app.on('exit',code=>process.exit(code||0));app.on('error',error=>{console.error(error.message);process.exit(1);});
