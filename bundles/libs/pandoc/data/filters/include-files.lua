--- include-files.lua – filter to include Markdown files
---
--- This filter allows including external Markdown files into a document by using
--- code blocks with the 'include' class. Each line in the code block should
--- contain a file path to include. Lines starting with '//' are treated as comments.
---
--- Features:
---   - Recursive file inclusion (included files can themselves include other files)
---   - Automatic heading level adjustment based on current document heading level
---   - Manual heading level shift via 'shift-heading-level-by' attribute
---   - Relative path resolution for images and code includes in included files
---   - Support for different input formats via 'format' attribute
---
--- Usage:
---   ```{.include format="markdown"}
---   path/to/file1.md
---   path/to/file2.md
---   // This is a comment and will be ignored
---   ```
---
---   ```{.include shift-heading-level-by=1}
---   path/to/file.md
---   ```
---
--- Metadata options:
---   - include-auto: Automatically shift heading levels based on current heading level
---   - update-contents: Update relative paths in included content (images, code includes)
---
--- Copyright: © 2019–2021 Albert Krewinkel
--- License:   MIT – see LICENSE file for details

-- Module pandoc.path is required and was added in version 2.12
PANDOC_VERSION:must_be_at_least '2.12'

local List = require 'pandoc.List'
local path = require 'pandoc.path'
local system = require 'pandoc.system'

-- ============================================================================
-- Configuration Variables
-- ============================================================================

--- Whether to automatically shift heading levels based on current heading level
local include_auto = false

--- Whether to update relative paths in included content
local update_cont = false

--- Tracks the last heading level encountered in the document
local last_heading_level = 0

--- Stores title content from headers shifted to level 0 (as Inlines to preserve formatting)
local document_title_inlines = nil

--- Optional depfile populated with every Markdown file actually transcluded.
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

local function write_depfile()
  if not depfile_path or depfile_path == '' then
    return
  end
  local dependencies = {}
  for dependency, _ in pairs(included_dependencies) do
    table.insert(dependencies, dependency)
  end
  table.sort(dependencies)
  local file, error_message = io.open(depfile_path, 'w')
  if not file then
    io.stderr:write(string.format(
      "Warning: Cannot write include depfile '%s': %s\n",
      depfile_path,
      tostring(error_message)
    ))
    return
  end
  file:write('# omnidoc-depfile-v1\n')
  for _, dependency in ipairs(dependencies) do
    file:write(dependency, '\n')
  end
  file:close()
end

-- ============================================================================
-- Meta Processing
-- ============================================================================

--- Extract configuration from document metadata
--- @param meta table The document metadata
--- @return table|nil The metadata (modified if title was set from shifted header)
function get_vars(meta)
  include_auto = meta['include-auto'] and true or false
  update_cont = meta['update-contents'] and true or false
  local generic_depfile = meta['omnidoc-depfile-include-files']
  local legacy_depfile = meta['omnidoc-include-depfile']
  depfile_path = generic_depfile and pandoc.utils.stringify(generic_depfile) or
    (legacy_depfile and pandoc.utils.stringify(legacy_depfile) or nil)
  included_dependencies = {}
  return nil
end

--- Process the document and update Meta title if a header was shifted to level 0
--- @param doc table The Pandoc document
--- @return table The processed document
function Pandoc(doc)
  -- Included files are parsed independently, so Pandoc may assign the same
  -- automatic identifier to recurring headings such as "Exercises" or
  -- "Summary" in every chapter.  Once the documents are transcluded those
  -- identifiers share one global namespace.  EPUB navigation generation in
  -- particular can otherwise resolve later TOC entries to the first matching
  -- chapter.  Make identifiers unique in document order, following Pandoc's
  -- usual -1, -2, ... suffix convention.
  local used_identifiers = {}
  local deduplicate_headers = {
    Header = function(header)
      local identifier = header.identifier
      if identifier == nil or identifier == '' then
        return header
      end

      if not used_identifiers[identifier] then
        used_identifiers[identifier] = true
        return header
      end

      local suffix = 1
      local candidate = identifier .. '-' .. suffix
      while used_identifiers[candidate] do
        suffix = suffix + 1
        candidate = identifier .. '-' .. suffix
      end

      header.identifier = candidate
      used_identifiers[candidate] = true
      return header
    end
  }

  doc.blocks = pandoc.walk_block(
    pandoc.Div(doc.blocks),
    deduplicate_headers
  ).content

  -- If a title was set from a shifted header, update the document meta
  if document_title_inlines then
    -- Convert Inlines to MetaInlines to preserve formatting
    doc.meta.title = pandoc.MetaInlines(document_title_inlines)
    -- Reset for next document (if processing multiple documents)
    document_title_inlines = nil
  end
  write_depfile()
  return doc
