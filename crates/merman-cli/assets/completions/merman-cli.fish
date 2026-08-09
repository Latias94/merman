# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_merman_cli_global_optspecs
    string join \n h/help V/version
end

function __fish_merman_cli_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_merman_cli_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_merman_cli_using_subcommand
    set -l cmd (__fish_merman_cli_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c merman-cli -n "__fish_merman_cli_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "lint-rules" -d 'List lint rule metadata'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "capabilities" -d 'Print the compiled capabilities from the canonical capability descriptor'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "detect" -d 'Detect the Mermaid diagram type'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "parse" -d 'Parse Mermaid source and print the semantic JSON model'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "render" -d 'Render one Mermaid diagram with Merman\'s native interface'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "batch" -d 'Render every Mermaid diagram in one Markdown document'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "lint" -d 'Analyze Mermaid source and print diagnostics JSON or text'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "fix" -d 'Apply non-conflicting diagnostics fixes to Mermaid or Markdown source'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "completion" -d 'Generate shell completion scripts'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "layout" -d 'Parse and layout Mermaid source, then print layout JSON'
complete -c merman-cli -n "__fish_merman_cli_needs_command" -f -a "mmdc" -d 'Render through the pinned mmdc-compatible interface'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint-rules" -l format -d 'Output format for rule metadata' -r -f -a "json\t''
text\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint-rules" -l pretty -d 'Pretty-print JSON output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint-rules" -l configurable -d 'Only list rules that public lint configuration can reference'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint-rules" -s h -l help -d 'Print help'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint-rules" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand capabilities" -l json -d 'Emit the machine-readable capability document'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand capabilities" -s h -l help -d 'Print help'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand capabilities" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand detect" -l resource-profile -d 'Resource policy used to bound source acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand detect" -l resource-limit -d 'Override source bytes as max_source_bytes=POSITIVE_U64' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand detect" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand detect" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -s t -l theme -d 'Mermaid theme override' -r -f -a "default\t''
base\t''
dark\t''
forest\t''
neutral\t''
neo\t''
neo-dark\t''
redux\t''
redux-dark\t''
redux-color\t''
redux-dark-color\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l pretty -d 'Pretty-print JSON output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l meta -d 'Include parse metadata alongside the model'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l suppress-errors -d 'Emit an error diagram instead of failing on parse errors'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand parse" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s o -l output -d 'Output file. Use `-` for stdout' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l input-kind -d 'Interpret the input as Mermaid source or an existing SVG document' -r -f -a "mermaid\t''
svg\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s f -l format -d 'Output format. Defaults to the first compiled output format' -r -f -a "svg\t''
ascii\t''
unicode\t''
png\t''
jpg\t''
pdf\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l svg-pipeline -d 'SVG output pipeline. Compiled binary exports always start from resvg-safe' -r -f -a "parity\t''
readable\t''
resvg-safe\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s b -l background -d 'Background color for the selected rendered output' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s C -l css-file -d 'CSS file injected into SVG output before export' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s s -l scale -d 'Raster output scale factor. Defaults to 1' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-fit-width -d 'Fit raster output to this CSS-pixel width before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-fit-height -d 'Fit raster output to this CSS-pixel height before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-max-width -d 'Maximum raster output width after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-max-height -d 'Maximum raster output height after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-max-pixels -d 'Maximum raster output pixels after scale and fit. Defaults to 4096*4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l pdf-filter-scale -d 'Sampling scale for SVG filters that require localized PDF bitmaps. Defaults to 4' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l pdf-max-filter-image-pixels -l pdf-max-filter-pixels -d 'Maximum aggregate pixels retained as localized PDF filter images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l embedded-image-max-bytes -d 'Maximum decoded data-URL bytes for one embedded image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l embedded-image-max-total-bytes -d 'Maximum aggregate decoded data-URL bytes for embedded images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l embedded-image-max-pixels -d 'Maximum intrinsic pixels for one embedded raster image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l embedded-image-max-total-pixels -d 'Maximum aggregate intrinsic pixels for embedded raster images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l icon-pack -d 'Iconify package name or local package path. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l icon-pack-source -d 'Iconify prefix and source as PREFIX#SOURCE. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s t -l theme -d 'Mermaid theme override' -r -f -a "default\t''
base\t''
dark\t''
forest\t''
neutral\t''
neo\t''
neo-dark\t''
redux\t''
redux-dark\t''
redux-color\t''
redux-dark-color\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l presentation-profile -d 'First-party presentation profile applied below explicit Mermaid configuration' -r -f -a "merman-modern\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l text-measurer -d 'Text measurement strategy' -r -f -a "deterministic\t''
vendored\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l math-renderer -d 'Math renderer override. Unspecified uses the compiled default; `ratex` requires `math`' -r -f -a "none\t''
ratex\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s w -l width -d 'Available container width for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s H -l height -d 'Available container height for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s I -l svg-id -d 'Root SVG id and internal marker prefix' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l hand-drawn-seed -d 'Stabilize rough/hand-drawn rendering where supported' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l ascii-charset -d 'Override the text renderer character set' -r -f -a "ascii\t''
unicode\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l ascii-width-profile -d 'Display-width convention used for terminal text measurement' -r -f -a "unicode\t''
cjk\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l ascii-direction -d 'Override the default graph direction when Mermaid input omits one' -r -f -a "left-right\t''
top-down\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l ascii-color -d 'Color mode for terminal text output' -r -f -a "plain\t''
auto\t''
ansi16\t''
ansi256\t''
truecolor\t''
html\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l xychart-vertical-plot-height -d 'XYChart vertical plot height for text output' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l xychart-category-band-width -d 'XYChart category band width for text output' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l xychart-horizontal-plot-width -d 'XYChart horizontal plot width for text output' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l ascii-max-grid-cells -d 'Maximum graph grid cells for text route planning' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s q -l quiet -d 'Suppress non-error log output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l raster-unbounded -d 'Disable raster size limits. Use only for trusted oversized exports'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l pdf-filter-images-unbounded -l pdf-filter-unbounded -d 'Disable the retained PDF filter-image pixel budget for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l embedded-images-unbounded -d 'Disable embedded raster image decode budgets for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l allow-network -d 'Allow icon pack loading from public HTTP(S) destinations'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l allow-private-network -d 'Allow explicitly configured icon URLs to resolve to private or loopback addresses'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l suppress-errors -d 'Emit an error diagram instead of failing on parse errors'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -l sequence-mirror-actors -d 'Mirror sequence participants below lifelines for ASCII/Unicode output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand render" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l stdin-file-name -d 'Logical source file name for Markdown read from stdin' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s o -l output-dir -d 'Tool-owned directory for the rewritten document and generated artifacts' -r -f -a "(__fish_complete_directories)"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s j -l jobs -d 'Maximum number of Markdown charts rendered concurrently' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s f -l format -d 'Output format. Defaults to SVG' -r -f -a "svg\t''
png\t''
jpg\t''
pdf\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l svg-pipeline -d 'SVG output pipeline. Compiled binary exports always start from resvg-safe' -r -f -a "parity\t''
readable\t''
resvg-safe\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s b -l background -d 'Background color for the selected rendered output' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s C -l css-file -d 'CSS file injected into SVG output before export' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s s -l scale -d 'Raster output scale factor. Defaults to 1' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-fit-width -d 'Fit raster output to this CSS-pixel width before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-fit-height -d 'Fit raster output to this CSS-pixel height before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-max-width -d 'Maximum raster output width after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-max-height -d 'Maximum raster output height after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-max-pixels -d 'Maximum raster output pixels after scale and fit. Defaults to 4096*4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l pdf-filter-scale -d 'Sampling scale for SVG filters that require localized PDF bitmaps. Defaults to 4' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l pdf-max-filter-image-pixels -l pdf-max-filter-pixels -d 'Maximum aggregate pixels retained as localized PDF filter images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l embedded-image-max-bytes -d 'Maximum decoded data-URL bytes for one embedded image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l embedded-image-max-total-bytes -d 'Maximum aggregate decoded data-URL bytes for embedded images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l embedded-image-max-pixels -d 'Maximum intrinsic pixels for one embedded raster image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l embedded-image-max-total-pixels -d 'Maximum aggregate intrinsic pixels for embedded raster images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l icon-pack -d 'Iconify package name or local package path. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l icon-pack-source -d 'Iconify prefix and source as PREFIX#SOURCE. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s t -l theme -d 'Mermaid theme override' -r -f -a "default\t''
base\t''
dark\t''
forest\t''
neutral\t''
neo\t''
neo-dark\t''
redux\t''
redux-dark\t''
redux-color\t''
redux-dark-color\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l presentation-profile -d 'First-party presentation profile applied below explicit Mermaid configuration' -r -f -a "merman-modern\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l text-measurer -d 'Text measurement strategy' -r -f -a "deterministic\t''
vendored\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l math-renderer -d 'Math renderer override. Unspecified uses the compiled default; `ratex` requires `math`' -r -f -a "none\t''
ratex\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s w -l width -d 'Available container width for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s H -l height -d 'Available container height for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s I -l svg-id -d 'Root SVG id and internal marker prefix' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l hand-drawn-seed -d 'Stabilize rough/hand-drawn rendering where supported' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s q -l quiet -d 'Suppress non-error log output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l raster-unbounded -d 'Disable raster size limits. Use only for trusted oversized exports'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l pdf-filter-images-unbounded -l pdf-filter-unbounded -d 'Disable the retained PDF filter-image pixel budget for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l embedded-images-unbounded -d 'Disable embedded raster image decode budgets for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l allow-network -d 'Allow icon pack loading from public HTTP(S) destinations'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l allow-private-network -d 'Allow explicitly configured icon URLs to resolve to private or loopback addresses'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l suppress-errors -d 'Emit an error diagram instead of failing on parse errors'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand batch" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l stdin-file-name -d 'Optional file name to use when linting stdin' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l format -d 'Output format for diagnostics' -r -f -a "json\t''
text\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l lint-profile -d 'Built-in lint rule profile: core, recommended, or strict' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l enable-rule -d 'Enable a configurable lint rule by stable rule id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l disable-rule -d 'Disable a configurable lint rule by stable rule id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l rule-severity -d 'Override a configurable lint rule severity as RULE_ID=error|warning|info|hint. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l pretty -d 'Pretty-print JSON output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l markdown -d 'Include Markdown fence diagnostics by scanning `.md`, `.markdown`, or `.mdx` input'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand lint" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l stdin-file-name -d 'Optional file name to use when fixing stdin' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -s o -l output -d 'Write the result to this file instead of stdout' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l rule -d 'Restrict fixes to this fixable lint rule id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l fix -d 'Select an exact stable fix id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l lint-profile -d 'Built-in lint rule profile: core, recommended, or strict' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l enable-rule -d 'Enable a configurable lint rule by stable rule id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l disable-rule -d 'Disable a configurable lint rule by stable rule id. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l rule-severity -d 'Override a configurable lint rule severity as RULE_ID=error|warning|info|hint. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l check -d 'Exit 1 when the selected fixes would change the source'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l diff -d 'Print a unified diff and exit 1 when the source would change'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l write -d 'Write the result back to the input file'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -s q -l quiet -d 'Suppress non-error fix selection diagnostics'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l markdown -d 'Include Markdown fence diagnostics by scanning `.md`, `.markdown`, or `.mdx` input'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand fix" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand completion" -s h -l help -d 'Print help'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand completion" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s c -l config-file -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s t -l theme -d 'Mermaid theme override' -r -f -a "default\t''
base\t''
dark\t''
forest\t''
neutral\t''
neo\t''
neo-dark\t''
redux\t''
redux-dark\t''
redux-color\t''
redux-dark-color\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l text-measurer -d 'Text measurement strategy' -r -f -a "deterministic\t''
vendored\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l math-renderer -d 'Math renderer override. Unspecified uses the compiled default; `ratex` requires `math`' -r -f -a "none\t''
ratex\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s w -l width -d 'Available container width for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s H -l height -d 'Available container height for size-sensitive layouts' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l pretty -d 'Pretty-print JSON output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l suppress-errors -d 'Emit an error diagram instead of failing on parse errors'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand layout" -s V -l version -d 'Print version'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s i -l input -d 'Input Mermaid file. Use `-` for stdin' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s o -l output -d 'Output file. Use `-` for stdout' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s a -l artefacts -d 'Output artefacts directory for Markdown input' -r -f -a "(__fish_complete_directories)"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s j -l jobs -d 'Parallel jobs for Markdown input. Defaults and maxima come from the resource profile' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s e -l outputFormat -l format -d 'Output format. Defaults to the output extension, then SVG' -r -f -a "svg\t''
png\t''
pdf\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l svg-pipeline -d 'SVG output pipeline. Compiled binary exports always start from resvg-safe' -r -f -a "parity\t''
readable\t''
resvg-safe\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s b -l backgroundColor -d 'Background color for the selected rendered output. `mmdc` defaults to white' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s C -l cssFile -d 'CSS file injected into SVG output before export' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s p -l puppeteerConfigFile -d 'JSON Puppeteer configuration file. Accepted for mmdc compatibility' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s s -l scale -d 'Raster output scale factor. Defaults to 1' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-fit-width -d 'Fit raster output to this CSS-pixel width before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-fit-height -d 'Fit raster output to this CSS-pixel height before applying --scale' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-max-width -d 'Maximum raster output width after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-max-height -d 'Maximum raster output height after scale and fit. Defaults to 4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-max-pixels -d 'Maximum raster output pixels after scale and fit. Defaults to 4096*4096' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l pdf-filter-scale -d 'Sampling scale for SVG filters that require localized PDF bitmaps. Defaults to 4' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l pdf-max-filter-image-pixels -l pdf-max-filter-pixels -d 'Maximum aggregate pixels retained as localized PDF filter images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l embedded-image-max-bytes -d 'Maximum decoded data-URL bytes for one embedded image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l embedded-image-max-total-bytes -d 'Maximum aggregate decoded data-URL bytes for embedded images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l embedded-image-max-pixels -d 'Maximum intrinsic pixels for one embedded raster image. Defaults to 16777216' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l embedded-image-max-total-pixels -d 'Maximum aggregate intrinsic pixels for embedded raster images. Defaults to 33554432' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l iconPacks -d 'Iconify package names' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l iconPacksNamesAndUrls -d 'Iconify prefix#url definitions' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s c -l configFile -d 'JSON Mermaid configuration file' -r -F
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s t -l theme -d 'Theme of the chart' -r -f -a "default\t''
forest\t''
dark\t''
neutral\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l runtime -d 'Runtime source for clock, local timezone, and operation randomness' -r -f -a "deterministic\t''
native\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l fixed-today -d 'Override the local "today" date for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l fixed-local-offset-minutes -d 'Override the local timezone offset in minutes for time-dependent diagrams' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l presentation-profile -d 'First-party presentation profile applied below explicit Mermaid configuration' -r -f -a "merman-modern\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l text-measurer -d 'Text measurement strategy' -r -f -a "deterministic\t''
vendored\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l math-renderer -d 'Math renderer override. Unspecified uses the compiled default' -r -f -a "none\t''
ratex\t''"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s w -l width -d 'Width of the page' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s H -l height -d 'Height of the page' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s I -l svgId -d 'Root SVG id and internal marker prefix' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l hand-drawn-seed -d 'Stabilize rough/hand-drawn rendering where supported' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l resource-profile -d 'Resource policy for input, semantic models, output, and CLI acquisition' -r -f -a "interactive\t'General interactive applications and public binding surfaces'
constrained\t'Constrained rendering for untrusted or publicly submitted documents'
trusted-native\t'Local CLI and controlled native batch rendering'
unbounded-for-trusted-input\t'Explicitly disable policy budgets while retaining hard backend capabilities'"
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l resource-limit -d 'Override a resource budget as STABLE_ID=POSITIVE_U64. Can be repeated' -r
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s f -l pdfFit -d 'Scale PDF to fit chart. Accepted for mmdc compatibility'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s q -l quiet -d 'Suppress non-error log output'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l raster-unbounded -d 'Disable raster size limits. Use only for trusted oversized exports'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l pdf-filter-images-unbounded -l pdf-filter-unbounded -d 'Disable the retained PDF filter-image pixel budget for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l embedded-images-unbounded -d 'Disable embedded raster image decode budgets for trusted inputs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l allow-network -d 'Allow icon pack loading from HTTP(S) URLs'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l allow-private-network -d 'Allow explicitly configured icon URLs to resolve to private or loopback addresses'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l system-clock -d 'Use the system clock while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l system-timezone -d 'Use the system local timezone while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l system-random -d 'Use system randomness while keeping other runtime sources deterministic'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -l system-timing -d 'Enable operation timing diagnostics through the compiled system timing adapter'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c merman-cli -n "__fish_merman_cli_using_subcommand mmdc" -s V -l version -d 'Print version'
