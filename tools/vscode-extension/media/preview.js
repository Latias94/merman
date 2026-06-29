(function () {
  const vscode = acquireVsCodeApi();
  const frame = document.querySelector(".frame");
  const viewport = document.querySelector(".viewport");
  const stage = document.querySelector(".stage");
  const canvas = document.querySelector(".canvas");
  const zoomValue = document.querySelector("[data-zoom-value]");
  const state = {
    zoom: 1,
    panX: 0,
    panY: 0,
    autoFit: true,
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

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
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
    updateCanvas();
  }

  function resetToActualSize() {
    state.autoFit = false;
    state.zoom = 1;
    state.panX = 0;
    state.panY = 0;
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
    updateCanvas();
  }

  function normalizeSvgSize() {
    const svg = canvas?.querySelector("svg");
    if (!svg) {
      return;
    }
    const viewBox = parseViewBox(svg.getAttribute("viewBox"));
    if (viewBox && (!positiveLength(svg.getAttribute("width")) || !positiveLength(svg.getAttribute("height")))) {
      svg.setAttribute("width", String(viewBox.width));
      svg.setAttribute("height", String(viewBox.height));
      return;
    }
    if (!svg.hasAttribute("viewBox")) {
      const box = tryGetBBox(svg);
      if (box && box.width > 0 && box.height > 0) {
        svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
        svg.setAttribute("width", String(box.width));
        svg.setAttribute("height", String(box.height));
      }
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
        if (frame) {
          frame.dataset.background = target.value;
        }
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

  viewport?.addEventListener("pointermove", (event) => {
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

  viewport?.addEventListener("pointerup", stopDragging);
  viewport?.addEventListener("pointercancel", stopDragging);

  if (viewport && canvas && stage && typeof ResizeObserver !== "undefined") {
    const observer = new ResizeObserver(() => {
      if (state.autoFit) {
        fitToView();
      }
    });
    observer.observe(viewport);
    observer.observe(canvas);
  }

  normalizeSvgSize();
  updateCanvas();
  requestAnimationFrame(fitToView);
})();
