import { currentRuntimeState, encodeOptions } from "./runtime-core.js";
import { withMermanRuntimeState, type MermanRuntimeState } from "./runtime-state.js";
import { createBrowserTextMeasurementSession, prepareSource } from "./runtime-render.js";
import { prepareSvgDomCandidate, publishSvgDomCandidate } from "./svg-dom.js";
import type {
  BrowserPreparedSource, BrowserTextMeasurementSession, MountDiagramOptions,
  MountedDiagram, MountedDiagramStatus, PreparedSourceOutputOptions, SvgRenderResult,
} from "./public-types.js";

const mounts = new WeakMap<Element, MountedDiagram>();
let namespace: string | undefined;
let nextId = 0;
type RetainedSource = { source: string; options: string | undefined; prepared: BrowserPreparedSource };
type MountSettings = {
  prepare: string | undefined;
  output: PreparedSourceOutputOptions;
  adaptation: "none" | "fit-width" | undefined;
  measurement: MountDiagramOptions["textMeasurement"];
  onRender: MountDiagramOptions["onRender"];
  onError: MountDiagramOptions["onError"];
};

function settings(options: MountDiagramOptions): MountSettings {
  const prepare = encodeOptions(options.prepareOptions);
  const output: PreparedSourceOutputOptions = JSON.parse(encodeOptions(options.outputOptions) ?? "{}");
  return {
    prepare, output,
    adaptation: output.layout?.adaptation ?? options.prepareOptions?.layout?.adaptation,
    measurement: options.textMeasurement ?? "deterministic",
    onRender: options.onRender, onError: options.onError,
  };
}

/** A new mount disposes this package's prior controller for the target. */
export function mountDiagram(target: Element, source: string, options: MountDiagramOptions = {}): MountedDiagram {
  mounts.get(target)?.destroy();
  const crypto = target.ownerDocument.defaultView?.crypto ?? globalThis.crypto;
  namespace ??= Array.from(crypto.getRandomValues(new Uint32Array(4)), value => value.toString(16)).join("-");
  const mount = new DiagramMount(target, currentRuntimeState(), `merman-mount-${namespace}-${++nextId}`);
  mounts.set(target, mount);
  mount.update(source, options);
  return mount;
}

class DiagramMount implements MountedDiagram {
  private requestedSource = "";
  private publishedResult: SvgRenderResult | null = null;
  private lastError: unknown = null;
  private state: MountedDiagramStatus = "pending";
  private options = settings({});
  private settingsError: { error: unknown } | null = null;
  private version = 0;
  private rendering = false;
  private candidate: RetainedSource | null = null;
  private lastGood: RetainedSource | null = null;
  private document: Document | null = null;
  private observer: ResizeObserver | null = null;
  private probes: BrowserTextMeasurementSession | null = null;
  private fontListener: (() => void) | null = null;
  private frame: { view: Window; id: number } | null = null;
  private realmVersion = 0;
  private observedWidth: number | null = null;

  constructor(private readonly target: Element, private readonly runtime: MermanRuntimeState, private readonly diagramId: string) {}
  get source(): string { return this.requestedSource; }
  get result(): SvgRenderResult | null { return this.publishedResult; }
  get error(): unknown { return this.lastError; }
  get status(): MountedDiagramStatus { return this.state; }

  update(source: string, options?: MountDiagramOptions): void {
    this.assertLive();
    this.requestedSource = source;
    const version = this.invalidate();
    if (options !== undefined) {
      this.settingsError = null;
      try { this.options = settings(options); }
      catch (error) { this.settingsError = { error }; this.fail(error, version); return; }
    }
    this.bindDocument();
    this.request(version);
  }

  refresh(): void {
    this.assertLive();
    const version = this.invalidate();
    this.bindDocument();
    this.probes?.dispose();
    this.probes = null;
    this.request(version);
  }

  destroy(): void {
    if (this.state === "destroyed") return;
    this.invalidate();
    this.state = "destroyed";
    this.releaseDocument();
    this.candidate?.prepared.dispose();
    if (this.lastGood !== this.candidate) this.lastGood?.prepared.dispose();
    this.candidate = this.lastGood = null;
    if (mounts.get(this.target) === this) mounts.delete(this.target);
  }

  private assertLive(): void {
    if (this.state === "destroyed") throw new Error("Merman mounted diagram is destroyed.");
  }
  private current(version: number): boolean {
    return this.state !== "destroyed" && this.version === version && mounts.get(this.target) === this;
  }
  private invalidate(): number {
    this.version++;
    if (this.frame) this.frame.view.cancelAnimationFrame(this.frame.id);
    this.frame = null;
    return this.version;
  }
  private request(version: number): void {
    if (this.rendering) this.schedule(version);
    else this.render(version);
  }
  private schedule(version = this.version): void {
    if (!this.current(version) || this.frame) return;
    const view = this.document?.defaultView;
    if (!view) return;
    this.frame = { view, id: view.requestAnimationFrame(() => {
      this.frame = null;
      if (this.current(version)) this.render(version);
    }) };
  }

