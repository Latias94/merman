import {chromium} from '../../../../playground/tests/node_modules/playwright/index.mjs';
import fs from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const dir = fileURLToPath(new URL('.', import.meta.url));
const manifest = JSON.parse(await fs.readFile(`${dir}/manifest.json`, 'utf8'));
const browser = await chromium.launch({headless: true});
const page = await browser.newPage({viewport: {width: 900, height: 450}, deviceScaleFactor: 1});
const results = [];

// Compare screenshots of the complete emitted SVG, retaining every instance and ancestor.
// Hiding only one formula distinguishes its visible contribution from borders and other labels.
async function visibleContribution(before, after) {
  return page.evaluate(async ({before, after}) => {
    const read = async data => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = image.width;
      canvas.height = image.height;
      const context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      return {width: canvas.width, height: canvas.height,
        pixels: context.getImageData(0, 0, canvas.width, canvas.height).data};
    };
    const a = await read(before), b = await read(after);
    if (a.width !== b.width || a.height !== b.height) throw Error('formula hiding resized SVG');
    let pixels = 0, left = a.width, top = a.height, right = 0, bottom = 0;
    for (let y = 0; y < a.height; y++) for (let x = 0; x < a.width; x++) {
      const i = (y * a.width + x) * 4;
      if ([0, 1, 2, 3].some(c => a.pixels[i + c] !== b.pixels[i + c])) {
        pixels++;
        left = Math.min(left, x); top = Math.min(top, y);
        right = Math.max(right, x + 1); bottom = Math.max(bottom, y + 1);
      }
    }
    return {pixels, boundsPx: pixels ? [left, top, right, bottom] : null};
  }, {before: before.toString('base64'), after: after.toString('base64')});
}

