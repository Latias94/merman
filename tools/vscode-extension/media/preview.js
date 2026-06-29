(function () {
  const vscode = acquireVsCodeApi();
  const frame = document.querySelector(".frame");
  const canvas = document.querySelector(".canvas");
  const state = {
    zoom: 1,
    fit: true,
  };

  function post(type, payload) {
    vscode.postMessage({ type, ...payload });
  }

  function updateCanvas() {
    if (!canvas || !frame) {
      return;
    }
    frame.dataset.fit = String(state.fit);
    canvas.style.transform = `scale(${state.zoom})`;
    canvas.style.width = `${100 / state.zoom}%`;
  }

  function setZoom(nextZoom) {
    state.fit = false;
    state.zoom = Math.min(4, Math.max(0.25, nextZoom));
    updateCanvas();
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
        setZoom(state.zoom + 0.1);
        break;
      case "zoom-out":
        setZoom(state.zoom - 0.1);
        break;
      case "reset":
        state.zoom = 1;
        state.fit = true;
        updateCanvas();
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
      case "theme":
        if (frame) {
          frame.dataset.theme = target.value;
        }
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

  updateCanvas();
})();
