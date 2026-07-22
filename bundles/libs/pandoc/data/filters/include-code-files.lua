--- include-code-files.lua – filter to include code from source files
---
--- This filter allows including code from external source files into code blocks.
--- The code block should have an 'include-code' attribute specifying the file path.
---
--- Features:
---   - Include entire files or specific line ranges
---   - Support for dedenting code (removing leading whitespace)
---   - Preserves code block attributes (class, identifier, etc.)
---   - Supports both hyphenated and PascalCase attribute names
---
--- Usage:
---   ```python {include-code="path/to/file.py"}
---   ```
---
---   ```python {include-code="path/to/file.py" startLine=10 endLine=20}
---   ```
---
---   ```python {include-code="path/to/file.py" start-line=10 end-line=20 dedent=2}
---   ```
---
--- Attributes:
---   - include-code: Path to the source file to include (required)
---   - startLine / start-line: First line to include (default: 1)
---   - endLine / end-line: Last line to include (default: end of file)
---   - dedent: Number of leading spaces to remove from each line (default: 0)
---
--- Copyright: © 2020 Bruno BEAUFILS
--- License:   MIT – see LICENSE file for details

-- ============================================================================
-- Helper Functions
-- ============================================================================

local path = require 'pandoc.path'
local system = require 'pandoc.system'

local depfile_path = nil
local included_dependencies = {}

local function absolute_dependency(file_path)
  local resolved = file_path
  if path.is_relative(resolved) then
    resolved = path.join({system.get_working_directory(), resolved})
  end
  return path.normalize(resolved)
end

local function record_dependency(file_path)
  local resolved = absolute_dependency(file_path)
  if not resolved:find('[\r\n]') then
    included_dependencies[resolved] = true
  end
end

local function configure_depfile(meta)
  local generic_depfile = meta['omnidoc-depfile-include-code-files']
  local legacy_depfile = meta['omnidoc-include-code-depfile']
  depfile_path = generic_depfile and pandoc.utils.stringify(generic_depfile) or
    (legacy_depfile and pandoc.utils.stringify(legacy_depfile) or nil)
  included_dependencies = {}
  return nil
end

local function write_depfile(doc)
  if not depfile_path or depfile_path == '' then
    return doc
  end
  local dependencies = {}
  for dependency, _ in pairs(included_dependencies) do
    table.insert(dependencies, dependency)
  end
  table.sort(dependencies)
  local file, error_message = io.open(depfile_path, 'w')
  if not file then
    io.stderr:write(string.format(
      "Warning: Cannot write include-code depfile '%s': %s\n",
      depfile_path,
      tostring(error_message)
    ))
    return doc
  end
  file:write('# omnidoc-depfile-v1\n')
  for _, dependency in ipairs(dependencies) do
    file:write(dependency, '\n')
  end
  file:close()
  return doc
end

--- Remove leading whitespace from a line
--- This function removes the specified number of leading spaces from a line.
--- If the line doesn't have enough leading spaces, it removes all leading spaces.
---
--- @param line string The line to dedent
--- @param n number Number of spaces to remove
--- @return string The dedented line
local function dedent(line, n)
  if not n or n <= 0 then
    return line
  end
  
  -- Count leading spaces
  local leading_spaces = line:match('^(%s*)')
  local space_count = #leading_spaces
  
  -- Remove up to n spaces (or all if fewer than n)
  local to_remove = math.min(space_count, n)
  return line:sub(to_remove + 1)
end

--- Normalize attribute names from hyphenated to PascalCase
--- This function converts hyphenated attribute names (e.g., "start-line")
--- to PascalCase (e.g., "startLine") for consistency.
---
--- @param attributes table The attributes table to normalize
--- @return table The normalized attributes table
local function normalize_attributes(attributes)
  local normalized = {}
  local pascal_case_map = {
    ["start-line"] = "startLine",
    ["end-line"] = "endLine",
  }
  
  for key, value in pairs(attributes) do
    -- Check if this is a hyphenated version that should be converted
    if pascal_case_map[key] then
      normalized[pascal_case_map[key]] = value
    else
      normalized[key] = value
    end
  end
  
  return normalized
end

-- ============================================================================
-- Code Block Processing
-- ============================================================================

