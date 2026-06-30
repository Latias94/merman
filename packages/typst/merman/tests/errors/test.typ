#import "@preview/merman:0.1.0": mermaid, mermaid-result

#let invalid-source = ""

#let result = mermaid-result(invalid-source)
#assert(not result.ok, message: "invalid source should return a structured render error")
#assert.eq(result.code_name, "MERMAN_NO_DIAGRAM")
#assert(result.message != none, message: "invalid source should carry an error message")

#mermaid(invalid-source, error-mode: "text")

#mermaid(invalid-source, error-mode: "placeholder", width: 80%)

Error fixture passed.
