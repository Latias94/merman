#import "@preview/merman:0.5.0": mermaid, mermaid-result

#let invalid-source = ""

#let result = mermaid-result(invalid-source)
#assert(not result.ok, message: "invalid source should return a structured render error")
#assert.eq(result.code_name, "MERMAN_NO_DIAGRAM")
#assert(result.message != none, message: "invalid source should carry an error message")

#let malformed-source = "\u{feff}---\ntitle: 中文\n---\n%% 注释\nflowchart TD\nA --> ]"
#let parse-result = mermaid-result(malformed-source)
#assert(not parse-result.ok, message: "malformed source should return a parse error")
#assert.eq(parse-result.operation, "render-svg")
#assert.eq(parse-result.code_name, "MERMAN_PARSE_ERROR")
#assert.eq(parse-result.code, 5)
#assert.eq(parse-result.details.diagnostic.code, "merman.parse.diagram_parse")
#assert.eq(parse-result.details.diagnostic.diagram_type, "flowchart-v2")
#assert.eq(parse-result.details.diagnostic.span, (start: 54, end: 55, kind: "exact"))

#let counts = json(bytes("{\"above_js_safe\":9007199254740993,\"i64_max\":9223372036854775807,\"u64_max\":18446744073709551615}"))
#assert.eq(type(counts.above_js_safe), int)
#assert.eq(counts.above_js_safe, 9007199254740993)
#assert.eq(type(counts.i64_max), int)
#assert.eq(counts.i64_max, 9223372036854775807)
#assert.eq(type(counts.u64_max), float)

#mermaid(invalid-source, error-mode: "text")

#mermaid(invalid-source, error-mode: "placeholder", width: 80%)

Error fixture passed.