end

-- ============================================================================
-- Header Level Tracking
-- ============================================================================

--- Update the last heading level when a header is encountered
--- This is used for automatic heading level adjustment
--- @param header table The header element
--- @return table The unchanged header element
function update_last_level(header)
  last_heading_level = header.level
  return header
end

-- ============================================================================
-- Content Transformation
-- ============================================================================

--- Update contents of included file to adjust headings and paths
--- 
--- This function applies transformations to the included content:
--- 1. Shifts heading levels by the specified amount
--- 2. Updates relative image paths to be relative to the included file's directory
--- 3. Updates relative code include paths similarly
---
--- @param blocks table List of block elements from the included file
--- @param shift_by number Number of levels to shift headings (nil = no shift)
--- @param include_path string Directory path of the included file
--- @return table Transformed list of block elements
local function update_contents(blocks, shift_by, include_path)
  local function update_relative_code_path(attributes, key)
    if attributes[key] and
       path.is_relative(attributes[key]) and
       update_cont then
      attributes[key] = path.normalize(path.join({include_path, attributes[key]}))
    end
  end

  local update_contents_filter = {
    -- Shift headings in block list by given number
    Header = function(header)
      if shift_by and shift_by ~= 0 then
        local new_level = header.level + shift_by
        -- If level becomes 0 or negative, remove the header
        if new_level <= 0 then
          if new_level == 0 then
            -- Store header content as document title
            document_title_inlines = header.content
          end
          return nil
        end
        -- Clamp level to maximum (6)
        if new_level > 6 then
          new_level = 6
        end
        -- Create a new header with updated level
        return pandoc.Header(new_level, header.content, header.attr)
      end
      return header
    end,
    -- If image paths are relative then prepend include file path
    Image = function(image)
      if path.is_relative(image.src) and update_cont then
        image.src = path.normalize(path.join({include_path, image.src}))
      end
      return image
    end,
    -- Update path for include-code-files.lua filter style CodeBlocks
    CodeBlock = function(cb)
      update_relative_code_path(cb.attributes, 'include-code')
      update_relative_code_path(cb.attributes, 'include')
      return cb
    end
  }

  return pandoc.walk_block(pandoc.Div(blocks), update_contents_filter).content
end

-- ============================================================================
-- File Transclusion
-- ============================================================================

-- Forward declaration of transclude function (defined later)
local transclude

--- Check if a CodeBlock has the 'include' class
--- @param cb table The code block element
--- @return boolean True if the block has 'include' class
local function has_include_class(cb)
  for i = 1, #cb.classes do
    if cb.classes[i] == 'include' then
      return true
    end
  end
  return false
end

--- Read and parse a file for inclusion
---
--- @param file_path string Path to the file to read
--- @param format string Format to parse the file as
--- @return table|nil Blocks from the parsed file, or nil on error
local function read_included_file(file_path, format)
  local fh = io.open(file_path)
  if not fh then
    io.stderr:write(string.format(
      "Warning: Cannot open file '%s' for inclusion. Skipping.\n",
      file_path
    ))
    return nil
  end

  -- Read file content
  local file_content = fh:read('*a')
  fh:close()
  record_dependency(file_path)

  -- Parse file content
  local success, result = pcall(function()
    return pandoc.read(file_content, format, PANDOC_READER_OPTIONS).blocks
  end)

  if not success then
    io.stderr:write(string.format(
      "Error: Failed to parse file '%s' as '%s'. Skipping.\n",
      file_path, format
    ))
    return nil
  end

  return result
end