try {
  for (const entry of manifest) {
    const svg = await fs.readFile(`${dir}/${entry.file}`, 'utf8');
    await page.setContent(`<!doctype html><body style="margin:20px;background:white">${svg}</body>`);
    await page.evaluate(() => document.fonts.ready);
    const check = await page.evaluate(() => {
      const root = document.querySelector('body > svg');
      const formulas = [...root.querySelectorAll('svg')].map((math, index) => ({math, index}))
        .filter(({math}) => math.querySelector(':scope > path, :scope > image') && !math.closest('template'));
      if (!formulas.length) throw Error('no emitted formula SVG');
      let primitiveCount = 0;
      for (const {math} of formulas) for (const n of math.querySelectorAll('path,rect,line,text,image')) {
        primitiveCount++;
        const style = getComputedStyle(n);
        if (style.getPropertyValue('filter') !== 'none') throw Error('formula filter leaked');
        for (const property of ['fill', 'stroke']) if (n.hasAttribute(property)) {
          const probe = document.createElementNS('http://www.w3.org/2000/svg', 'path');
          probe.style.setProperty(property, n.getAttribute(property), 'important');
          root.append(probe);
          const want = getComputedStyle(probe).getPropertyValue(property);
          probe.remove();
          if (style.getPropertyValue(property) !== want) throw Error(`paint mismatch ${property}`);
        }
      }
      return {viewBox: root.getAttribute('viewBox'), primitiveCount,
        formulas: formulas.map(({math, index}) => ({index, style: math.getAttribute('style')}))};
    });
    const original = await page.locator('body > svg').screenshot();
    const visibleFormulas = [];
    for (const formula of check.formulas) {
      await page.evaluate(index => {
        document.querySelector('body > svg').querySelectorAll('svg')[index]
          .style.setProperty('opacity', '0', 'important');
      }, formula.index);
      const hidden = await page.locator('body > svg').screenshot();
      await page.evaluate(({index, style}) => {
        const math = document.querySelector('body > svg').querySelectorAll('svg')[index];
        if (style === null) math.removeAttribute('style'); else math.setAttribute('style', style);
      }, formula);
      const contribution = await visibleContribution(original, hidden);
      if (!contribution.pixels) throw Error(`${entry.file}: formula ${formula.index} has no visible contribution in full SVG`);
      visibleFormulas.push(contribution);
    }
    results.push({...entry, check: 'full-svg-visible-formula-contribution',
      viewBox: check.viewBox, formulaCount: visibleFormulas.length,
      primitiveCount: check.primitiveCount, visibleFormulas});
  }

  // These pure-formula fixtures share layout across pipelines. Compare independent full-SVG
  // screenshots, not a copied native scene. One CSS pixel permits raster edge quantization only.
  const placementCases = new Set(['flowchart', 'fraction-styled', 'swimlane', 'sequence', 'cjk-smash', 'emoji']);
  const placementTolerancePx = 1;
  for (const parity of results.filter(x => x.pipeline === 'parity')) {
    if (!placementCases.has(parity.case)) {
      parity.placementCheck = 'not-compared-by-this-fixture-contract';
      continue;
    }
    const native = results.find(x => x.line === parity.line && x.case === parity.case && x.pipeline === 'resvg-safe');
    if (!native || native.viewBox !== parity.viewBox || native.formulaCount !== parity.formulaCount) {
      throw Error(`${parity.file}: parity/native viewport or formula count differs`);
    }
    const deltas = parity.visibleFormulas.flatMap((formula, index) =>
      formula.boundsPx.map((edge, axis) => Math.abs(edge - native.visibleFormulas[index].boundsPx[axis])));
    const maxEdgeDeltaPx = Math.max(...deltas);
    if (maxEdgeDeltaPx > placementTolerancePx) {
      throw Error(`${parity.line} ${parity.case}: full-SVG formula placement differs by ${maxEdgeDeltaPx}px (limit ${placementTolerancePx}px)`);
    }
    results.push({line: parity.line, case: parity.case, check: 'parity-native-visible-ink-placement',
      placementTolerancePx, maxEdgeDeltaPx});
  }

  // Limited paint oracle: reassemble the emitted glyph payload in isolated and classDef contexts.
  // This checks important paint precedence only; it does not preserve or validate actual placement.
  for (const entry of manifest.filter(x => x.case === 'fraction-styled' && x.pipeline === 'parity')) {
    const svg = await fs.readFile(`${dir}/${entry.file}`, 'utf8');
    await page.setContent(`<body>${svg}</body>`);
    const {math, viewBox, css, id, nodeClasses, look} = await page.evaluate(() => {
      const root = document.querySelector('svg');
      const math = [...root.querySelectorAll('svg')].find(n => n.querySelector(':scope > path') && !n.closest('template'));
      const node = root.querySelector('g.node.hostile');
      if (!math || !node) throw Error('styled fraction fixture missing math or hostile node');
      return {math: math.innerHTML, viewBox: math.getAttribute('viewBox'),
        css: [...root.querySelectorAll('style')].map(n => n.textContent).join(''), id: root.id,
        nodeClasses: node.getAttribute('class'), look: node.getAttribute('data-look') || ''};
    });
    const capture = async context => {
      await page.setContent(`<body style="margin:0"><svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="240" height="120" viewBox="${viewBox}">${context ? `<style>${css}</style><g class="${nodeClasses}" data-look="${look}"><g class="label nodeLabel">` : ''}${math}${context ? '</g></g>' : ''}</svg></body>`);
      return page.locator('body > svg').screenshot();
    };
    const a = await capture(false), b = await capture(true);
    const ink = await page.evaluate(async data => {
      const image = new Image(); image.src = data; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const context = canvas.getContext('2d'); context.drawImage(image, 0, 0);
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      let ink = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i + 3] > 128 && Math.min(pixels[i], pixels[i + 1], pixels[i + 2]) < 200) ink++;
      }
      return ink;
    }, `data:image/png;base64,${a.toString('base64')}`);
    if (!ink) throw Error('isolated glyph paint reference has no ink');
    if (!a.equals(b)) throw Error(`${entry.line}: reassembled glyph classDef paint differs`);
    results.push({line: entry.line, case: 'fraction-styled', check: 'reassembled-glyph-classDef-important-paint',
      exactScreenshotMatch: true, referenceInkPixels: ink});
  }
  const report = {browser: await browser.version(), source: 'committed-svg-exports', results};
  if (!process.argv.includes('--check-only')) {
    await fs.writeFile(`${dir}/browser-results.json`, JSON.stringify(report, null, 2) + '\n');
  }
  console.log(JSON.stringify({browser: report.browser, source: report.source, checks: results.length,
    visibleFormulas: results.reduce((n, x) => n + (x.formulaCount || 0), 0),
    placementPairs: results.filter(x => x.check === 'parity-native-visible-ink-placement').length,
    passed: true}));
} finally {
  await browser.close();
}
