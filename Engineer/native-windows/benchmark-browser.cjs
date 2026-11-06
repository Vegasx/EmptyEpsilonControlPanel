// Development-only baseline. Uses an isolated profile inside native-windows/tools.
// Never changes the browser app or normal Chrome profile/settings.
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const {spawn,execFileSync} = require('node:child_process');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root=path.resolve(__dirname,'..');
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
 const server=spawn(process.execPath,[path.join(root,'server.mjs')],{cwd:root,env:{...process.env,PORT:'0'},windowsHide:true,stdio:['ignore','pipe','pipe']});
 let log='';server.stdout.on('data',data=>{log+=data;});server.stderr.on('data',data=>process.stderr.write(data));
 let context;
 try {
  for(let i=0;i<200&&!log.includes('Engineering controller:');i++)await sleep(50);
  const url=log.match(/Engineering controller: (http:\/\/\S+)/)?.[1];if(!url)throw new Error('Server startup failed: '+log);
  context=await chromium.launchPersistentContext(path.join(__dirname,'tools','benchmark-chrome-profile'),{executablePath:process.env.CHROME_EXE||'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:false,viewport:null,args:['--start-fullscreen','--no-first-run','--no-default-browser-check'],ignoreDefaultArgs:['--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-backgrounding-occluded-windows']});
  const page=context.pages()[0];await page.goto(url);await page.locator('#mode-switch').waitFor({state:'visible'});await page.waitForFunction(()=>!document.querySelector('#mode-switch').disabled);
  const browserSession=await context.browser().newBrowserCDPSession();
  const pageSession=await context.newCDPSession(page);
  const {windowId}=await pageSession.send('Browser.getWindowForTarget');
  await browserSession.send('Browser.setWindowBounds',{windowId,bounds:{windowState:'fullscreen'}});
  await page.evaluate(()=>document.fonts.ready);await sleep(8000);
  async function stats(){
   const {processInfo}=await browserSession.send('SystemInfo.getProcessInfo');
   const ids=processInfo.map(p=>p.id).join(',');
   const memory=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command',`Get-Process -Id ${ids} -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64 | ConvertTo-Json -Compress`],{encoding:'utf8',windowsHide:true}));
   const rows=Array.isArray(memory)?memory:[memory];
   return {cpuSeconds:processInfo.reduce((sum,p)=>sum+p.cpuTime,0),processes:processInfo.map(p=>({id:p.id,type:p.type})),workingMiB:rows.reduce((sum,p)=>sum+p.WorkingSet64,0)/1048576,privateMiB:rows.reduce((sum,p)=>sum+p.PrivateMemorySize64,0)/1048576};
  }
  const conditions=await page.evaluate(()=>({widthCss:innerWidth,heightCss:innerHeight,devicePixelRatio,screenWidth:screen.width,screenHeight:screen.height,userAgent:navigator.userAgent,reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches}));
  async function measure(rapid){
   const before=await stats();const start=performance.now();
   const frames=await page.evaluate(async rapid=>{
    if(!rapid){await new Promise(r=>setTimeout(r,10000));return null;}
    let switches=0,resets=0,last,done=false;const intervals=[];
    function frame(){if(done)return;const now=performance.now();if(last!==undefined)intervals.push(now-last);last=now;requestAnimationFrame(frame);}requestAnimationFrame(frame);
    const timer=setInterval(()=>{document.querySelector('#mode-switch').click();switches++;if(switches%7===0){document.querySelector('#reset').click();resets++;}},100);
    await new Promise(r=>setTimeout(r,20000));clearInterval(timer);done=true;
    intervals.sort((a,b)=>a-b);const q=f=>intervals[Math.min(intervals.length-1,Math.ceil(intervals.length*f)-1)];
    return {switches,resets,count:intervals.length,wallCallbackMedianMs:q(.5),wallCallbackP95Ms:q(.95),wallCallbackP99Ms:q(.99),wallCallbackMaxMs:intervals.at(-1)};
   },rapid);
   const elapsed=(performance.now()-start)/1000;const after=await stats();const cpu=after.cpuSeconds-before.cpuSeconds;
   return {phase:rapid?'rapid-switch-10Hz':'idle',elapsedSeconds:elapsed,cpuSeconds:cpu,cpuPercentOfMachine:cpu/elapsed/os.cpus().length*100,cpuPercentOfOneLogicalCore:cpu/elapsed*100,workingSetMiB:after.workingMiB,privateMiB:after.privateMiB,frames,processes:after.processes};
  }
  const idle=await measure(false);const rapid=await measure(true);await sleep(1000);const recovery=await measure(false);
  const report={conditions,logicalProcessors:os.cpus().length,cpuModel:os.cpus()[0].model,idle,rapid,recovery,note:'Headful installed Chrome with isolated profile, same unchanged browser source/config; CDP process CPU sums browser/renderer/GPU/utility. Working set sums shared pages more than once. Node static server and benchmark harness excluded. rAF wall callback timing is not display-present timing.'};
  await fs.writeFile(path.join(__dirname,'artifacts','browser-performance.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
 }finally{if(context)await context.close();server.kill();}
})().catch(error=>{console.error(error);process.exitCode=1;});
