import { currentRuntimeState, encodeOptions, getMerman } from "./runtime-core.js";
import { type MermanRuntimeState, withMermanRuntimeState } from "./runtime-state.js";
import { encodeJsonInput } from "./json-input.js";
import type {
  BrowserRenderSession,
  HostTextMeasurer,
  HostTextMeasurerSvgBindingOptions,
  RenderSessionOptions,
  SvgBindingOptions,
  SvgRenderResult,
  WasmRenderSessionBinding,
} from "./public-types.js";

/** Admit loaded packs once; every output uses the same immutable scoped services. */
export function createRenderSession(options: RenderSessionOptions = {}): BrowserRenderSession {
  const state = currentRuntimeState();
  const RenderSession = getMerman().RenderSession;
  if (!RenderSession) throw new Error("Merman createRenderSession() is not available in this artifact.");
  const packs = options.iconPacks ?? [];
  const native = new RenderSession(encodeJsonInput(packs, "icon packs"));
  return new BrowserRenderSessionImpl(native, state);
}

class BrowserRenderSessionImpl implements BrowserRenderSession {
  private native: WasmRenderSessionBinding | null;
  private activeCalls = 0;
  private pendingFree: WasmRenderSessionBinding | null = null;

  constructor(native: WasmRenderSessionBinding, private readonly runtimeState: MermanRuntimeState) {
    this.native = native;
  }

  renderSvg(source: string, options?: SvgBindingOptions | string): string {
    return this.withNative(native => native.renderSvg(source, encodeOptions(options)));
  }
  renderSvgResult(source: string, options?: SvgBindingOptions | string): SvgRenderResult {
    return this.withNative(native => native.renderSvgResult(source, encodeOptions(options)));
  }
  layoutJson(source: string, options?: SvgBindingOptions | string): string {
    return this.withNative(native => native.layoutJson(source, encodeOptions(options)));
  }
  edgeGeometryJson(source: string, options?: SvgBindingOptions | string): string {
    return this.withNative(native => native.edgeGeometryJson(source, encodeOptions(options)));
  }
  renderSvgWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string {
    return this.withNative(native => native.renderSvgWithTextMeasurer(source, encodeOptions(options), measurer));
  }
  renderSvgResultWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): SvgRenderResult {
    return this.withNative(native => native.renderSvgResultWithTextMeasurer(source, encodeOptions(options), measurer));
  }
  layoutJsonWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string {
    return this.withNative(native => native.layoutJsonWithTextMeasurer(source, encodeOptions(options), measurer));
  }
  edgeGeometryJsonWithTextMeasurer(source: string, measurer: HostTextMeasurer, options?: HostTextMeasurerSvgBindingOptions | string): string {
    return this.withNative(native => native.edgeGeometryJsonWithTextMeasurer(source, encodeOptions(options), measurer));
  }

  dispose(): void {
    const native = this.native;
    if (!native) return;
    this.native = null;
    if (this.activeCalls > 0) this.pendingFree = native;
    else withMermanRuntimeState(this.runtimeState, () => native.free());
  }

  private withNative<T>(run: (native: WasmRenderSessionBinding) => T): T {
    const native = this.native;
    if (!native) throw new Error("Merman render session is disposed.");
    this.activeCalls++;
    try {
      return withMermanRuntimeState(this.runtimeState, () => run(native));
    } finally {
      this.activeCalls--;
      if (this.activeCalls === 0 && this.pendingFree) {
        const retired = this.pendingFree;
        this.pendingFree = null;
        withMermanRuntimeState(this.runtimeState, () => retired.free());
      }
    }
  }
}
