import {chromium} from '../../../../playground/tests/node_modules/playwright/index.mjs';
import fs from 'node:fs/promises';
const dir=new URL('.',import.meta.url).pathname;
const manifest=JSON.parse(await fs.readFile(`${dir}/manifest.json`,'utf8'));
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:900,height:450},deviceScaleFactor:1});
const results=[];
try {
 for (const entry of manifest) {
  const svg=await fs.readFile(`${dir}/${entry.file}`,'utf8');
  await page.setContent(`<!doctype html><body style="margin:20px;background:white">${svg}</body>`);
  const check=await page.evaluate(()=>{
   const root=document.querySelector('svg');
   const formulas=[...root.querySelectorAll('svg')].filter(n=>n.querySelector('path')&&!n.closest('template'));
   const painted=formulas.flatMap(math=>[...math.querySelectorAll('path,rect,line,text')].map(n=>{
    const s=getComputedStyle(n);
    return ['fill','stroke','stroke-width','filter','opacity'].map(p=>s.getPropertyValue(p));
   }));
   if(!painted.length) throw Error('no formula paint');
   for(const math of formulas) for(const n of math.querySelectorAll('path,rect,line,text')) {
    const s=getComputedStyle(n);
    if(s.getPropertyValue('filter')!=='none') throw Error('formula filter leaked');
    for(const p of ['fill','stroke']) {
     if(n.hasAttribute(p)) {
      const probe=document.createElementNS('http://www.w3.org/2000/svg','path');
      probe.style.setProperty(p,n.getAttribute(p),'important');root.append(probe);
      const want=getComputedStyle(probe).getPropertyValue(p);probe.remove();
      if(s.getPropertyValue(p)!==want) throw Error(`paint mismatch ${p}`);
     }
    }
   }
   return {formulaCount:formulas.length,primitiveCount:painted.length};
  });
  results.push({...entry,...check});
 }
 // Browser !important behavior: compare actual emitted math under real classDef CSS
 // with the exact same math in an isolated canvas. All screenshots retain source bytes.
 const styled=manifest.filter(x=>x.case==='fraction-styled'&&x.pipeline==='parity');
 for(const entry of styled) {
  const svg=await fs.readFile(`${dir}/${entry.file}`,'utf8');
  await page.setContent(`<body>${svg}</body>`);
  const {math,viewBox,css,id}=await page.evaluate(()=>{
   const root=document.querySelector('svg');
   const math=[...root.querySelectorAll('svg')].find(n=>n.querySelector('path')&&!n.closest('template'));
   return {math:math.innerHTML,viewBox:math.getAttribute("viewBox"),css:[...root.querySelectorAll('style')].map(n=>n.textContent).join(''),id:root.id};
  });
  const capture=async context=>{
   await page.setContent(`<body style="margin:0"><svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="240" height="120" viewBox="${viewBox}">${context?`<style>${css}</style><g class="node hostile" data-look="neo"><g class="label nodeLabel">`:''}${math}${context?'</g></g>':''}</svg></body>`);
   return page.locator('body>svg').screenshot();
  };
  const a=await capture(false), b=await capture(true);
  const ink=await page.evaluate(async data=>{const img=new Image();img.src=data;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const x=c.getContext('2d');x.drawImage(img,0,0);const p=x.getImageData(0,0,c.width,c.height).data;let n=0;for(let i=0;i<p.length;i+=4)if(p[i+3]>128&&Math.min(p[i],p[i+1],p[i+2])<200)n++;return n;},`data:image/png;base64,${a.toString('base64')}`);
  if(ink===0)throw Error('browser reference has no ink');
  if(!a.equals(b)) throw Error(`${entry.line} browser important paint differs`);
  results.push({line:entry.line,case:'browser-classDef-important',exactScreenshotMatch:true,referenceInkPixels:ink});
 }
 await fs.writeFile(`${dir}/browser-results.json`,JSON.stringify({browser:await browser.version(),results},null,2)+'\n');
 console.log(JSON.stringify({browser:await browser.version(),cases:results.length,passed:true}));
} finally {await browser.close();}
