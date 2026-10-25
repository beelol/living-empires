import test from 'node:test';
import assert from 'node:assert/strict';
import {themeInferencePrompt} from '../foundry.mjs';
test('per-function inference preserves full brief, exact palette and bounded style hint',()=>{
 const brief='solar function '+ 'x'.repeat(1980),palette={shell:'#c2a275',frame:'#3b2615',accent:'#d49b42'};
 const prompt=themeInferencePrompt({userPrompt:brief,artDirection:'hut chimney '.repeat(1000),familyPalette:palette},{artDirection:'old hut'});
 assert.ok(prompt.includes(brief));assert.ok(prompt.length<=3000);assert.ok(prompt.includes('shell #c2a275'));assert.ok(prompt.includes('Do not invent a different gameplayRole'));
 const court=themeInferencePrompt({userPrompt:'basketball court with hoop',familyPalette:palette},{artDirection:'timber'});
 assert.ok(court.includes('basketball court with hoop'));assert.ok(!court.includes('Requested function: solar'));
});
