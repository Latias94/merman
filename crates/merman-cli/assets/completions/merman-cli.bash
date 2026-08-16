_merman-cli() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="merman__cli"
                ;;
            merman__cli,batch)
                cmd="merman__cli__subcmd__batch"
                ;;
            merman__cli,capabilities)
                cmd="merman__cli__subcmd__capabilities"
                ;;
            merman__cli,completion)
                cmd="merman__cli__subcmd__completion"
                ;;
            merman__cli,detect)
                cmd="merman__cli__subcmd__detect"
                ;;
            merman__cli,fix)
                cmd="merman__cli__subcmd__fix"
                ;;
            merman__cli,layout)
                cmd="merman__cli__subcmd__layout"
                ;;
            merman__cli,lint)
                cmd="merman__cli__subcmd__lint"
                ;;
            merman__cli,lint-rules)
                cmd="merman__cli__subcmd__lint__subcmd__rules"
                ;;
            merman__cli,mmdc)
                cmd="merman__cli__subcmd__mmdc"
                ;;
            merman__cli,parse)
                cmd="merman__cli__subcmd__parse"
                ;;
            merman__cli,render)
                cmd="merman__cli__subcmd__render"
                ;;
            merman__cli,rustdoc)
                cmd="merman__cli__subcmd__rustdoc"
                ;;
            merman__cli__subcmd__rustdoc,build)
                cmd="merman__cli__subcmd__rustdoc__subcmd__build"
                ;;
            merman__cli__subcmd__rustdoc,check)
                cmd="merman__cli__subcmd__rustdoc__subcmd__check"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        merman__cli)
            opts="-h -V --help --version lint-rules rustdoc capabilities detect parse render batch lint fix completion layout mmdc"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__batch)
            opts="-o -j -f -b -C -q -s -c -t -w -H -I -h -V --stdin-file-name --output-dir --jobs --format --svg-pipeline --background --css-file --quiet --scale --raster-fit-width --raster-fit-height --raster-max-width --raster-max-height --raster-max-pixels --raster-unbounded --pdf-filter-scale --pdf-max-filter-pixels --pdf-max-filter-image-pixels --pdf-filter-unbounded --pdf-filter-images-unbounded --embedded-image-max-bytes --embedded-image-max-total-bytes --embedded-image-max-pixels --embedded-image-max-total-pixels --embedded-images-unbounded --allow-network --allow-private-network --icon-pack --icon-pack-source --suppress-errors --config-file --theme --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --presentation-profile --text-measurer --math-renderer --width --height --svg-id --hand-drawn-seed --resource-profile --resource-limit --operation-timeout-ms --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --stdin-file-name)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --output-dir)
                    COMPREPLY=()
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o plusdirs
                    fi
                    return 0
                    ;;
                -o)
                    COMPREPLY=()
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o plusdirs
                    fi
                    return 0
                    ;;
                --jobs)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -j)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "svg png jpg pdf" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "svg png jpg pdf" -- "${cur}"))
                    return 0
                    ;;
                --svg-pipeline)
                    COMPREPLY=($(compgen -W "parity readable resvg-safe" -- "${cur}"))
                    return 0
                    ;;
                --background)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --css-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -C)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-filter-scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-image-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --icon-pack)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --icon-pack-source)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --presentation-profile)
                    COMPREPLY=($(compgen -W "merman-modern" -- "${cur}"))
                    return 0
                    ;;
                --text-measurer)
                    COMPREPLY=($(compgen -W "deterministic vendored" -- "${cur}"))
                    return 0
                    ;;
                --math-renderer)
                    COMPREPLY=($(compgen -W "none ratex" -- "${cur}"))
                    return 0
                    ;;
                --width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -w)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -H)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --svg-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -I)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --hand-drawn-seed)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operation-timeout-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__capabilities)
            opts="-h -V --json --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__completion)
            opts="-h -V --help --version bash elvish fish powershell zsh"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__detect)
            opts="-h -V --resource-profile --resource-limit --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__fix)
            opts="-o -q -c -h -V --stdin-file-name --check --diff --write --output --rule --fix --quiet --markdown --config-file --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --lint-profile --enable-rule --disable-rule --rule-severity --resource-profile --resource-limit --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --stdin-file-name)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --output)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -o)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --rule)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fix)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lint-profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --enable-rule)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --disable-rule)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rule-severity)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__layout)
            opts="-c -t -w -H -h -V --pretty --suppress-errors --config-file --theme --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --text-measurer --math-renderer --width --height --resource-profile --resource-limit --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --text-measurer)
                    COMPREPLY=($(compgen -W "deterministic vendored" -- "${cur}"))
                    return 0
                    ;;
                --math-renderer)
                    COMPREPLY=($(compgen -W "none ratex" -- "${cur}"))
                    return 0
                    ;;
                --width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -w)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -H)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__lint)
            opts="-c -h -V --stdin-file-name --format --pretty --markdown --config-file --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --lint-profile --enable-rule --disable-rule --rule-severity --resource-profile --resource-limit --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --stdin-file-name)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "json text" -- "${cur}"))
                    return 0
                    ;;
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --lint-profile)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --enable-rule)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --disable-rule)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rule-severity)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__lint__subcmd__rules)
            opts="-h -V --format --pretty --configurable --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "json text" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__mmdc)
            opts="-i -o -a -j -e -b -C -f -q -p -s -c -t -w -H -I -h -V --input --output --artefacts --jobs --format --outputFormat --svg-pipeline --backgroundColor --cssFile --pdfFit --quiet --puppeteerConfigFile --scale --raster-fit-width --raster-fit-height --raster-max-width --raster-max-height --raster-max-pixels --raster-unbounded --pdf-filter-scale --pdf-max-filter-pixels --pdf-max-filter-image-pixels --pdf-filter-unbounded --pdf-filter-images-unbounded --embedded-image-max-bytes --embedded-image-max-total-bytes --embedded-image-max-pixels --embedded-image-max-total-pixels --embedded-images-unbounded --allow-network --allow-private-network --iconPacks --iconPacksNamesAndUrls --configFile --theme --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --presentation-profile --text-measurer --math-renderer --width --height --svgId --hand-drawn-seed --resource-profile --resource-limit --operation-timeout-ms --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -i)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --output)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -o)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --artefacts)
                    COMPREPLY=()
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o plusdirs
                    fi
                    return 0
                    ;;
                -a)
                    COMPREPLY=()
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o plusdirs
                    fi
                    return 0
                    ;;
                --jobs)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -j)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --outputFormat)
                    COMPREPLY=($(compgen -W "svg png pdf" -- "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "svg png pdf" -- "${cur}"))
                    return 0
                    ;;
                -e)
                    COMPREPLY=($(compgen -W "svg png pdf" -- "${cur}"))
                    return 0
                    ;;
                --svg-pipeline)
                    COMPREPLY=($(compgen -W "parity readable resvg-safe" -- "${cur}"))
                    return 0
                    ;;
                --backgroundColor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cssFile)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -C)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --puppeteerConfigFile)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -p)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-filter-scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-image-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --iconPacks)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --iconPacksNamesAndUrls)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --configFile)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "default forest dark neutral" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "default forest dark neutral" -- "${cur}"))
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --presentation-profile)
                    COMPREPLY=($(compgen -W "merman-modern" -- "${cur}"))
                    return 0
                    ;;
                --text-measurer)
                    COMPREPLY=($(compgen -W "deterministic vendored" -- "${cur}"))
                    return 0
                    ;;
                --math-renderer)
                    COMPREPLY=($(compgen -W "none ratex" -- "${cur}"))
                    return 0
                    ;;
                --width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -w)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -H)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --svgId)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -I)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --hand-drawn-seed)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operation-timeout-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__parse)
            opts="-c -t -h -V --pretty --meta --suppress-errors --config-file --theme --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --resource-profile --resource-limit --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__render)
            opts="-o -f -b -C -q -s -c -t -w -H -I -h -V --output --input-kind --format --svg-pipeline --background --css-file --quiet --scale --raster-fit-width --raster-fit-height --raster-max-width --raster-max-height --raster-max-pixels --raster-unbounded --pdf-filter-scale --pdf-max-filter-pixels --pdf-max-filter-image-pixels --pdf-filter-unbounded --pdf-filter-images-unbounded --embedded-image-max-bytes --embedded-image-max-total-bytes --embedded-image-max-pixels --embedded-image-max-total-pixels --embedded-images-unbounded --allow-network --allow-private-network --icon-pack --icon-pack-source --suppress-errors --config-file --theme --runtime --system-clock --system-timezone --system-random --system-timing --fixed-today --fixed-local-offset-minutes --presentation-profile --text-measurer --math-renderer --width --height --svg-id --hand-drawn-seed --sequence-mirror-actors --ascii-charset --ascii-width-profile --ascii-direction --ascii-color --xychart-vertical-plot-height --xychart-category-band-width --xychart-horizontal-plot-width --ascii-max-grid-cells --resource-profile --resource-limit --operation-timeout-ms --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -o)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --input-kind)
                    COMPREPLY=($(compgen -W "mermaid svg" -- "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "svg ascii unicode png jpg pdf" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "svg ascii unicode png jpg pdf" -- "${cur}"))
                    return 0
                    ;;
                --svg-pipeline)
                    COMPREPLY=($(compgen -W "parity readable resvg-safe" -- "${cur}"))
                    return 0
                    ;;
                --background)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --css-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -C)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-fit-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --raster-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-filter-scale)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-image-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf-max-filter-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --embedded-image-max-total-pixels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --icon-pack)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --icon-pack-source)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --config-file)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                -c)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "default base dark forest neutral neo neo-dark redux redux-dark redux-color redux-dark-color" -- "${cur}"))
                    return 0
                    ;;
                --runtime)
                    COMPREPLY=($(compgen -W "deterministic native" -- "${cur}"))
                    return 0
                    ;;
                --fixed-today)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fixed-local-offset-minutes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --presentation-profile)
                    COMPREPLY=($(compgen -W "merman-modern" -- "${cur}"))
                    return 0
                    ;;
                --text-measurer)
                    COMPREPLY=($(compgen -W "deterministic vendored" -- "${cur}"))
                    return 0
                    ;;
                --math-renderer)
                    COMPREPLY=($(compgen -W "none ratex" -- "${cur}"))
                    return 0
                    ;;
                --width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -w)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -H)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --svg-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -I)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --hand-drawn-seed)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ascii-charset)
                    COMPREPLY=($(compgen -W "ascii unicode" -- "${cur}"))
                    return 0
                    ;;
                --ascii-width-profile)
                    COMPREPLY=($(compgen -W "unicode cjk" -- "${cur}"))
                    return 0
                    ;;
                --ascii-direction)
                    COMPREPLY=($(compgen -W "left-right top-down" -- "${cur}"))
                    return 0
                    ;;
                --ascii-color)
                    COMPREPLY=($(compgen -W "plain auto ansi16 ansi256 truecolor html" -- "${cur}"))
                    return 0
                    ;;
                --xychart-vertical-plot-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --xychart-category-band-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --xychart-horizontal-plot-width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ascii-max-grid-cells)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --resource-profile)
                    COMPREPLY=($(compgen -W "interactive constrained trusted-native unbounded-for-trusted-input" -- "${cur}"))
                    return 0
                    ;;
                --resource-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operation-timeout-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__rustdoc)
            opts="-h -V --help --version build check"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__rustdoc__subcmd__build)
            opts="-h -V --config --quiet --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        merman__cli__subcmd__rustdoc__subcmd__check)
            opts="-h -V --config --quiet --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    local oldifs
                    if [ -n "${IFS+x}" ]; then
                        oldifs="$IFS"
                    fi
                    IFS=$'\n'
                    COMPREPLY=($(compgen -f "${cur}"))
                    if [ -n "${oldifs+x}" ]; then
                        IFS="$oldifs"
                    fi
                    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
                        compopt -o filenames
                    fi
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _merman-cli -o nosort -o bashdefault -o default merman-cli
else
    complete -F _merman-cli -o bashdefault -o default merman-cli
fi
