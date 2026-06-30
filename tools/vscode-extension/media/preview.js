(function () {
  const vscode = acquireVsCodeApi();
  const frame = document.querySelector(".frame");
  const viewport = document.querySelector(".viewport");
  const stage = document.querySelector(".stage");
  const canvas = document.querySelector("[data-preview-canvas]");
  const zoomValue = document.querySelector("[data-zoom-value]");
  const titleElement = document.querySelector("[data-preview-title]");
  const subtitleElement = document.querySelector("[data-preview-subtitle]");
  const diagnosticsElement = document.querySelector("[data-preview-diagnostics]");
  const statusElement = document.querySelector("[data-preview-status]");
  const emptyElement = document.querySelector("[data-preview-empty]");
  const sourceListElement = document.querySelector("[data-preview-source-list]");
  const themeElement = document.querySelector('[data-action="diagram-theme"]');
  const backgroundElement = document.querySelector('[data-action="background"]');
  const pinElement = document.querySelector('[data-action="pin"]');
  const persisted = vscode.getState?.() || {};
  const state = {
    zoom: finiteNumber(persisted.zoom, 1),
    panX: finiteNumber(persisted.panX, 0),
    panY: finiteNumber(persisted.panY, 0),
    autoFit: persisted.autoFit !== false,
    background: typeof persisted.background === "string" ? persisted.background : "transparent",
    sourceKeyId: typeof persisted.sourceKeyId === "string" ? persisted.sourceKeyId : undefined,
    sourceIdentityKey:
      typeof persisted.sourceIdentityKey === "string" ? persisted.sourceIdentityKey : undefined,
    activeRequestId: undefined,
    dragging: false,
    pointerId: undefined,
    lastClientX: 0,
    lastClientY: 0,
  };
  const minZoom = 0.1;
  const maxZoom = 8;

  function post(type, payload) {
    vscode.postMessage({ type, ...payload });
  }

  function persistState() {
    vscode.setState?.({
      zoom: state.zoom,
      panX: state.panX,
      panY: state.panY,
      autoFit: state.autoFit,
      background: state.background,
      sourceKeyId: state.sourceKeyId,
      sourceIdentityKey: state.sourceIdentityKey,
    });
  }

  function finiteNumber(value, fallback) {
    return Number.isFinite(value) ? value : fallback;
  }

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function sourceKeyId(snapshot) {
    if (!snapshot?.sourceKey) {
      return undefined;
    }
    const key = snapshot.sourceKey;
    return [key.documentUri, key.sourceId, key.sourceHash, key.diagramTheme].join("\u0000");
  }

  function sourceIdentityKey(snapshot) {
    if (!snapshot?.sourceKey) {
      return undefined;
    }
    const key = snapshot.sourceKey;
    return [key.documentUri, key.sourceId, key.sourceHash].join("\u0000");
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
    state.autoFit = false;
    state.zoom = 1;
    state.panX = 0;
    state.panY = 0;
    applyVectorZoom();
    updateCanvas();
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

  function fitToView() {
    if (!viewport || !canvas) {
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
    const svg = canvas?.querySelector("svg");
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
    const svg = canvas?.querySelector("svg");
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
    const svg = canvas?.querySelector("svg");
    if (!svg) {
      return;
    }
    post("copySvg", { svg: svg.outerHTML });
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
    setText(titleElement, snapshot.title || "Merman Preview");
    setText(subtitleElement, snapshot.subtitle || "");
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
    sourceListElement.hidden = sources.length <= 1;
  }

  function patchSettings(snapshot) {
    if (themeElement instanceof HTMLSelectElement && snapshot.diagramTheme) {
      themeElement.value = snapshot.diagramTheme;
    }
    if (pinElement instanceof HTMLButtonElement) {
      pinElement.setAttribute("aria-pressed", snapshot.pinned ? "true" : "false");
      pinElement.textContent = snapshot.pinned ? "Pinned" : "Pin";
    }
  }

  function patchDiagnostics(diagnostics) {
    if (!diagnosticsElement) {
      return;
    }
    if (!diagnostics) {
      diagnosticsElement.hidden = true;
      diagnosticsElement.replaceChildren();
      return;
    }

    diagnosticsElement.hidden = false;
    diagnosticsElement.replaceChildren();
    const summary = document.createElement("p");
    summary.className = "diagnostics-summary";
    summary.textContent = diagnosticsSummaryText(diagnostics);
    diagnosticsElement.appendChild(summary);

    if (!Array.isArray(diagnostics.items) || diagnostics.items.length === 0) {
      return;
    }

    const list = document.createElement("ol");
    list.className = "diagnostics-list";
    for (const item of diagnostics.items) {
      list.appendChild(renderDiagnosticItem(item));
    }
    diagnosticsElement.appendChild(list);
  }

  function diagnosticsSummaryText(diagnostics) {
    if (diagnostics.totalCount > diagnostics.visibleCount) {
      return `${diagnostics.summary}. Showing first ${diagnostics.visibleCount} of ${diagnostics.totalCount}.`;
    }
    if (diagnostics.totalCount > 0) {
      return `${diagnostics.summary}. Showing ${diagnostics.totalCount}.`;
    }
    return `${diagnostics.summary}. No issues in the active preview range.`;
  }

  function renderDiagnosticItem(item) {
    const listItem = document.createElement("li");
    listItem.className = "diagnostic-item";
    listItem.dataset.severity = item.severityKey;

    const button = document.createElement("button");
    button.type = "button";
    button.className = "diagnostic-button";
    button.dataset.action = "diagnostic";
    button.dataset.target = JSON.stringify(item.target);
    button.title = "Open diagnostic location in editor";

    const header = document.createElement("p");
    header.className = "diagnostic-header";
    for (const part of [
      ["diagnostic-severity", item.severityLabel],
      ["diagnostic-location", `Ln ${item.line}, Col ${item.column}`],
      ["diagnostic-source", [item.source, item.code].filter(Boolean).join(": ")],
    ]) {
      if (!part[1]) {
        continue;
      }
      const span = document.createElement("span");
      span.className = part[0];
      span.textContent = part[1];
      header.appendChild(span);
    }
    button.appendChild(header);

    const message = document.createElement("p");
    message.className = "diagnostic-message";
    message.textContent = item.message || "";
    button.appendChild(message);
    listItem.appendChild(button);

    if (item.hasQuickFixes) {
      const actions = document.createElement("p");
      actions.className = "diagnostic-actions";
      const fix = document.createElement("button");
      fix.type = "button";
      fix.className = "diagnostic-action";
      fix.dataset.action = "quick-fix";
      fix.dataset.target = JSON.stringify(item.target);
      fix.title = "Request available quick fixes";
      fix.textContent = "Quick Fixes";
      actions.appendChild(fix);
      listItem.appendChild(actions);
    }

    return listItem;
  }

  function showStatus(text, kind) {
    if (!statusElement) {
      return;
    }
    statusElement.hidden = false;
    statusElement.dataset.kind = kind || "info";
    statusElement.textContent = text;
  }

  function hideStatus() {
    if (statusElement) {
      statusElement.hidden = true;
      statusElement.textContent = "";
      delete statusElement.dataset.kind;
    }
  }

  function hideEmpty() {
    if (emptyElement) {
      emptyElement.hidden = true;
    }
  }

  function showEmpty(heading, detail) {
    if (emptyElement) {
      emptyElement.hidden = false;
      const headingElement = emptyElement.querySelector("h2");
      const detailElement = emptyElement.querySelector("p");
      setText(headingElement, heading);
      setText(detailElement, detail);
    }
    if (canvas) {
      canvas.replaceChildren();
    }
    hideStatus();
  }

  function replaceSvg(svg, snapshot) {
    if (!canvas) {
      return;
    }
    const nextSourceKeyId = sourceKeyId(snapshot);
    const nextSourceIdentityKey = sourceIdentityKey(snapshot);
    const shouldResetViewport =
      state.sourceIdentityKey !== undefined &&
      nextSourceIdentityKey !== undefined &&
      state.sourceIdentityKey !== nextSourceIdentityKey;
    if (shouldResetViewport) {
      state.autoFit = true;
      state.zoom = 1;
      state.panX = 0;
      state.panY = 0;
    }
    canvas.innerHTML = svg;
    state.sourceKeyId = nextSourceKeyId;
    state.sourceIdentityKey = nextSourceIdentityKey;
    normalizeSvgSize();
    applyVectorZoom();
    updateCanvas();
    if (state.autoFit) {
      requestAnimationFrame(fitToView);
    }
    hideEmpty();
  }

  function handleMessage(message) {
    switch (message.type) {
      case "showEmpty":
        showEmpty(message.heading, message.detail);
        break;
      case "sourceListUpdated":
      case "selectionChanged":
      case "diagnosticsUpdated":
      case "settingsUpdated":
        patchSnapshot(message.snapshot);
        break;
      case "renderStarted":
        state.activeRequestId = message.requestId;
        patchSnapshot(message.snapshot);
        hideEmpty();
        showStatus(`Rendering preview: ${message.snapshot?.subtitle || "Mermaid source"}`, "loading");
        break;
      case "renderSucceeded":
        if (state.activeRequestId !== undefined && state.activeRequestId !== message.requestId) {
          return;
        }
        patchSnapshot(message.snapshot);
        replaceSvg(message.svg, message.snapshot);
        state.activeRequestId = undefined;
        hideStatus();
        break;
      case "renderFailed":
        if (state.activeRequestId !== undefined && state.activeRequestId !== message.requestId) {
          return;
        }
        patchSnapshot(message.snapshot);
        state.activeRequestId = undefined;
        showStatus(message.error || "Render failed", "error");
        break;
    }
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
      case "pin":
        post("togglePin", {});
        break;
      case "diagnostic":
        post("revealDiagnostic", { target: actionElement.dataset.target });
        break;
      case "quick-fix":
        post("showDiagnosticFixes", { target: actionElement.dataset.target });
        break;
    }
  });

  document.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLSelectElement)) {
      return;
    }

    switch (target.dataset.action) {
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
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.001);
    setZoom(state.zoom * factor, {
      clientX: event.clientX,
      clientY: event.clientY,
    });
  }, { passive: false });

  viewport?.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.target instanceof HTMLButtonElement || event.target instanceof HTMLSelectElement) {
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
  }
  if (backgroundElement instanceof HTMLSelectElement) {
    backgroundElement.value = state.background;
  }

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
