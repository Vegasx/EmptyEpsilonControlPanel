const fs=require('node:fs/promises');
const path=require('node:path');
const os=require('node:os');
const {spawn}=require('node:child_process');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const root=path.resolve(__dirname,'..');
const out=path.resolve(process.argv[2]);
(async()=>{
 const server=spawn(process.execPath,[path.join(root,'server.mjs')],{cwd:root,env:{...process.env,PORT:'0'},windowsHide:true,stdio:['ignore','pipe','pipe']});
 let message='';server.stdout.on('data',d=>message+=d);let context;
 try{
  for(let i=0;i<100&&!message.includes('Engineering controller:');i++)await sleep(100);
  const url=message.match(/Engineering controller: (http:\/\/\S+)/)?.[1];if(!url)throw Error(message);
  const {chromeLaunch}=await import(require('node:url').pathToFileURL(path.join(root,'browser.mjs')).href);
  const launch=chromeLaunch(url,'profile');const chrome=process.env.CHROME_EXE||launch?.command;if(!chrome)throw Error('Installed Chrome not found; set CHROME_EXE');
  context=await chromium.launchPersistentContext(path.join(__dirname,'artifacts','chrome-profile'),{executablePath:chrome,headless:false,
   viewport:{width:1000,height:700},deviceScaleFactor:Number(process.env.PROFILE_SCALE||1),args:['--app='+url,'--window-position=40,40','--no-first-run','--no-default-browser-check'],
   ignoreDefaultArgs:['--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows']});
  const page=context.pages()[0];await page.goto(url);await page.waitForFunction(()=>!document.querySelector('#mode-switch').disabled);
  const cdp=await context.newCDPSession(page),browser=await context.browser().newBrowserCDPSession();
  await page.evaluate(()=>document.fonts.ready);await sleep(6000);
  await page.evaluate(async()=>{
   for(let i=1;i<=30;i++){document.querySelector('#mode-switch').click();if(i%7===0)document.querySelector('#reset').click();await new Promise(r=>setTimeout(r,100));}
   const input=document.querySelector('#reactor');for(let i=1;i<=60;i++){input.value=Math.round((.5+.48*Math.sin(i*Math.PI/30))*300);input.dispatchEvent(new Event('input',{bubbles:true}));await new Promise(r=>setTimeout(r,33));}
   input.value=100;input.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('#reset').click();
  });
  await sleep(6000);
  const detailedTrace=!process.env.PROFILE_NO_BROWSER_TRACE;
  if(detailedTrace)await browser.send('Tracing.start',{categories:'toplevel,devtools.timeline,disabled-by-default-devtools.timeline,blink,cc,gpu',transferMode:'ReturnAsStream'});
  const info=await page.evaluate(()=>({width:innerWidth,height:innerHeight,deviceScaleFactor:devicePixelRatio,reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches,userAgent:navigator.userAgent}));
  const phases=[];
  for(const name of ['idle','switching','settled','dragging']){
   const before=await browser.send('SystemInfo.getProcessInfo');const cpuStart=before.processInfo.reduce((s,p)=>s+p.cpuTime,0);
   const start=new Date().toISOString();const clock=performance.now();
   const actions=await page.evaluate(async name=>{
    console.timeStamp('epsilon:'+name+':start');
    const count=name==='switching'?120:name==='dragging'?300:0,duration=name==='switching'?12000:name==='dragging'?10000:5000;
    let resets=0;const began=performance.now();
    if(!count)await new Promise(r=>setTimeout(r,duration));
    for(let i=1;i<=count;i++){
     await new Promise(r=>setTimeout(r,Math.max(0,i*duration/count-(performance.now()-began))));
     if(name==='switching'){document.querySelector('#mode-switch').click();if(i%7===0){document.querySelector('#reset').click();resets++;}}
     else {const input=document.querySelector('#reactor');input.value=Math.round((.5+.48*Math.sin(i*Math.PI/30))*300);input.dispatchEvent(new Event('input',{bubbles:true}));}
    }console.timeStamp('epsilon:'+name+':end');return {actions:count,resets};
   },name);
   const end=new Date().toISOString(),elapsedMs=performance.now()-clock;
   const after=await browser.send('SystemInfo.getProcessInfo');
   phases.push({name,start,end,elapsedMs,...actions,cpuMs:(after.processInfo.reduce((s,p)=>s+p.cpuTime,0)-cpuStart)*1000,
    pids:after.processInfo.map(p=>({id:p.id,role:p.type})),serverPid:server.pid});
  }
  if(detailedTrace){
  const finished=new Promise(r=>browser.once('Tracing.tracingComplete',r));await browser.send('Tracing.end');const {stream}=await finished;
  const chunks=[];for(;;){const row=await browser.send('IO.read',{handle:stream});chunks.push(row.base64Encoded?Buffer.from(row.data,'base64'):Buffer.from(row.data));if(row.eof)break;}
  await browser.send('IO.close',{handle:stream});await fs.writeFile(out.replace('.json','-trace.json'),Buffer.concat(chunks));
  }
  await fs.writeFile(out,JSON.stringify({implementation:'browser',timestamp:new Date().toISOString(),...info,logicalProcessors:os.cpus().length,detailedTrace,interactionWarmup:true,phases},null,2));
 }finally{if(context)await context.close();server.kill();}
})().catch(e=>{console.error(e);process.exitCode=1;});
