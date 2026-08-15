local function read_file(file_path)
  local handle = assert(io.open(file_path, "rb"))
  local content = assert(handle:read("*a"))
  assert(handle:close())
  return content
end

local function write_file(file_path, content)
  local handle = assert(io.open(file_path, "wb"))
  assert(handle:write(content))
  assert(handle:close())
end

local package_root = assert(
  vim.env.MERMAID_PACKAGE_ROOT,
  "MERMAID_PACKAGE_ROOT is required"
)
local parser_library = assert(
  vim.env.MERMAID_PARSER_LIBRARY,
  "MERMAID_PARSER_LIBRARY is required"
)
local matrix = vim.json.decode(read_file(
  package_root .. "/test/queries/neovim/applicability.json"
))
local support = vim.json.decode(read_file(
  package_root .. "/metadata/support.json"
))

local version = vim.version()
local actual_version = string.format(
  "%d.%d.%d",
  version.major,
  version.minor,
  version.patch
)
assert(
  actual_version == matrix.consumer.version,
  string.format(
    "fixed Neovim mismatch: expected %s, got %s",
    matrix.consumer.version,
    actual_version
  )
)

vim.treesitter.language.add("mermaid", { path = parser_library })

local runtime_root = vim.fn.tempname()
local runtime_query_root = runtime_root .. "/queries/mermaid"
local runtimepath_added = false
local parsed_sources = {}

local function cleanup()
  for _, parsed in pairs(parsed_sources) do
    if parsed.buffer and vim.api.nvim_buf_is_valid(parsed.buffer) then
      pcall(vim.api.nvim_buf_delete, parsed.buffer, { force = true })
    end
  end
  if runtimepath_added then
    pcall(function()
      vim.opt.runtimepath:remove(runtime_root)
    end)
  end
  pcall(vim.fn.delete, runtime_root, "rf")
end

local function run()
  assert(vim.fn.mkdir(runtime_query_root, "p") == 1)

  for _, surface in ipairs(matrix.surfaces) do
    local source = read_file(
      package_root .. "/queries/neovim/" .. surface .. ".scm"
    )
    if surface == "highlights" then
      source = read_file(
        package_root .. "/queries/portable/highlights.scm"
      ) .. "\n" .. source
    end
    write_file(runtime_query_root .. "/" .. surface .. ".scm", source)
  end

  vim.opt.runtimepath:prepend(runtime_root)
  runtimepath_added = true

  local roots = {}
  for _, family in ipairs(support.families) do
    roots[family.publicId] = family.rootNode
  end

  local queries = {}
  for _, surface in ipairs(matrix.surfaces) do
    queries[surface] = assert(
      vim.treesitter.query.get("mermaid", surface),
      "missing Neovim query: " .. surface
    )
  end

  local counts = {}
  for _, surface in ipairs(matrix.surfaces) do
    counts[surface] = { applicable = 0, not_applicable = 0 }
  end

  local function parsed_source(relative_source)
    local parsed = parsed_sources[relative_source]
    if parsed then
      return parsed
    end
    local source = read_file(package_root .. "/" .. relative_source)
    local buffer = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(
      buffer,
      0,
      -1,
      false,
      vim.split(source, "\n", { plain = true })
    )
    vim.bo[buffer].filetype = "mermaid"

    local parser = vim.treesitter.get_parser(buffer, "mermaid", {})
    local tree = assert(parser:parse()[1])
    local root = tree:root()
    local diagram_count = 0
    local diagram_type = nil
    for child in root:iter_children() do
      if child:named() and child:type():match("_diagram$") then
        diagram_count = diagram_count + 1
        diagram_type = child:type()
      end
    end
    parsed = {
      buffer = buffer,
      parser = parser,
      tree = tree,
      root = root,
      diagram_count = diagram_count,
      diagram_type = diagram_type,
    }
    parsed_sources[relative_source] = parsed
    return parsed
  end

  for _, family in ipairs(matrix.families) do
    local explicit_surface_count = 0
    for _ in pairs(family.surfaces) do
      explicit_surface_count = explicit_surface_count + 1
    end
    assert(explicit_surface_count == 9, family.publicId .. ": explicit surfaces")
    for _, surface in ipairs(matrix.surfaces) do
      local cell = assert(family.surfaces[surface])
      counts[surface][cell.status] = counts[surface][cell.status] + 1
      if cell.status == "applicable" then
        assert(
          cell.query == "queries/neovim/" .. surface .. ".scm",
          family.publicId .. "/" .. surface .. ": query path"
        )
        local parsed = parsed_source(cell.source or family.source)
        local root = parsed.root
        assert(root:type() == "source_file")
        assert(not root:has_error(), family.publicId .. "/" .. surface .. ": parse error")

        assert(
          parsed.diagram_count == 1,
          family.publicId .. "/" .. surface .. ": diagram count"
        )
        assert(
          parsed.diagram_type == roots[family.publicId],
          family.publicId .. "/" .. surface .. ": family root"
        )

        local query = queries[surface]
        local captures = {}
        local offsets = {}
        for id, _, metadata in query:iter_captures(root, parsed.buffer, 0, -1) do
          captures[query.captures[id]] = true
          local capture_metadata = metadata[id]
          if query.captures[id] == "injection.content"
            and capture_metadata
            and capture_metadata.offset
          then
            local offset = {}
            for index, value in ipairs(capture_metadata.offset) do
              offset[index] = assert(tonumber(value))
            end
            table.insert(offsets, offset)
          end
        end
        for _, required in ipairs(cell.requiredCaptures) do
          assert(
            captures[required],
            string.format(
              "%s/%s: missing @%s",
              family.publicId,
              surface,
              required
            )
          )
        end
        if cell.requiredOffset then
          local found_offset = false
          for _, offset in ipairs(offsets) do
            if vim.deep_equal(offset, cell.requiredOffset) then
              found_offset = true
              break
            end
          end
          assert(
            found_offset,
            string.format(
              "%s/%s: missing required injection offset",
              family.publicId,
              surface
            )
          )
        end
      else
        assert(cell.query == nil, family.publicId .. "/" .. surface .. ": N/A query path")
        assert(
          type(cell.rationale) == "string" and #vim.trim(cell.rationale) >= 20,
          family.publicId .. "/" .. surface .. ": missing N/A rationale"
        )
      end
    end
  end

  for _, surface in ipairs(matrix.surfaces) do
    local count = counts[surface]
    assert(count.applicable + count.not_applicable == 35)
    print(string.format(
      "%s: %d applicable, %d not_applicable",
      surface,
      count.applicable,
      count.not_applicable
    ))
  end
  print("Neovim query matrix: 315 cells passed")
end

local ok, error_message = xpcall(run, debug.traceback)
cleanup()
if not ok then
  error(error_message, 0)
end
