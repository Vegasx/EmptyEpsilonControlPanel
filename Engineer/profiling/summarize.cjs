const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const dir=path.resolve(process.argv[2]);
const taskSource=process.argv[3]?JSON.parse(fs.readFileSync(path.join(process.argv[3],'comparison.json'),'utf8')):null;
const gpuConfig=fs.existsSync(path.join(dir,'gpu-sources.json'))?readConfig():null;
function readConfig(){const config=JSON.parse(fs.readFileSync(path.join(dir,'gpu-sources.json'),'utf8').replace(/^\uFEFF/,''));return {runs:config.reports.map(f=>JSON.parse(fs.readFileSync(path.join(dir,f),'utf8'))),samples:JSON.parse(fs.readFileSync(path.join(dir,config.sampleFile),'utf8').replace(/^\uFEFF/,'')).filter(r=>r.timestamp)};}
const read=name=>JSON.parse(fs.readFileSync(path.join(dir,name),'utf8').replace(/^\uFEFF/,''));
const cpu=read('cpu-attribution.json'),gpu=read('gpu.json');
const runs=['native-1.json','browser-1.json','browser-2.json','native-2.json'].map(read);
function category(module){
 const m=module.toLowerCase();
 if(m.includes('epsilon'))return 'Application';
 if(/presentation|windowsbase|directwriteforwarder/.test(m))return 'WPF UI and layout';
 if(/wpfgfx|d3d|dxgi|dwrite|d2d|nvwg|nvld|igd|igc|vulkan|opengl|gdi/.test(m))return 'Graphics and driver CPU';
 if(/chrome|libegl|libgles/.test(m))return 'Chrome engine';
 if(/coreclr|clrjit|system\.|mscor|vcruntime|ucrt/.test(m))return 'Runtime and libraries';
 if(/unresolved/.test(m))return 'Unresolved';
 return 'Windows and other modules';
}
function browserTasks(trace,rolePids){
 const events=trace.traceEvents,threads=new Map();
 for(const e of events)if(e.ph==='M'&&e.name==='thread_name')threads.set(e.pid+':'+e.tid,e.args?.name);
 const mains=new Set([...threads].filter(([id,name])=>name==='CrRendererMain'&&rolePids.has(Number(id.split(':')[0]))).map(([id])=>id));
 const spans=[],pending=new Map(),markers={};
 for(const e of events){
  const id=e.pid+':'+e.tid;if(!mains.has(id))continue;
  const message=e.args?.data?.message;if(e.name==='TimeStamp'&&message?.startsWith('epsilon:'))markers[message]=e.ts;
  if(e.ph==='X'&&e.dur>0)spans.push({start:e.ts,end:e.ts+e.dur,name:e.name,thread:id});
  if(e.ph==='B'){if(!pending.has(id))pending.set(id,[]);pending.get(id).push(e);}
  if(e.ph==='E'){const begin=pending.get(id)?.pop();if(begin&&e.ts>begin.ts)spans.push({start:begin.ts,end:e.ts,name:begin.name,thread:id});}
 }
 function cat(name){
  if(/paint|raster|layerize|layer.*tree|composit/i.test(name))return 'Paint and composition preparation';
  if(/layout|style|recalc/i.test(name))return 'Style and layout';
  if(/functioncall|eventdispatch|timerfire|fireanimationframe|microtask|evaluatescript|v8|javascript/i.test(name))return 'JavaScript and event handling';
  if(/animation/i.test(name))return 'Animation processing';
  return 'Other renderer work';
 }
 const result={};
 for(const phase of ['idle','switching','settled','dragging']){
  const from=markers['epsilon:'+phase+':start'],to=markers['epsilon:'+phase+':end'];
  assert(Number.isFinite(from)&&to>from,'Missing browser phase markers: '+phase);
  const buckets=new Map();
  for(const thread of mains){
   const points=[];let serial=0;
   for(const span of spans){if(span.thread!==thread||span.end<=from||span.start>=to)continue;const s={...span,id:serial++,start:Math.max(from,span.start),end:Math.min(to,span.end)};points.push({time:s.start,add:s},{time:s.end,remove:s});}
   points.sort((a,b)=>a.time-b.time);const active=new Map();let prev=points[0]?.time;
   for(let i=0;i<points.length;){const time=points[i].time;
    if(time>prev&&active.size){const winner=[...active.values()].sort((a,b)=>b.start-a.start||a.end-b.end||a.id-b.id)[0];const key=cat(winner.name)+'|'+winner.name;buckets.set(key,(buckets.get(key)||0)+(time-prev)/1000);}
    while(i<points.length&&points[i].time===time){const p=points[i++];if(p.remove)active.delete(p.remove.id);if(p.add)active.set(p.add.id,p.add);}prev=time;
   }
  }
  result[phase]=[...buckets].map(([key,value])=>{const [group,name]=key.split('|');return {group,name,value};});
 }
 return result;
}
for(const run of runs)if(run.implementation==='browser'&&!taskSource){
 const filename='browser-'+(runs.indexOf(run)===1?'1':'2')+'-trace.json';
 run.tasks=browserTasks(read(filename),new Set(run.phases.flatMap(p=>p.pids.filter(p=>p.role==='renderer').map(p=>p.id))));
}
const output={generated:new Date().toISOString(),eventsLost:cpu.eventsLost,taskSource:taskSource?path.basename(process.argv[3]):null,conditions:runs.map(({phases,tasks,...r})=>r),gpuConditions:gpuConfig?.runs.map(({phases,...r})=>r),phases:{}};
for(const name of ['idle','switching','settled','dragging']){
 output.phases[name]={};
 for(const implementation of ['native','browser']){
  const selected=runs.filter(r=>r.implementation===implementation),n=selected.length;
  const leaves=[],engines=[],tasks=[],totals=[];
  for(const run of selected){
   const p=run.phases.find(p=>p.name===name),pids=new Map(p.pids.map(p=>[p.id,p.role]));
   const attr=cpu.results.find(r=>r.implementation===implementation&&r.phase===name&&!r.used);assert(attr,'Missing CPU phase');attr.used=true;
   for(const leaf of attr.leaves)leaves.push({group:category(leaf.module),branch:leaf.role,name:leaf.module,detail:leaf.method,value:leaf.samples*attr.sampleIntervalMs/n,samples:leaf.samples});
   let covered=0;
   for(let i=1;i<gpu.length;i++){
    const sample=gpu[i],end=Date.parse(sample.timestamp),start=Date.parse(sample.intervalStart||gpu[i-1].timestamp);
    if(!Number.isFinite(start)||!Number.isFinite(end)||start<Date.parse(p.start)||end>Date.parse(p.end)||end<=start||end-start>2000)continue;
    const dt=end-start;covered+=dt;
    for(const e of sample.active||[])if(pids.has(e.pid))engines.push({group:e.engine.split('_engtype_')[1]||e.engine,branch:pids.get(e.pid),name:e.engine,detail:'Process '+e.pid,value:e.percent/100*dt/n});
   }
   if(run.tasks)for(const task of run.tasks[name])tasks.push({...task,value:task.value/n});
   totals.push({cpuMs:p.cpuMs,elapsedMs:p.elapsedMs,sampledCpuMs:attr.totalSamples*attr.sampleIntervalMs,samples:attr.totalSamples,withStack:attr.samplesWithStack,gpuCoverageMs:covered,actions:p.actions,resets:p.resets});
  }
  function merge(list){const map=new Map();for(const item of list){const key=[item.group,item.branch||'',item.name,item.detail||''].join('|');if(!map.has(key))map.set(key,{...item,value:0});map.get(key).value+=item.value;}return [...map.values()].sort((a,b)=>b.value-a.value);}
  const sum=key=>totals.reduce((s,t)=>s+t[key],0);
  let gpuCoverageMs=sum('gpuCoverageMs')/n,gpuTrials=n;
  if(gpuConfig){
   engines.length=0;gpuCoverageMs=0;const gpuRuns=gpuConfig.runs.filter(r=>r.implementation===implementation);gpuTrials=gpuRuns.length;
   for(const run of gpuRuns){const p=run.phases.find(p=>p.name===name),pids=new Map(p.pids.map(p=>[p.id,p.role]));
    for(const sample of gpuConfig.samples){const end=Date.parse(sample.timestamp),start=Date.parse(sample.intervalStart);if(!Number.isFinite(start)||start<Date.parse(p.start)||end>Date.parse(p.end)||end<=start||end-start>2000)continue;
     const dt=end-start;gpuCoverageMs+=dt/gpuTrials;for(const e of sample.active||[])if(pids.has(e.pid))engines.push({group:e.engine.split('_engtype_')[1]||e.engine,branch:pids.get(e.pid),name:e.engine,detail:'Process '+e.pid,value:e.percent/100*dt/gpuTrials});
    }
   }
   assert(gpuCoverageMs>1000,'Insufficient GPU sample coverage');
  }
  const merged=merge(leaves),gpuMerged=merge(engines),taskMerged=taskSource?taskSource.phases[name][implementation].tasks:merge(tasks);
  const sampleTotal=sum('sampledCpuMs')/n;
  assert(Math.abs(merged.reduce((s,x)=>s+x.value,0)-sampleTotal)<.01,'CPU treemap must conserve samples');
  output.phases[name][implementation]={trials:n,cpuMs:sum('cpuMs')/n,elapsedMs:sum('elapsedMs')/n,cpuPercent:sum('cpuMs')/sum('elapsedMs')/selected[0].logicalProcessors*100,
   sampleCount:sum('samples'),sampledCpuMs:sampleTotal,stackCoverage:sum('samples')?sum('withStack')/sum('samples'):null,
   gpuTrials,gpuCoverageMs,gpuEngineMs:gpuMerged.reduce((s,e)=>s+e.value,0),actions:totals[0].actions,resets:totals[0].resets,
   cpu:merged,gpu:gpuMerged,tasks:taskMerged};
 }
}
assert(cpu.eventsLost===0,'ETW lost events; inspect before reporting');
for(const run of runs){assert(run.width===1000&&run.height===700,'Panel sizes differ');assert(run.deviceScaleFactor===runs[0].deviceScaleFactor,'Pixel scales differ');assert(run.reducedMotion===false,'Animations disabled');}
for(const run of gpuConfig?.runs||[]){assert(run.width===1000&&run.height===700,'GPU panel sizes differ');assert(run.deviceScaleFactor===runs[0].deviceScaleFactor,'GPU pixel scales differ');assert(run.reducedMotion===false,'GPU animations disabled');}
fs.writeFileSync(path.join(dir,'comparison.json'),JSON.stringify(output,null,2));
console.log(JSON.stringify(Object.fromEntries(Object.entries(output.phases).map(([p,v])=>[p,Object.fromEntries(Object.entries(v).map(([impl,r])=>[impl,{cpu:r.cpuPercent,samples:r.sampleCount,gpuEngineMs:r.gpuEngineMs,gpuCoverageMs:r.gpuCoverageMs}]))])),null,2));
