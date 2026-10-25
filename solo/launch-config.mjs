import path from 'node:path';

function port(value,name){
 const text=String(value);
 if(!/^\d+$/.test(text)||Number(text)<1||Number(text)>65535)throw Error(`${name} must be a port between 1 and 65535.`);
 return Number(text);
}

export function launchConfiguration({repo,canonical,environment}){
 const appPort=port(environment.SOLO_PORT||4320,'SOLO_PORT');
 let supplied;
 if(environment.SOLO_FOUNDRY_URL){
  supplied=new URL(environment.SOLO_FOUNDRY_URL);
  if(supplied.protocol!=='http:'||!['127.0.0.1','localhost'].includes(supplied.hostname)||supplied.pathname!=='/'||supplied.search||supplied.hash||supplied.username||supplied.password)throw Error('Foundry URL must use a plain loopback host and root path.');
 }
 const foundryPort=port(environment.FOUNDRY_PORT||supplied?.port||environment.PORT||4317,'FOUNDRY_PORT');
 if(supplied&&Number(supplied.port||80)!==foundryPort)throw Error('FOUNDRY_PORT conflicts with SOLO_FOUNDRY_URL.');
 if(appPort===foundryPort)throw Error('Game and Foundry need different ports on the same loopback host.');
 const appURL=`http://127.0.0.1:${appPort}`,foundryURL=`http://127.0.0.1:${foundryPort}`;
 return {
  repo:path.resolve(repo),appURL,foundryURL,runtimeDir:path.join(repo,'.local'),
  environment:{...environment,PORT:String(foundryPort),SOLO_PORT:String(appPort),SOLO_FOUNDRY_URL:foundryURL,
   SOLO_DATA_DIR:environment.SOLO_DATA_DIR||path.join(canonical,'.local','solo-colony'),
   ASSET_DATA_DIR:environment.ASSET_DATA_DIR||path.join(canonical,'.data')}
 };
}

export function verifyPair(config,foundry,app){
 if(foundry&&(foundry.service!=='asset-foundry'||foundry.sourceRoot!==config.repo))throw Error('Foundry is running from a different checkout. Preserve that service; stop it when idle or choose FOUNDRY_PORT for this branch.');
 if(foundry){
  const cap=foundry.capabilities;
  if(foundry.protocolVersion!==1||foundry.packageVersion!==1||!cap?.nativeProvider||cap.pipelineVersion!==1||cap.providerFormat!=='response-format-json-v1'||cap.maxAttempts!==30||cap.maxProviderCalls!==60||cap.noDeadline!==true||cap.styleCompletion!==true)throw Error('Foundry binary capabilities are incompatible or live transport is disabled. Rebuild this checkout before playing.');
  if(!/^[a-f0-9]{64}$/.test(foundry.rustBinarySha256||'')||(config.binaryHash&&foundry.rustBinarySha256!==config.binaryHash))throw Error('Foundry binary hash does not match the verified local binary.');
  if(config.sourceHash&&cap.sourceSha256!==config.sourceHash)throw Error('Foundry binary was built from stale source.');
  for(const role of ['creator','repair','reviewer'])if(typeof foundry.roleModels?.[role]!=='string'||(config.models&&foundry.roleModels[role]!==config.models[role]))throw Error('Foundry creator/reviewer models do not match the launch configuration.');
 }
 if(app){
  if(app.app!=='first-landing'||app.sourceRoot!==config.repo)throw Error('The game port belongs to a different checkout. Use SOLO_PORT for this branch.');
  if(app.apiVersion!==1||app.saveVersion!==2||(config.appSourceHash&&app.sourceSha256!==config.appSourceHash))throw Error('The running game has stale source or an incompatible protocol. Restart that game when idle.');
  if(app.foundryURL!==config.foundryURL)throw Error('The running game points to a different Foundry. Use a separate game port or restart that game.');
 }
}
