# Icon Registry Constructor Service

Merman exposes Iconify support as an immutable constructor service. It is part of the existing SVG
capability; it is not a separate renderer feature and it does not add filesystem, package-manager,
network, DNS, async-runtime, or CLI dependencies to Rust SDK builds.

## Shared Contract

One registry is built transactionally from one or more borrowed `IconPack` values:

```rust
use merman_bindings_core::{
    BindingEngine, BindingEngineServices, IconPack, build_icon_registry,
};

let logos = std::fs::read("icons.json")?;
let registry = build_icon_registry([IconPack::new(&logos)])?;
let services = BindingEngineServices::new().with_icon_registry(registry.clone());

let first = BindingEngine::from_options_and_services(b"{}", services.clone())?;
let second = BindingEngine::from_options_and_services(b"{}", services)?;
drop(logos); // Construction retained validated state, not the borrowed input buffer.
# Ok::<(), Box<dyn std::error::Error>>(())
```

Acquisition in this example is host code. Native transports receive bytes from their host-specific
constructor records; the shared SDK layer does not open paths or URLs. CLI package/path lookup and
optional HTTP acquisition remain CLI-only behavior.

The binding facade deliberately returns `BindingIconRegistry`, whose renderer field is private.
Bindings must use `build_icon_registry`; they cannot construct a wrapper around an unchecked raw
renderer value or mutate a published registry. Cloning the wrapper shares the immutable parsed
state.

## Input And Lifetime

- Each `IconPack` contains borrowed JSON bytes plus an optional registration-name override.
- The constructor borrows every byte slice only until it returns.
- Success means the registry owns all validated state and the caller may release every source
  buffer.
- Any invalid pack rejects the complete transaction. No partial registry or reusable partially
  populated builder is published.
- Engine construction does not render and does not invoke a host text-measurement callback.

Complete Iconify collections and host-curated subsets use the same path. A complete collection is
supported only when it fits the fixed renderer limits; Merman does not silently slice or sample a
larger collection.

## Fixed Resource Contract

The renderer publishes transport-neutral constructor limits through
`icon-registry` service metadata. The primary ceilings are 16 packs, 16 MiB for one encoded pack,
32 MiB aggregate encoded input, 32,768 direct icons, 32,768 aliases, 65,536 total entries, 256 KiB
for one decoded SVG body, and 32 MiB aggregate retained bodies. JSON structure, identifier length,
alias graph, XML structure, rewrite-plan memory, coordinate magnitude, and total build work have
additional fixed ceilings.

Defaults and hard maxima are identical and `caller_configurable` is false. A transport may expose a
tighter host/CLI acquisition policy, but it must not present a way to loosen the shared constructor.
Resource-limit failures include a stable typed limit ID plus actual and maximum values. Content
failures retain their icon-registry kind and pack index without echoing the input body.

## Render-Time Security

Host-selected pack bytes are still untrusted input. Admission rejects malformed UTF-8/JSON,
duplicate raw JSON keys, invalid Iconify identifiers or geometry, graph cycles/collisions, DTDs,
entities, processing instructions, malformed XML, duplicate IDs, and resource amplification above
the fixed ceilings.

Each insertion performs XML-aware deterministic ID scoping, icon-SVG assembly, sanitization under
the request's effective Mermaid configuration, and post-sanitizer XML validation. Repeated
expansion is charged to the operation work and SVG-byte budgets before the corresponding allocation.

This does not turn parity/readable SVG into a browser-DOM-safe type. Use `SafeInlineSvg` or
`assertSafeSvgForDom()`, CSP, or sandboxing for DOM insertion. Merman performs no icon acquisition
I/O, but a downstream consumer can still load a policy-allowed external reference.

See [Rendering Security](../security/RENDERING_SECURITY.md) and the
[Threat Model](../security/THREAT_MODEL.md) for the destination-specific output boundary.
