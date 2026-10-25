import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {SpendLedger} from '../lib/spend-ledger.mjs';
function folder(t){const p=fs.mkdtempSync(path.join(os.tmpdir(),'foundry-spend-'));t.after(()=>fs.rmSync(p,{recursive:true,force:true}));return p;}
test('default allowance remains closed without operator authorization',t=>{
 const ledger=new SpendLedger(folder(t));
 assert.throws(()=>ledger.reserve({model:'gemini-3.8-flash'}),/allowance/);
});
test('explicit sponsored mode records unknown and returned usage across restart without a dollar ceiling',t=>{
 const p=folder(t), ledger=new SpendLedger(p,undefined,{sponsoredCredits:true});
 const id=ledger.reserve({jobId:'job-1',label:'creator',model:'sponsored-model',managed:true});
 ledger.unknown(id);
 const again=new SpendLedger(p,undefined,{sponsoredCredits:true});
 assert.equal(again.state.entries[0].status,'failed_or_unknown');
 const id2=again.reserve({jobId:'job-2',label:'review',model:'sponsored-model',managed:false});
 again.settle(id2,{totalTokenCount:100});
 assert.equal(again.summary().sponsoredCredits,true);
 assert.equal(again.summary().capUsd,null);
 assert.equal(again.state.entries[1].usage.totalTokenCount,100);
 assert.equal(again.state.entries[1].allowanceUsd,0);
});
test('sponsored setting is explicit and not inherited from an old ledger',t=>{
 const p=folder(t);new SpendLedger(p,undefined,{sponsoredCredits:true});
 const capped=new SpendLedger(p);
 assert.throws(()=>capped.reserve({model:'gemini-3.8-flash'}),/allowance/);
});
