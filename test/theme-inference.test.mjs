import test from 'node:test';
import assert from 'node:assert/strict';
import {inferTheme} from '../lib/theme-inference.mjs';
import {THEMES} from '../public/themes.mjs';

test('theme inference prepares one structured call and returns an editable validated draft',async()=>{
  let calls=0,request;
  const result=await inferTheme({prompt:'A moss-covered forest lookout for a lost civilization',apiKey:'test',model:'test',create:async input=>{calls++;request=input;return {recipe:{...structuredClone(THEMES.sylvan),id:'custom-grove',assumptions:['Scale was not specified; proposed six-metre pavilion.']},usage:{totalTokenCount:30},response:{test:true}};}});
  assert.equal(calls,1);assert.equal(request.provider,'gemini');assert.ok(request.recipeSchema);
  assert.match(request.prompt,/assumptions/);assert.match(request.prompt,/not implemented/);
  assert.equal(result.theme.id,'custom-grove');assert.equal(result.theme.referencePreset,'sylvan');assert.equal(result.usage.totalTokenCount,30);
});
test('theme inference rejects malformed contracts and bad prompts without retries',async()=>{
  let calls=0;
  await assert.rejects(inferTheme({prompt:'x',create:async()=>{calls++;}}),/5–3000/);assert.equal(calls,0);
  await assert.rejects(inferTheme({prompt:'A new alien world',create:async()=>{calls++;return{recipe:{id:'unknown'}};}}));assert.equal(calls,1);
});

for(const palette of [{shell:'#ffffff'},['#ffffff','#000000','#112233','#445566','#778899'],{shell:'ivory',frame:'#000000',accent:'#112233',ground:'#445566',rock:'#778899'}])test('invalid live palettes retain returned usage and response without retries: '+JSON.stringify(palette),async()=>{
 let calls=0;const recipe={...structuredClone(THEMES.sylvan),palette},usage={totalTokenCount:42},response={id:'returned-response'};
 await assert.rejects(inferTheme({prompt:'A copper observatory',create:async input=>{calls++;assert.match(input.prompt,/shell, frame, accent, ground, rock/);return {recipe,usage,response};}}),error=>{assert.match(error.message,/five #RRGGBB/);assert.deepEqual(error.themeInference,{recipe,usage,response});return true;});assert.equal(calls,1);
});