--- Parse line range parameters from attributes
---
--- @param normalized_attrs table Normalized attributes
--- @param file_path string File path for error messages
--- @return number, number|nil, number Start line, end line (or nil), dedent count
local function parse_line_range_params(normalized_attrs, file_path)
  local start_line = 1
  local end_line = nil
  local dedent_count = 0

  if normalized_attrs.startLine then
    start_line = tonumber(normalized_attrs.startLine) or 1
    if start_line < 1 then
      start_line = 1
    end
  end

  if normalized_attrs.endLine then
    end_line = tonumber(normalized_attrs.endLine)
    if end_line and end_line < start_line then
      io.stderr:write(string.format(
        "Warning: endLine (%d) is less than startLine (%d) in include-code for '%s'. Ignoring endLine.\n",
        end_line, start_line, file_path
      ))
      end_line = nil
    end
  end

  if normalized_attrs.dedent then
    dedent_count = tonumber(normalized_attrs.dedent) or 0
    if dedent_count < 0 then
      dedent_count = 0
    end
  end

  return start_line, end_line, dedent_count
end

--- Read and process lines from file
---
--- @param file_path string Path to the file
--- @param start_line number First line to include
--- @param end_line number|nil Last line to include (or nil for end of file)
--- @param dedent_count number Number of spaces to remove from each line
--- @return string, number Content string and number of included lines
local function read_file_lines(file_path, start_line, end_line, dedent_count)
  local fh = io.open(file_path)
  if not fh then
    io.stderr:write(string.format(
      "Error: Cannot open file '%s' for code inclusion. Skipping.\n",
      file_path
    ))
    return nil, 0
  end

  local content = ""
  local line_number = 1
  local included_lines = 0

  for line in fh:lines("L") do
    -- Include lines in the specified range
    if line_number >= start_line then
      if not end_line or line_number <= end_line then
        -- Apply dedenting if specified
        local processed_line = line
        if dedent_count > 0 then
          processed_line = dedent(line, dedent_count)
        end
        content = content .. processed_line
        included_lines = included_lines + 1
      else
        -- We've reached the end line, stop reading
        break
      end
    end
    line_number = line_number + 1
  end

  fh:close()
  return content, included_lines
end

--- Remove processing attributes from code block attributes
---
--- @param attributes table Original attributes
--- @return table Filtered attributes without processing keys
local function filter_processing_attributes(attributes)
  local final_attrs = {}
  for key, value in pairs(attributes) do
    if key ~= 'include-code' and
       key ~= 'startLine' and key ~= 'start-line' and
       key ~= 'endLine' and key ~= 'end-line' and
       key ~= 'dedent' then
      final_attrs[key] = value
    end
  end
  return final_attrs
end

--- Transclude code from external files into code blocks
---
--- This function processes code blocks with an 'include-code' attribute.
--- It reads the specified file and includes its content (or a line range) into
--- the code block, optionally dedenting the code.
---
--- @param cb table The code block element
--- @return table|nil The updated code block, or nil if not an include block
local function transclude(cb)
  -- Check if this is a code include block
  if not cb.attributes['include-code'] then
    return nil
  end

  local file_path = cb.attributes['include-code']

  -- Normalize attribute names (convert hyphenated to PascalCase)
  local normalized_attrs = normalize_attributes(cb.attributes)

  -- Parse line range parameters
  local start_line, end_line, dedent_count = parse_line_range_params(normalized_attrs, file_path)

  -- Read and process file lines
  local content, included_lines = read_file_lines(file_path, start_line, end_line, dedent_count)

  -- Handle file read errors
  if content == nil then
    -- Return error code block
    return pandoc.CodeBlock(
      string.format("// Error: Could not include file '%s'", file_path),
      cb.attr
    )
  end
  record_dependency(file_path)

  -- Warn if no lines were included
  if included_lines == 0 then
    io.stderr:write(string.format(
      "Warning: No lines included from '%s' (startLine=%d, endLine=%s).\n",
      file_path, start_line, end_line and tostring(end_line) or "nil"
    ))
  end

  -- Remove processing attributes
  local final_attrs = filter_processing_attributes(cb.attributes)

  -- Create new code block with included content
  -- Preserve the original code block's identifier, classes, and other attributes
  local new_attr = pandoc.Attr(
    cb.identifier,
    cb.classes,
    final_attrs
  )

  return pandoc.CodeBlock(content, new_attr)
end

-- ============================================================================
-- Filter Registration
-- ============================================================================

return {
  { Meta = configure_depfile },
  { CodeBlock = transclude },
  { Pandoc = write_depfile }
}
