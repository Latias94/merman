#import "context.typ": typst-layout
#import "image.typ": result-image, svg-bytes-or-panic
#import "options.typ": config-with-context-width, context-host-theme, options-bytes, render-config
#import "plugin.typ": merman-plugin
#import "source.typ": source-text-value

#let render-svg-result-with-config(source, config) = {
  let source-text = source-text-value(source)
  json(merman-plugin.render_svg_json(bytes(source-text), options-bytes(config.binding_options)))
}

#let render-svg-result(source, ..args) = {
  render-svg-result-with-config(source, render-config(..args))
}

#let render-svg-bytes(..args) = {
  svg-bytes-or-panic(render-svg-result(..args))
}

#let validate-payload-with-config(source, config) = {
  let source-text = source-text-value(source)
  json(merman-plugin.validate_json(bytes(source-text), options-bytes(config.binding_options)))
}

#let validate-payload(source, ..args) = {
  validate-payload-with-config(source, render-config(..args))
}

#let mermaid-result(source, ..args) = render-svg-result(source, ..args)

#let mermaid-svg(source, ..args) = {
  str(render-svg-bytes(source, ..args))
}

#let validate-mermaid(source, ..args) = validate-payload(source, ..args)

#let render-image(
  source,
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
  ..args,
) = {
  let result = render-svg-result(source, ..args)
  result-image(result, width, height, fit, alt, scale, error-mode)
}

#let render-image-with-document-context(
  source,
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
  ..args,
) = context {
  let inferred-host-theme = context-host-theme(text.font, text.size)
  let base-config = render-config(context-host-theme: inferred-host-theme, ..args)
  if base-config.direct_layout != none or base-config.direct_viewport_width != none or base-config.direct_options != none or base-config.profile_options != none {
    let result = render-svg-result-with-config(source, base-config)
    result-image(result, width, height, fit, alt, scale, error-mode)
  } else {
    typst-layout(size => {
      let result = render-svg-result-with-config(
        source,
        config-with-context-width(base-config, size.width / 1pt),
      )
      result-image(result, width, height, fit, alt, scale, error-mode)
    })
  }
}

#let mermaid(
  source,
  document-context: false,
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
  ..args,
) = {
  if document-context {
    render-image-with-document-context(
      source,
      width: width,
      height: height,
      fit: fit,
      scale: scale,
      alt: alt,
      error-mode: error-mode,
      ..args,
    )
  } else {
    render-image(
      source,
      width: width,
      height: height,
      fit: fit,
      scale: scale,
      alt: alt,
      error-mode: error-mode,
      ..args,
    )
  }
}
