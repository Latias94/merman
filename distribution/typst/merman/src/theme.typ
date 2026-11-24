#import "plugin.typ": merman-plugin

#let theme-operation(operation, input) = {
  let envelope = json(
    merman-plugin.theme_operation_json(bytes(operation), input, bytes(())),
  )
  if envelope.operation != operation {
    panic("merman Typst plugin returned an unexpected theme operation")
  }
  if envelope.ok { envelope.data.result } else { envelope }
}

#let resolve-theme(document, appearance: none, auto-fix-contrast: false) = {
  let request = (document: document, auto_fix_contrast: auto-fix-contrast)
  if appearance != none { request.insert("appearance", appearance) }
  theme-operation("resolve-theme", bytes(json.encode(request)))
}

#let export-theme-preset(preset-id) = {
  if type(preset-id) != str or preset-id.trim() == "" {
    panic("merman theme preset ID must be a non-empty string")
  }
  theme-operation("export-theme-preset", bytes(preset-id))
}

#let theme-catalog() = {
  let envelope = json(merman-plugin.theme_catalog_json())
  if envelope.operation != "theme-catalog" {
    panic("merman Typst plugin returned an unexpected theme catalog operation")
  }
  if envelope.ok { envelope.data.result } else { envelope }
}
