/** Server-only local configuration. Explicit environment > .env.local > .env. */
import fs from 'node:fs';
import path from 'node:path';
import {parseEnv} from 'node:util';
export function localConfiguration({repo,canonical=repo,environment=process.env}){
 const main=path.resolve(canonical),checkout=path.resolve(repo),files=[];let values={};
 const base=[path.join(main,'.env'),path.join(checkout,'.env')].find(file=>fs.existsSync(file));
 if(base){values={...values,...parseEnv(fs.readFileSync(base,'utf8'))};files.push(base);}
 const override=environment.ARGON_ENV_FILE;
 const local=override?path.resolve(override):[path.join(main,'.env.local'),path.join(checkout,'.env.local')].find(file=>fs.existsSync(file));
 if(override&&!fs.existsSync(local))throw Error('ARGON_ENV_FILE does not exist.');
 if(local){values={...values,...parseEnv(fs.readFileSync(local,'utf8'))};files.push(local);}
 return {environment:{...values,...environment},files,localFile:local||null};
}
