import test from 'node:test';
import assert from 'node:assert/strict';
import {systems, getCoolantAllocation} from '../model.mjs';
const weights = values => Object.fromEntries(systems.map(({id},index)=>[id,values[index]||0]));
test('one, two, and three full coolant sliders show a shared 100% pool',()=>{
  for(const count of [1,2,3]){
    const allocation=getCoolantAllocation(weights(Array(count).fill(300)));
    assert.ok(Math.abs(allocation.scaleMax-100/count)<1e-10);
    for(let i=0;i<count;i++)assert.ok(Math.abs(allocation.shares[systems[i].id]-100/count)<1e-10);
    assert.equal(Math.round(allocation.shares.reactor),count===3?33:100/count);
  }
});
test('unequal coolant settings are proportional and retain exact positions',()=>{
  const raw=weights([300,150,50]);
  const before=structuredClone(raw);
  const allocation=getCoolantAllocation(raw);
  assert.equal(allocation.shares.reactor,60);
  assert.equal(allocation.shares.beamweapons,30);
  assert.equal(allocation.shares.missilesystem,10);
  assert.equal(allocation.scaleMax,60);
  assert.deepEqual(raw,before);
});
test('empty coolant is finite and a single partial slider leaves capacity unused',()=>{
  const empty=getCoolantAllocation(weights([]));
  assert.equal(empty.scaleMax,100);
  assert.ok(Object.values(empty.shares).every(share=>share===0));
  const partial=getCoolantAllocation(weights([150]));
  assert.equal(partial.shares.reactor,50);
  assert.equal(partial.scaleMax,100);
});
test('changing one coolant weight updates every share and the ruler consistently',()=>{
  for(const raw of [weights([300,300]),weights([300,300,300]),weights([300,150,100,50,25,10,5,1])]){
    const allocation=getCoolantAllocation(raw);
    assert.ok(Math.abs(Object.values(allocation.shares).reduce((sum,value)=>sum+value,0)-100)<1e-10);
    for(const {id} of systems)assert.ok(Math.abs(allocation.shares[id]-raw[id]/300*allocation.scaleMax)<1e-10);
  }
});

test('coolant ruler never exceeds 100% for barely raised sliders',()=>{
  for(const values of [[1],[15],[150],[299],[150,149],[1,1,1,1,1,1,1,1]]){
    const allocation=getCoolantAllocation(weights(values));
    assert.equal(allocation.scaleMax,100);
    assert.ok(Math.abs(Object.values(allocation.shares).reduce((sum,share)=>sum+share,0)-allocation.total/3)<1e-10);
  }
  assert.equal(getCoolantAllocation(weights([300,300])).scaleMax,50);
  assert.ok(Math.abs(getCoolantAllocation(weights([300,300,300])).scaleMax-100/3)<1e-10);
});
test('under-budget coolant matches the ruler instead of forcing a 100% total',()=>{
  const single=getCoolantAllocation(weights([100]));
  assert.equal(Math.round(single.shares.reactor),33);
  assert.equal(single.scaleMax,100);
  const two=getCoolantAllocation(weights([90,60]));
  assert.equal(two.shares.reactor,30);
  assert.equal(two.shares.beamweapons,20);
  assert.equal(Object.values(two.shares).reduce((sum,value)=>sum+value,0),50);
  const full=getCoolantAllocation(weights([180,120]));
  assert.equal(full.shares.reactor,60);
  assert.equal(full.shares.beamweapons,40);
  const over=getCoolantAllocation(weights([300,150]));
  assert.ok(Math.abs(over.shares.reactor-200/3)<1e-10);
  assert.ok(Math.abs(over.shares.beamweapons-100/3)<1e-10);
});