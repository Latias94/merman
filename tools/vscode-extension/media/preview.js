(function () {
  const vscode = acquireVsCodeApi();
  const frame = document.querySelector(".frame");
  const viewport = document.querySelector(".viewport");
  const stage = document.querySelector(".stage");
  const canvas = document.querySelector("[data-preview-canvas]");
  const zoomValue = document.querySelector("[data-zoom-value]");
  const diagnosticsElement = document.querySelector("[data-preview-diagnostics]");
  const statusElement = document.querySelector("[data-preview-status]");
  const emptyElement = document.querySelector("[data-preview-empty]");
  const sourceBarElement = document.querySelector("[data-preview-sourcebar]");
  const sourceListElement = document.querySelector("[data-preview-source-list]");
  const displayModeElement = document.querySelector('[data-action="display-mode"]');
  const themeElement = document.querySelector('[data-action="diagram-theme"]');
  const backgroundElement = document.querySelector('[data-action="background"]');
  const lockElement = document.querySelector("[data-preview-lock]");
  const outputControlsElement = document.querySelector("[data-preview-output-controls]");
  const outputActionElements = [
    {
      element: document.querySelector('[data-action="copy-svg"]'),
      readyTitle: "Copy rendered SVG",
      staleTitle: "Copy SVG is disabled while the preview shows the last successful render",
    },
    {
      element: document.querySelector('[data-action="export-svg"]'),
      readyTitle: "Export SVG",
      staleTitle: "Export SVG is disabled while the preview shows the last successful render",
    },
    {
      element: document.querySelector('[data-action="export-png"]'),
      readyTitle: "Export PNG",
      staleTitle: "Export PNG is disabled while the preview shows the last successful render",
    },
  ];
  const persisted = vscode.getState?.() || {};
  const state = {
    zoom: finiteNumber(persisted.zoom, 1),
    panX: finiteNumber(persisted.panX, 0),
    panY: finiteNumber(persisted.panY, 0),
    autoFit: persisted.autoFit !== false,
    background: typeof persisted.background === "string" ? persisted.background : "paper",
    displayMode: typeof persisted.displayMode === "string" ? persisted.displayMode : "svg",
    locked: persisted.locked === true,
    sourceKey: undefined,
    sourceKeyId: typeof persisted.sourceKeyId === "string" ? persisted.sourceKeyId : undefined,
    sourceLocationKey:
      typeof persisted.sourceLocationKey === "string" ? persisted.sourceLocationKey : undefined,
    sourceIdentityKey:
      typeof persisted.sourceIdentityKey === "string" ? persisted.sourceIdentityKey : undefined,
    latestRequestId: 0,
    activeRequestId: undefined,
    dragging: false,
    pointerId: undefined,
    lastClientX: 0,
    lastClientY: 0,
  };
  const minZoom = 0.1;
  const maxZoom = 8;
  let svgRenderHost;
  let svgRenderRoot;

  function post(type, payload = {}) {
    vscode.postMessage({ type, ...payload });
  }

  function persistState() {
    vscode.setState?.({
      zoom: state.zoom,
      panX: state.panX,
      panY: state.panY,
      autoFit: state.autoFit,
      background: state.background,
      displayMode: state.displayMode,
      locked: state.locked,
      sourceKeyId: state.sourceKeyId,
      sourceLocationKey: state.sourceLocationKey,
      sourceIdentityKey: state.sourceIdentityKey,
    });
  }

  function finiteNumber(value, fallback) {
    return Number.isFinite(value) ? value : fallback;
  }

  function validRequestId(value) {
    return Number.isSafeInteger(value) && value >= 0;
  }

  function observeRenderStarted(requestId) {
    if (!validRequestId(requestId) || requestId < state.latestRequestId) {
      return false;
    }
    state.latestRequestId = requestId;
    state.activeRequestId = requestId;
    return true;
  }

  function observeRenderInvalidated(requestId) {
    if (!validRequestId(requestId)) {
      state.latestRequestId += 1;
    } else if (requestId < state.latestRequestId) {
      return false;
    } else {
      state.latestRequestId = requestId;
    }
    state.activeRequestId = undefined;
    return true;
  }

  function shouldAcceptRenderTerminal(requestId) {
    if (!validRequestId(requestId) || requestId < state.latestRequestId) {
      return false;
    }
    if (state.activeRequestId !== undefined && state.activeRequestId !== requestId) {
      return false;
    }
    state.latestRequestId = requestId;
    return true;
  }

  function finishRenderTerminal(requestId) {
    if (state.activeRequestId === requestId) {
      state.activeRequestId = undefined;
    }
  }

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function sourceKeyId(snapshot) {
    if (!snapshot?.sourceKey) {
      return undefined;
    }
    const key = snapshot.sourceKey;
    return [
      key.documentUri,
      key.sourceId,
      key.sourceHash,
      key.diagramTheme,
      key.displayMode,
      key.background,
    ].join("\u0000");
  }

  function sourceIdentityKey(snapshot) {
    if (!snapshot?.sourceKey) {
      return undefined;
    }
    const key = snapshot.sourceKey;
    return [key.documentUri, key.sourceId, key.sourceHash].join("\u0000");
  }

  function sourceLocationKey(snapshot) {
    if (!snapshot?.sourceKey) {
      return undefined;
    }
    const key = snapshot.sourceKey;
    return [key.documentUri, key.sourceId].join("\u0000");
  }

  function isDifferentSourceLocation(snapshot) {
    const nextLocationKey = sourceLocationKey(snapshot);
    return (
      state.sourceLocationKey !== undefined &&
      nextLocationKey !== undefined &&
      state.sourceLocationKey !== nextLocationKey
    );
  }

  function updateCanvas() {
    if (!frame) {
      return;
    }
    frame.style.setProperty("--preview-zoom", String(state.zoom));
    frame.style.setProperty("--preview-pan-x", `${state.panX}px`);
    frame.style.setProperty("--preview-pan-y", `${state.panY}px`);
    if (zoomValue) {
      zoomValue.textContent = `${Math.round(state.zoom * 100)}%`;
    }
    persistState();
  }

  function setZoom(nextZoom, anchor) {
    if (state.displayMode !== "svg") {
      return;
    }
    const previousZoom = state.zoom;
    const zoom = clamp(nextZoom, minZoom, maxZoom);
    if (!Number.isFinite(zoom) || zoom === previousZoom) {
      return;
    }
    state.autoFit = false;
    if (anchor && viewport) {
      const rect = viewport.getBoundingClientRect();
      const anchorX = anchor.clientX - rect.left - rect.width / 2;
      const anchorY = anchor.clientY - rect.top - rect.height / 2;
      const ratio = zoom / previousZoom;
      state.panX = anchorX - (anchorX - state.panX) * ratio;
      state.panY = anchorY - (anchorY - state.panY) * ratio;
    }
    state.zoom = zoom;
    applyVectorZoom();
    updateCanvas();
  }

  function resetToActualSize() {
    if (state.displayMode !== "svg") {
      return;
    }
    state.autoFit = false;
    state.zoom = 1;
    state.panX = 0;
    state.panY = 0;
    applyVectorZoom();
    updateCanvas();
  }

  function resetViewportForNewContent() {
    state.autoFit = true;
    state.zoom = 1;
    state.panX = 0;
    state.panY = 0;
  }

  function measureCanvas() {
    if (!canvas) {
      return undefined;
    }
    return {
      width: Math.max(canvas.offsetWidth, 1),
      height: Math.max(canvas.offsetHeight, 1),
    };
  }

  function clearRenderedContent() {
    if (canvas) {
      canvas.replaceChildren();
    }
    svgRenderHost = undefined;
    svgRenderRoot = undefined;
  }

  function svgContentRoot() {
    if (!canvas) {
      return undefined;
    }
    svgRenderHost = document.createElement("div");
    svgRenderHost.className = "svg-preview-host";
    svgRenderRoot = svgRenderHost.attachShadow({ mode: "open" });
    canvas.replaceChildren(svgRenderHost);
    return svgRenderRoot;
  }

  function renderedSvg() {
    return svgRenderRoot?.querySelector("svg");
  }

  function fitToView() {
    if (state.displayMode !== "svg" || !viewport || !canvas) {
      return;
    }
    normalizeSvgSize();
    applyVectorZoom(1);
    const availableWidth = Math.max(viewport.clientWidth - 48, 1);
    const availableHeight = Math.max(viewport.clientHeight - 48, 1);
    const measured = measureCanvas();
    if (!measured) {
      return;
    }
    state.autoFit = true;
    state.zoom = clamp(
      Math.min(availableWidth / measured.width, availableHeight / measured.height, 1),
      minZoom,
      maxZoom,
    );
    state.panX = 0;
    state.panY = 0;
    applyVectorZoom();
    updateCanvas();
  }

  function normalizeSvgSize() {
    const svg = renderedSvg();
    if (!svg) {
      return;
    }
    const viewBox = parseViewBox(svg.getAttribute("viewBox"));
    if (viewBox) {
      svg.dataset.baseWidth = String(viewBox.width);
      svg.dataset.baseHeight = String(viewBox.height);
      if (!positiveLength(svg.getAttribute("width")) || !positiveLength(svg.getAttribute("height"))) {
        svg.setAttribute("width", String(viewBox.width));
        svg.setAttribute("height", String(viewBox.height));
      }
      return;
    }
    if (!svg.hasAttribute("viewBox")) {
      const box = tryGetBBox(svg);
      if (box && box.width > 0 && box.height > 0) {
        svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
        svg.setAttribute("width", String(box.width));
        svg.setAttribute("height", String(box.height));
        svg.dataset.baseWidth = String(box.width);
        svg.dataset.baseHeight = String(box.height);
      }
    }
  }

  function applyVectorZoom(zoomOverride) {
    const svg = renderedSvg();
    if (!svg) {
      return;
    }
    const baseWidth = Number.parseFloat(svg.dataset.baseWidth || svg.getAttribute("width") || "0");
    const baseHeight = Number.parseFloat(svg.dataset.baseHeight || svg.getAttribute("height") || "0");
    if (baseWidth > 0 && baseHeight > 0) {
      const zoom = Number.isFinite(zoomOverride) ? zoomOverride : state.zoom;
      svg.setAttribute("width", String(baseWidth * zoom));
      svg.setAttribute("height", String(baseHeight * zoom));
    }
  }

  function parseViewBox(value) {
    if (!value) {
      return undefined;
    }
    const parts = value.trim().split(/[\s,]+/).map(Number);
    if (parts.length !== 4 || parts.some((part) => !Number.isFinite(part))) {
      return undefined;
    }
    const [x, y, width, height] = parts;
    if (width <= 0 || height <= 0) {
      return undefined;
    }
    return { x, y, width, height };
  }

  function positiveLength(value) {
    if (!value) {
      return false;
    }
    return Number.parseFloat(value) > 0;
  }

  function tryGetBBox(svg) {
    try {
      return svg.getBBox();
    } catch {
      return undefined;
    }
  }

  function previewModeLabel(mode) {
    switch (mode) {
      case "ascii":
        return "ASCII";
      case "unicode":
        return "Unicode";
      case "svg":
      default:
        return "SVG";
    }
  }

  function zoomFromButton(delta) {
    if (!viewport) {
      setZoom(state.zoom + delta);
      return;
    }
    const rect = viewport.getBoundingClientRect();
    setZoom(state.zoom + delta, {
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + rect.height / 2,
    });
  }

  function copySvg() {
    const svg = renderedSvg();
    if (!svg || !state.sourceKey) {
      return;
    }
    post("copySvg", { svg: serializedSourceSvg(svg), sourceKey: state.sourceKey });
  }

  function serializedSourceSvg(svg) {
    const copy = svg.cloneNode(true);
    if (!(copy instanceof SVGElement)) {
      return svg.outerHTML;
    }
    const baseWidth = svg.dataset.baseWidth;
    const baseHeight = svg.dataset.baseHeight;
    if (baseWidth && baseHeight) {
      copy.setAttribute("width", baseWidth);
      copy.setAttribute("height", baseHeight);
    }
    delete copy.dataset.baseWidth;
    delete copy.dataset.baseHeight;
    return copy.outerHTML;
  }

  function setText(element, text) {
    if (element) {
      element.textContent = text || "";
    }
  }

  function patchSnapshot(snapshot) {
    if (!snapshot) {
      return;
    }
    patchSourceList(snapshot);
    patchSettings(snapshot);
    patchDiagnostics(snapshot.diagnostics);
  }

  function patchSourceList(snapshot) {
    if (!(sourceListElement instanceof HTMLSelectElement)) {
      return;
    }
    const sources = Array.isArray(snapshot.sources) ? snapshot.sources : [];
    sourceListElement.replaceChildren(
      ...sources.map((source) => {
        const option = document.createElement("option");
        option.value = source.sourceId;
        option.textContent = source.subtitle || source.title || source.sourceId;
        option.selected = source.sourceId === snapshot.sourceId;
        return option;
      }),
    );
    if (sourceBarElement) {
      const hasMultipleMarkdownSources =
        sources.length > 1 && sources.some((source) => source.kind === "markdown-fence");
      sourceBarElement.hidden = !hasMultipleMarkdownSources;
      viewport?.classList.toggle("has-sourcebar", hasMultipleMarkdownSources);
    }
  }

  function patchSettings(snapshot) {
    state.displayMode = snapshot.displayMode || "svg";
    state.background = snapshot.background || state.background;
    state.locked = snapshot.locked === true;
    if (frame) {
      frame.dataset.displayMode = state.displayMode;
      frame.dataset.background = state.background;
      frame.dataset.locked = state.locked ? "true" : "false";
    }
    if (displayModeElement instanceof HTMLSelectElement) {
      displayModeElement.value = state.displayMode;
    }
    if (themeElement instanceof HTMLSelectElement && snapshot.diagramTheme) {
      themeElement.value = snapshot.diagramTheme;
    }
    if (backgroundElement instanceof HTMLSelectElement) {
      backgroundElement.value = state.background;
    }
    if (outputControlsElement instanceof HTMLElement) {
      outputControlsElement.hidden = state.displayMode !== "svg";
    }
    updateLockControl(true);
    persistState();
  }

  function updateLockControl(enabled) {
    if (!(lockElement instanceof HTMLButtonElement)) {
      return;
    }
    lockElement.textContent = state.locked ? "Locked" : "Follow";
    lockElement.title = enabled
      ? state.locked
        ? "Unlock preview so it follows the active source"
        : "Lock preview to the current source"
      : "Open a Mermaid preview before locking it to a source";
    lockElement.disabled = !enabled;
    lockElement.setAttribute("aria-pressed", state.locked ? "true" : "false");
  }

  function patchDiagnostics(diagnostics) {
    if (!diagnosticsElement) {
      return;
    }
    if (!diagnostics || Number(diagnostics.totalCount) <= 0) {
      diagnosticsElement.hidden = true;
      diagnosticsElement.replaceChildren();
      return;
    }

    diagnosticsElement.hidden = false;
    diagnosticsElement.replaceChildren();
    const summary = document.createElement("button");
    summary.type = "button";
    summary.className = "diagnostics-summary";
    summary.textContent = diagnosticsSummaryText(diagnostics);
    if (diagnostics.firstTarget) {
      summary.dataset.action = "diagnostic";
      summary.title = "Open first diagnostic location in editor";
    } else {
      summary.disabled = true;
    }
    diagnosticsElement.appendChild(summary);
  }

  function diagnosticsSummaryText(diagnostics) {
    if (diagnostics.totalCount > 0) {
      return diagnostics.summary;
    }
    return diagnostics.summary;
  }

  function showStatus(text, kind) {
    if (!statusElement) {
      return;
    }
    statusElement.hidden = false;
    statusElement.dataset.kind = kind || "info";
    statusElement.textContent = text;
  }

  function showRenderFailure(error, stale) {
    const text = stale
      ? `Render failed. Showing last successful preview.\n${error || "Render failed"}`
      : error || "Render failed";
    showStatus(text, "error");
    setRenderState(stale ? "stale" : "error");
  }

  function hideStatus() {
    if (statusElement) {
      statusElement.hidden = true;
      statusElement.textContent = "";
      delete statusElement.dataset.kind;
    }
  }

  function setRenderState(renderState) {
    if (!frame) {
      return;
    }
    frame.dataset.renderState = renderState;
    updateOutputActions(renderState);
  }

  function updateOutputActions(renderState) {
    const disabled = renderState !== "ready";
    for (const { element, readyTitle, staleTitle } of outputActionElements) {
      if (!(element instanceof HTMLButtonElement)) {
        continue;
      }
      element.disabled = disabled;
      element.title = disabled ? staleTitle : readyTitle;
    }
  }

  function hasPreviewContent() {
    return !!canvas && (canvas.children.length > 0 || !!renderedSvg());
  }

  function hideEmpty() {
    if (emptyElement) {
      emptyElement.hidden = true;
    }
  }

  function showEmpty(heading, detail, requestId) {
    if (!observeRenderInvalidated(requestId)) {
      return;
    }
    state.locked = false;
    state.sourceKey = undefined;
    state.sourceKeyId = undefined;
    state.sourceLocationKey = undefined;
    state.sourceIdentityKey = undefined;
    setRenderState("empty");
    if (emptyElement) {
      emptyElement.hidden = false;
      const headingElement = emptyElement.querySelector("h2");
      const detailElement = emptyElement.querySelector("p");
      setText(headingElement, heading);
      setText(detailElement, detail);
    }
    clearRenderedContent();
    if (frame) {
      frame.dataset.locked = "false";
    }
    updateLockControl(false);
    hideStatus();
    persistState();
  }

  function clearPreviewContent(snapshot) {
    clearRenderedContent();
    resetViewportForNewContent();
    state.sourceKey = undefined;
    state.sourceKeyId = sourceKeyId(snapshot);
    state.sourceLocationKey = sourceLocationKey(snapshot);
    state.sourceIdentityKey = sourceIdentityKey(snapshot);
    setRenderState("empty");
    updateCanvas();
  }

  function replacePreviewContent(content, snapshot) {
    if (!canvas) {
      return;
    }
    const nextSourceKeyId = sourceKeyId(snapshot);
    const nextSourceIdentityKey = sourceIdentityKey(snapshot);
    const sourceIdentityChanged =
      state.sourceIdentityKey !== undefined &&
      nextSourceIdentityKey !== undefined &&
      state.sourceIdentityKey !== nextSourceIdentityKey;
    const renderKeyChanged =
      state.sourceKeyId !== undefined &&
      nextSourceKeyId !== undefined &&
      state.sourceKeyId !== nextSourceKeyId;
    const shouldResetViewport = sourceIdentityChanged || renderKeyChanged;
    if (shouldResetViewport) {
      resetViewportForNewContent();
    }
    clearRenderedContent();
    if (snapshot?.displayMode === "svg") {
      const root = svgContentRoot();
      if (!root) {
        return;
      }
      root.innerHTML = content;
      normalizeSvgSize();
      applyVectorZoom();
    } else {
      const textPreview = document.createElement("pre");
      textPreview.className = "text-preview";
      textPreview.textContent = content || "";
      canvas.appendChild(textPreview);
      state.autoFit = false;
      state.zoom = 1;
      state.panX = 0;
      state.panY = 0;
    }
    state.sourceKeyId = nextSourceKeyId;
    state.sourceKey = snapshot?.sourceKey;
    state.sourceLocationKey = sourceLocationKey(snapshot);
    state.sourceIdentityKey = nextSourceIdentityKey;
    setRenderState("ready");
    updateCanvas();
    if (state.autoFit && snapshot?.displayMode === "svg") {
      requestAnimationFrame(fitToView);
    }
    hideEmpty();
  }

  function handleMessage(message) {
    switch (message.type) {
      case "showEmpty":
        showEmpty(message.heading, message.detail, message.requestId);
        break;
      case "sourceListUpdated":
      case "selectionChanged":
      case "diagnosticsUpdated":
      case "settingsUpdated":
        patchSnapshot(message.snapshot);
        break;
      case "renderStarted":
        if (!observeRenderStarted(message.requestId)) {
          return;
        }
        if (isDifferentSourceLocation(message.snapshot)) {
          clearPreviewContent(message.snapshot);
        }
        setRenderState("loading");
        patchSnapshot(message.snapshot);
        hideEmpty();
        showStatus(
          `Rendering ${previewModeLabel(message.snapshot?.displayMode)} preview: ${message.snapshot?.subtitle || "Mermaid source"}`,
          "loading",
        );
        break;
      case "renderInvalidated":
        if (!observeRenderInvalidated(message.requestId)) {
          return;
        }
        setRenderState("stale");
        showStatus("Source changed. Waiting to refresh preview.", "loading");
        break;
      case "renderSucceeded":
        if (!shouldAcceptRenderTerminal(message.requestId)) {
          return;
        }
        patchSnapshot(message.snapshot);
        replacePreviewContent(message.content, message.snapshot);
        finishRenderTerminal(message.requestId);
        hideStatus();
        break;
      case "renderFailed":
        if (!shouldAcceptRenderTerminal(message.requestId)) {
          return;
        }
        const sourceLocationChanged = isDifferentSourceLocation(message.snapshot);
        if (sourceLocationChanged) {
          clearPreviewContent(message.snapshot);
        }
        patchSnapshot(message.snapshot);
        finishRenderTerminal(message.requestId);
        showRenderFailure(message.error, !sourceLocationChanged && hasPreviewContent());
        break;
    }
  }

  function exportRendered(format) {
    if (!state.sourceKey) {
      return;
    }
    post("exportRendered", { format, sourceKey: state.sourceKey });
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }

    const actionElement = target.closest("[data-action]");
    if (!(actionElement instanceof HTMLElement)) {
      return;
    }
    if (canvas?.contains(actionElement)) {
      return;
    }
    if (actionElement instanceof HTMLButtonElement && actionElement.disabled) {
      return;
    }

    switch (actionElement.dataset.action) {
      case "zoom-in":
        zoomFromButton(0.1);
        break;
      case "zoom-out":
        zoomFromButton(-0.1);
        break;
      case "fit":
        fitToView();
        break;
      case "reset":
        resetToActualSize();
        break;
      case "copy-svg":
        copySvg();
        break;
      case "refresh":
        post("refresh", {});
        break;
      case "show-source":
        post("showSource", {});
        break;
      case "export-svg":
        exportRendered("svg");
        break;
      case "export-png":
        exportRendered("png");
        break;
      case "lock":
        state.locked = !state.locked;
        post("setLocked", { locked: state.locked });
        persistState();
        break;
      case "diagnostic":
        post("revealDiagnostic");
        break;
    }
  });

  document.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLSelectElement)) {
      return;
    }
    if (canvas?.contains(target)) {
      return;
    }

    switch (target.dataset.action) {
      case "display-mode":
        state.displayMode = target.value;
        if (frame) {
          frame.dataset.displayMode = target.value;
        }
        post("setDisplayMode", { mode: target.value });
        persistState();
        break;
      case "diagram-theme":
        post("setDiagramTheme", { theme: target.value });
        break;
      case "background":
        state.background = target.value;
        if (frame) {
          frame.dataset.background = target.value;
        }
        post("setBackground", { background: target.value });
        persistState();
        break;
      case "source":
        post("selectSource", { sourceId: target.value });
        break;
    }
  });

  viewport?.addEventListener("wheel", (event) => {
    const target = event.target;
    if (
      state.displayMode !== "svg" ||
      (target instanceof HTMLElement && target.closest(".text-preview"))
    ) {
      return;
    }
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.001);
    setZoom(state.zoom * factor, {
      clientX: event.clientX,
      clientY: event.clientY,
    });
  }, { passive: false });

  viewport?.addEventListener("pointerdown", (event) => {
    const target = event.target;
    if (
      state.displayMode !== "svg" ||
      event.button !== 0 ||
      target instanceof HTMLButtonElement ||
      target instanceof HTMLSelectElement ||
      (target instanceof HTMLElement && target.closest(".preview-menu, .preview-sourcebar, .text-preview"))
    ) {
      return;
    }
    state.autoFit = false;
    state.dragging = true;
    state.pointerId = event.pointerId;
    state.lastClientX = event.clientX;
    state.lastClientY = event.clientY;
    viewport.classList.add("is-dragging");
    viewport.setPointerCapture(event.pointerId);
  });

  document.addEventListener("pointermove", (event) => {
    if (!state.dragging || state.pointerId !== event.pointerId) {
      return;
    }
    state.panX += event.clientX - state.lastClientX;
    state.panY += event.clientY - state.lastClientY;
    state.lastClientX = event.clientX;
    state.lastClientY = event.clientY;
    updateCanvas();
  });

  function stopDragging(event) {
    if (state.pointerId !== event.pointerId) {
      return;
    }
    state.dragging = false;
    state.pointerId = undefined;
    viewport?.classList.remove("is-dragging");
    if (viewport?.hasPointerCapture(event.pointerId)) {
      viewport.releasePointerCapture(event.pointerId);
    }
  }

  document.addEventListener("pointerup", stopDragging);
  document.addEventListener("pointercancel", stopDragging);
  window.addEventListener("message", (event) => {
    handleMessage(event.data);
  });

  if (frame) {
    frame.dataset.background = state.background;
    frame.dataset.displayMode = state.displayMode;
  }
  if (displayModeElement instanceof HTMLSelectElement) {
    displayModeElement.value = state.displayMode;
  }
  if (backgroundElement instanceof HTMLSelectElement) {
    backgroundElement.value = state.background;
  }
  if (lockElement instanceof HTMLButtonElement) {
    updateLockControl(false);
  }
  setRenderState("empty");

  if (viewport && canvas && stage && typeof ResizeObserver !== "undefined") {
    const observer = new ResizeObserver(() => {
      if (state.autoFit) {
        fitToView();
      }
    });
    observer.observe(viewport);
    observer.observe(canvas);
  }

  updateCanvas();
  post("ready", {});
})();