  private bindDocument(): void {
    const document = this.target.ownerDocument;
    if (this.document === document) return;
    this.releaseDocument();
    this.document = document;
    const realm = this.realmVersion;
    const view = document.defaultView;
    if (view?.ResizeObserver) {
      this.observer = new view.ResizeObserver(entries => {
        if (this.document !== document || this.realmVersion !== realm || this.target.ownerDocument !== document) return;
        const entry = entries.find(value => value.target === this.target);
        if (!entry) return;
        const width = this.availableWidth();
        if (width === this.observedWidth) return;
        const wasHidden = !this.observedWidth || this.observedWidth <= 0;
        this.observedWidth = width;
        if (this.options.adaptation === "fit-width" || this.state === "pending" || wasHidden) {
          this.schedule(this.invalidate());
        }
      });
      this.observer.observe(this.target);
    }
    const fontsChanged = () => {
      if (this.state === "destroyed" || this.document !== document || this.realmVersion !== realm || this.target.ownerDocument !== document || this.options.measurement === "deterministic") return;
      this.probes?.dispose();
      this.probes = null;
      this.schedule(this.invalidate());
    };
    this.fontListener = fontsChanged;
    document.fonts?.addEventListener("loadingdone", fontsChanged);
    document.fonts?.addEventListener("loadingerror", fontsChanged);
    void document.fonts?.ready.then(fontsChanged);
  }

  private releaseDocument(): void {
    this.realmVersion++;
    this.observer?.disconnect();
    this.observer = null;
    if (this.document && this.fontListener) {
      this.document.fonts?.removeEventListener("loadingdone", this.fontListener);
      this.document.fonts?.removeEventListener("loadingerror", this.fontListener);
    }
    this.fontListener = null;
    this.probes?.dispose();
    this.probes = null;
    this.document = null;
    this.observedWidth = null;
  }

  private availableWidth(): number {
    const style = this.document?.defaultView?.getComputedStyle(this.target);
    if (style?.visibility === "hidden" || style?.visibility === "collapse") return 0;
    return this.target.clientWidth - parseFloat(style?.paddingLeft || "0") - parseFloat(style?.paddingRight || "0");
  }

  private render(version: number): void {
    if (!this.current(version) || this.target.ownerDocument !== this.document) return;
    if (this.settingsError) { this.fail(this.settingsError.error, version); return; }
    const width = this.availableWidth();
    this.observedWidth = width;
    if (!this.target.isConnected || !Number.isFinite(width) || width <= 0) {
      this.state = "pending";
      return;
    }
    this.rendering = true;
    this.state = "rendering";
    let result: SvgRenderResult | null = null;
    try {
      const source = this.retainedSource();
      const output: PreparedSourceOutputOptions = {
        ...this.options.output, svg: { ...this.options.output.svg, diagram_id: this.diagramId },
      };
      if (this.options.adaptation === "fit-width") output.layout = { ...output.layout, container_width: width };
      const measurement = this.options.measurement;
      if (measurement === "browser") this.probes ??= createBrowserTextMeasurementSession(this.document!);
      const measurer = measurement === "browser" ? this.probes!.measure : typeof measurement === "function" ? measurement : null;
      result = measurer
        ? source.prepared.renderSvgResultWithTextMeasurer(measurer, output)
        : source.prepared.renderSvgResult(output);
      if (!this.current(version) || this.target.ownerDocument !== this.document) return;
      const candidate = prepareSvgDomCandidate(this.document!, result.svg);
      if (!this.current(version) || this.target.ownerDocument !== this.document) return;
      publishSvgDomCandidate(this.target, candidate);
      if (!this.current(version)) return;
      const previous = this.lastGood;
      this.lastGood = source;
      if (previous !== source) previous?.prepared.dispose();
      this.publishedResult = result;
      this.lastError = null;
      this.state = "ready";
    } catch (error) {
      this.fail(error, version);
      return;
    } finally {
      this.rendering = false;
    }
    if (result && this.current(version)) {
      try { this.options.onRender?.(result, this); }
      catch (error) { this.fail(error, version); }
    }
  }

  private retainedSource(): RetainedSource {
    const matches = (value: RetainedSource | null) => value?.source === this.requestedSource && value.options === this.options.prepare;
    if (matches(this.candidate)) return this.candidate!;
    if (this.candidate !== this.lastGood) this.candidate?.prepared.dispose();
    this.candidate = null;
    if (matches(this.lastGood)) this.candidate = this.lastGood;
    else this.candidate = {
      source: this.requestedSource, options: this.options.prepare,
      prepared: withMermanRuntimeState(this.runtime, () => prepareSource(this.requestedSource, this.options.prepare)),
    };
    return this.candidate!;
  }

  private fail(error: unknown, version: number): void {
    if (!this.current(version)) return;
    this.lastError = error;
    this.state = "failed";
    try { this.options.onError?.(error, this); }
    catch (callbackError) { if (this.current(version)) this.lastError = callbackError; }
  }
}