--- Process a single included file with recursive transclusion
---
--- @param file_path string Path to the file to include
--- @param format string Format to parse the file as
--- @param shift_heading_level_by number Number of levels to shift headings
--- @return table|nil Blocks from the processed file, or nil on error
local function process_included_file(file_path, format, shift_heading_level_by)
  -- Read and parse the file
  local contents = read_included_file(file_path, format)
  if not contents then
    return nil
  end

  -- Keep track of level before recursion
  local buffer_last_heading_level = last_heading_level

  -- Reset heading level for recursive processing
  last_heading_level = 0

  -- Recursive transclusion: process the included file in its own directory
  -- This allows relative paths in the included file to work correctly
  -- Process blocks in the included file, handling nested includes
  local function process_blocks_recursive(blocks_list)
    local result = List:new()
    for _, block in ipairs(blocks_list) do
      if block.t == "Header" then
        update_last_level(block)
        result:insert(block)
      elseif block.t == "CodeBlock" and has_include_class(block) then
        -- Recursively process include CodeBlocks
        local nested_result = transclude(block)
        if nested_result then
          for _, b in ipairs(nested_result) do
            result:insert(b)
          end
        end
      else
        result:insert(block)
      end
    end
    return result
  end
  
  contents = system.with_working_directory(
    path.directory(file_path),
    function()
      return process_blocks_recursive(contents)
    end
  )

  -- Reset to level before recursion
  last_heading_level = buffer_last_heading_level

  -- Update contents (shift headings, fix paths) and return
  return update_contents(
    contents,
    shift_heading_level_by,
    path.directory(file_path)
  )
end

--- Determine heading level shift from code block attributes
---
--- @param cb table The code block element
--- @return number Number of levels to shift headings
local function determine_heading_shift(cb)
  local shift_input = cb.attributes['shift-heading-level-by']
  if shift_input then
    local shift_num = tonumber(shift_input)
    if shift_num then
      return shift_num
    end
    -- Invalid value, log warning and return 0
    io.stderr:write(string.format(
      "Warning: Invalid value for 'shift-heading-level-by' attribute: '%s'. Using 0.\n",
      tostring(shift_input)
    ))
    return 0
  end
  -- Auto shift if enabled, otherwise no shift
  return include_auto and last_heading_level or 0
end

--- Transclude external files into the document
--- 
--- This function processes code blocks with the 'include' class. Each line
--- in the code block is treated as a file path to include. Lines starting
--- with '//' are treated as comments and ignored.
---
--- The function supports:
--- - Recursive inclusion (included files can include other files)
--- - Heading level adjustment (automatic or manual)
--- - Different input formats (defaults to markdown)
--- - Relative path resolution for resources in included files
---
--- @param cb table The code block element
--- @return table|nil List of blocks if this is an include block, nil otherwise
transclude = function(cb)
  -- Ignore code blocks which are not of class "include"
  if not has_include_class(cb) then
    return nil
  end

  -- IMPORTANT: Once we've identified this as an include CodeBlock,
  -- we MUST replace it completely to prevent its attributes (like
  -- shift-heading-level-by) from being passed to the output.
  -- Even if processing fails, we return an empty blocks list to delete it.

  -- Get the format to use for reading the included file
  local format = cb.attributes['format'] or 'markdown'

  -- Determine heading level shift
  local shift_heading_level_by = determine_heading_shift(cb)

  local blocks = List:new()

  -- Process each line in the code block as a file path
  for raw_line in cb.text:gmatch('[^\n]+') do
    -- Trim whitespace and skip comments/empty lines
    local line = raw_line:match('^%s*(.-)%s*$') or raw_line
    if line ~= '' and line:sub(1, 2) ~= '//' then
      local file_blocks = process_included_file(line, format, shift_heading_level_by)
      if file_blocks then
        blocks:extend(file_blocks)
      end
    end
  end

  -- CRITICAL: Always return blocks (even if empty) to replace the include CodeBlock.
  -- This ensures the original CodeBlock with ALL its attributes (including
  -- shift-heading-level-by, format, etc.) is completely removed and doesn't
  -- get passed through to the output (e.g., LaTeX lstlisting environment).
  -- Returning an empty list will delete the CodeBlock, preventing its attributes
  -- from appearing in the final output.
  return blocks
end

-- ============================================================================
-- Filter Registration
-- ============================================================================

-- Return filter functions in the correct order
-- Meta must be processed first to get configuration
-- Then headers and code blocks are processed
-- Pandoc filter is processed last to update Meta title if needed
return {
  { Meta = get_vars },
  { Header = update_last_level, CodeBlock = transclude },
  { Pandoc = Pandoc }
}
