import test from 'node:test';
import assert from 'node:assert/strict';
import {inferTheme} from '../lib/theme-inference.mjs';
import {THEMES,validateTheme} from '../public/themes.mjs';

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

test('overlong returned art direction preserves every word in a valid draft without another paid call',async()=>{
 const shapeLanguage='Primary masses establish an L-shaped silhouette: a soaring vertical cylindrical tower paired with a low, wide rectangular dynamo hall and an elevated horizontal nacelle box. Secondary forms introduce structural rhythm through stepped tower collars, deep recessed wall bays framed by heavy pillars, and four orthogonal box-lattice rotor arms (two vertical, two horizontal) extending from a forward hub. Tertiary details at audit scale include protruding busbar conduits, intake louver slats made of stacked horizontal plates, and bright copper-orange hub rings and capacitor terminals.';
 const recipe={...structuredClone(THEMES.argonaut),shapeLanguage},original=structuredClone(recipe),usage={totalTokenCount:3501},response={id:'returned-windmill-draft'};let calls=0;
 const result=await inferTheme({prompt:'windmill power generator',create:async()=>{calls++;return {recipe,usage,response};}});
 assert.equal(calls,1);assert.doesNotThrow(()=>validateTheme(result.theme));
 const continuation=result.theme.renderingNotes.slice(original.renderingNotes.length);
 assert.ok(continuation.length>0);assert.ok(continuation.every(text=>text.startsWith('shapeLanguage continued: ')));
 assert.equal([result.theme.shapeLanguage,...continuation.map(text=>text.slice('shapeLanguage continued: '.length))].join(' '),shapeLanguage);
 assert.deepEqual(result.theme.visualChecks,original.visualChecks);assert.deepEqual(result.theme.exclusions,original.exclusions);
 assert.deepEqual(recipe,original);assert.deepEqual(result.usage,usage);assert.deepEqual(result.response,response);
 assert.deepEqual(result.normalizations.map(item=>item.field),['shapeLanguage']);
});

test('draft prose wrapping respects UTF-8 bytes and leaves frozen criteria intact',async()=>{
 const recipe={...structuredClone(THEMES.argonaut),materialFinish:('éclat — céramique brillante. ').repeat(20).trim()};
 const result=await inferTheme({prompt:'A ceramic tower',create:async()=>({recipe})});
 assert.ok(Buffer.byteLength(result.theme.materialFinish)<=500);assert.doesNotThrow(()=>validateTheme(result.theme));
 assert.equal([result.theme.materialFinish,...result.theme.renderingNotes.slice(recipe.renderingNotes.length).map(text=>text.slice('materialFinish continued: '.length))].join(' '),recipe.materialFinish);
 const invalid={...recipe,visualChecks:['x'.repeat(501)]};
 await assert.rejects(inferTheme({prompt:'A ceramic tower',create:async()=>({recipe:invalid})}),/visualChecks/);
});

test('draft overflow fails with retained response instead of silently dropping direction when notes are full',async()=>{
 const recipe={...structuredClone(THEMES.argonaut),shapeLanguage:('grounded ceramic shell ').repeat(25),renderingNotes:Array.from({length:32},(_,i)=>'Existing requirement '+i)},usage={totalTokenCount:9};
 await assert.rejects(inferTheme({prompt:'A ceramic tower',create:async()=>({recipe,usage})}),error=>{assert.match(error.message,/renderingNotes|room/);assert.deepEqual(error.themeInference.recipe,recipe);assert.deepEqual(error.themeInference.usage,usage);return true;});
});
