const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');

const requireFromCwd = createRequire(path.resolve(process.cwd(), 'package.json'));
const katex = requireFromCwd('katex');

const lineBreakRegex = /<br\s*\/?>/gi;
const katexRegex = /\$\$(.*)\$\$/g;

function hasKatex(text) {
  return String(text ?? '').includes('$$');
}

function renderKatexHtml(text, config) {
  const input = String(text ?? '');
  if (!hasKatex(input)) {
    return input;
  }

  const output =
    config && config.forceLegacyMathML ? 'htmlAndMathml' : 'mathml';

  return input
    .split(lineBreakRegex)
    .map((line) =>
      hasKatex(line)
        ? `<div style="display: flex; align-items: center; justify-content: center; white-space: nowrap;">${line}</div>`
        : `<div>${line}</div>`
    )
    .join('')
    .replace(katexRegex, (_, content) =>
      katex
        .renderToString(content, {
          throwOnError: true,
          displayMode: true,
          output,
        })
        .replace(/\n/g, ' ')
        .replace(/<annotation.*<\/annotation>/g, '')
    );
}

async function measureHtml(html, styleCss, maxWidthPx) {
  const puppeteer = requireFromCwd('puppeteer');
  const mermaidCliIndexHtml = path.join(
    process.cwd(),
    'node_modules',
    '@mermaid-js',
    'mermaid-cli',
    'dist',
    'index.html'
  );
  const browser = await puppeteer.launch({
    headless: 'shell',
    args: [
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--allow-file-access-from-files',
      '--force-device-scale-factor=1',
    ],
  });
  try {
    const page = await browser.newPage();
    await page.setViewport({
      width: 1200,
      height: 800,
      deviceScaleFactor: 1,
    });
    await page.goto(url.pathToFileURL(mermaidCliIndexHtml).href);
    return await page.evaluate(
      (payload) => {
        const SVG_NS = 'http://www.w3.org/2000/svg';
        const XHTML_NS = 'http://www.w3.org/1999/xhtml';
        const host = document.getElementById('container') || document.body;
        host.innerHTML = '';

        const svg = document.createElementNS(SVG_NS, 'svg');
        svg.setAttribute('xmlns', SVG_NS);
        svg.setAttribute('width', `${payload.maxWidthPx * 10}`);
        svg.setAttribute('height', `${payload.maxWidthPx * 10}`);
        svg.style.position = 'absolute';
        svg.style.top = '0';
        svg.style.left = '0';
        svg.style.visibility = 'hidden';

        const fo = document.createElementNS(SVG_NS, 'foreignObject');
        fo.setAttribute('width', `${payload.maxWidthPx * 10}`);
        fo.setAttribute('height', `${payload.maxWidthPx * 10}`);
        svg.appendChild(fo);

        const div = document.createElementNS(XHTML_NS, 'div');
        if (payload.styleCss) {
          div.setAttribute('style', payload.styleCss);
        }
        div.style.display = 'table-cell';
        div.style.whiteSpace = 'nowrap';
        div.style.lineHeight = '1.5';
        div.style.maxWidth = `${payload.maxWidthPx}px`;
        div.style.textAlign = 'center';
        div.setAttribute('xmlns', XHTML_NS);
        fo.appendChild(div);

        const span = document.createElementNS(XHTML_NS, 'span');
        span.className = 'nodeLabel';
        if (payload.styleCss) {
          span.setAttribute('style', payload.styleCss);
        }
        span.innerHTML = payload.html;
        div.appendChild(span);
        host.appendChild(svg);

        let bbox = div.getBoundingClientRect();
        fo.setAttribute('width', `${bbox.width}`);
        fo.setAttribute('height', `${bbox.height}`);
        if (bbox.width === payload.maxWidthPx) {
          div.style.display = 'table';
          div.style.whiteSpace = 'break-spaces';
          div.style.width = `${payload.maxWidthPx}px`;
          bbox = div.getBoundingClientRect();
          fo.setAttribute('width', `${bbox.width}`);
          fo.setAttribute('height', `${bbox.height}`);
        }

        return {
          width: bbox.width,
          height: bbox.height,
        };
      },
      {
        html,
        styleCss,
        maxWidthPx,
      }
    );
  } finally {
    await browser.close();
  }
}

async function main() {
  const mode = process.argv[2];
  const raw = fs.readFileSync(0, 'utf8');
  const payload = raw ? JSON.parse(raw) : {};
  const html = renderKatexHtml(payload.text, payload.config || {});

  if (mode === 'render') {
    process.stdout.write(JSON.stringify({ html }));
    return;
  }

  if (mode === 'probe') {
    const result = await measureHtml(
      html,
      payload.styleCss || '',
      Number.isFinite(payload.maxWidthPx) ? payload.maxWidthPx : 200
    );
    process.stdout.write(JSON.stringify({ html, ...result }));
    return;
  }

  throw new Error(`unknown mode: ${mode}`);
}

main().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
