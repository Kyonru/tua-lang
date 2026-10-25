const assert = require('node:assert/strict');
// Optional browser regression: supply PLAYWRIGHT_MODULE and CHROMIUM_PATH,
// or install playwright-core and its matching Chromium locally.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const root = require('node:path').resolve(__dirname, '..');
const views=[['ecs','ecsExplorerHtml',['.catalog','.detail']],['object-pools','objectPoolExplorerHtml',['.catalog','.detail']],['modules','moduleGraphHtml',['.graph-scroll','.inspector']]];
(async()=>{
 const browser=await chromium.launch({executablePath: process.env.CHROMIUM_PATH || undefined});
 try {
 for(const [name,fn,panes] of views) for(const [width,height] of [[1200,700],[800,250],[500,400]]) {
  const page=await browser.newPage({viewport:{width,height}});
  const html=require(root+'/src/'+name+'/template')[fn]({cspSource:"'self'"},{}).replace(/<script[\s\S]*?<\/script>/g,'').replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/,'');
  await page.setContent(html);
  await page.addStyleTag({content:':root { --vscode-font-family: sans-serif; --vscode-font-size: 13px; }'});
  await page.evaluate(panes=>{
   // Long details and wrapped headers reproduce content-heavy/zoomed previews.
   document.querySelector('.title-block').append(document.createElement('div'));
   document.querySelector('.title-block').lastChild.style.height='65px';
   for(const selector of panes){const p=document.querySelector(selector);p.innerHTML='<div style="height:1800px">Long content</div><button class="last">Last item</button>';}
  },panes);
  for(const pane of panes) {
   const box=await page.locator(pane).boundingBox();
   if(width>720) assert(box.y+box.height<=height+1,`${name} ${width} ${pane} clipped below viewport: ${JSON.stringify(box)}`);
   await page.locator(pane+' .last').scrollIntoViewIfNeeded();
   const last=await page.locator(pane+' .last').boundingBox();
   assert(last.y>=0 && last.y+last.height<=height+1,`${name}: last item inaccessible`);
  }
  if(width>720) {
   await page.locator(panes[0]).evaluate(e=>e.scrollTop=0);
   await page.locator(panes[0]).hover(); await page.mouse.wheel(0,400); await page.waitForTimeout(150);
   assert(await page.locator(panes[0]).evaluate(e=>e.scrollTop>0),`${name}: mouse wheel does not scroll`);
  }
  console.log(`PASS ${name} ${width}x${height}`);await page.close();
 }
 const page=await browser.newPage({viewport:{width:1200,height:250}});
 const html=require(root+'/src/love/sprite-preview/template').spritePreviewHtml({cspSource:"'self'"},{source:'fixture.tua',sprites:[]}).replace(/<script[\s\S]*?<\/script>/g,'').replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/,'');
 await page.setContent(html);
 await page.locator('#sprites').evaluate(e=>e.innerHTML='<div class="workbench"><div class="stage"><div class="stage-frame">Sprite</div><div style="height:500px">Controls</div><button id="last">Last control</button></div><div style="height:1400px">Frames</div></div>');
 await page.locator('#last').scrollIntoViewIfNeeded();
 const box=await page.locator('#last').boundingBox();assert(box.y>=0&&box.y+box.height<=250,'sprite controls clipped');
 assert(await page.locator('.stage').evaluate(e=>e.scrollTop>0),'sprite stage must scroll');
 console.log('PASS sprite sticky controls 1200x250');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
