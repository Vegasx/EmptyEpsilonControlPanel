const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const {pathToFileURL}=require('node:url');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_EXE});
 try{
  const page=await browser.newPage({viewport:{width:1060,height:1100}});const errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto(pathToFileURL(path.resolve(process.argv[2])).href);const frame=page.frameLocator('iframe');
  await frame.locator('#epsilon-performance-map .ep-map').first().waitFor();
  const checks=[];
  for(const phase of ['switching','dragging','idle','settled'])for(const metric of ['cpu','gpu','tasks']){
   await frame.locator('[data-phase]').selectOption(phase);await frame.locator('[data-metric]').selectOption(metric);
   assert(await frame.locator('[data-total]').first().innerText());
   if(metric==='tasks')assert.equal(await frame.locator('.ep-empty').count(),1);
   checks.push(phase+'/'+metric);
  }
  await frame.locator('[data-phase]').selectOption('switching');await frame.locator('[data-metric]').selectOption('cpu');
  const nativePanel=frame.locator('[data-implementation="native"]');
  await nativePanel.locator('.ep-map a').first().click();assert((await nativePanel.locator('[data-path] button').count())>1);checks.push('CPU tile drill-down');
  await nativePanel.locator('[data-path] button').first().click();assert.equal(await nativePanel.locator('[data-path] button').count(),1);checks.push('Breadcrumb return');
  await nativePanel.locator('summary').click();assert(await nativePanel.locator('tbody tr').count());await nativePanel.locator('summary').click();checks.push('Exact values');
  for(const width of [1060,736,360]){
   await page.setViewportSize({width,height:1100});await page.waitForTimeout(350);
   const overflow=await frame.locator('#epsilon-performance-map').evaluate(root=>root.scrollWidth-root.clientWidth);
   assert(overflow<=2,'Horizontal overflow at '+width+': '+overflow);checks.push('Layout '+width);
  }
  await page.setViewportSize({width:1060,height:1100});await page.waitForTimeout(350);
  await page.screenshot({path:path.resolve(process.argv[3]),fullPage:true});
  assert.deepEqual(errors,[],'Visualization runtime errors');
  fs.writeFileSync(path.join(path.dirname(process.argv[3]),'view-verification.json'),JSON.stringify({checks,errors},null,2));console.log(checks.length+' visualization checks passed');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
