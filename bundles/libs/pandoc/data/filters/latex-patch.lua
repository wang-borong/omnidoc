--- latex-patch.lua – LaTeX-specific patches and fixes
---
--- This filter applies various LaTeX-specific patches and fixes to improve
--- the quality of LaTeX output:
---
--- Features:
---   - Fixes spacing around \ref commands (adds non-breaking space)
---   - Automatically finds and corrects image paths
---   - Removes spaces before citations and references
---   - Adds spaces after citations when needed
---
--- Copyright: Original author unknown
--- License:   MIT

-- ============================================================================
-- Helper Functions
-- ============================================================================

local system = require 'pandoc.system'

--- Create a LaTeX raw inline element
--- @param str string The LaTeX code
--- @return table A RawInline element
local function latex(str)
  return pandoc.RawInline('latex', str)
end

--- Check if a string starts with a given prefix
--- @param start string The prefix to check
--- @param str string The string to check
--- @return boolean True if the string starts with the prefix
local function starts_with(start, str)
  return str:sub(1, #start) == start
end

--- Check if a list contains a specific value
--- @param list table The list to search
--- @param x any The value to find
--- @return boolean True if the value is in the list
local function contains_value(list, x)
  for _, v in pairs(list) do
    if v == x then
      return true
    end
  end
  return false
end

--- Check if a file has a legal extension
--- @param filename string The filename to check
--- @param legal_ext table List of legal extensions (without the dot)
--- @return boolean True if the file has a legal extension
local function check_file_ext(filename, legal_ext)
  local file_ext = filename:match("^.+%.(.+)$")
  if not file_ext then
    return false
  end
  return contains_value(legal_ext, file_ext)
end

local function basename(filename)
  return filename:match("[^/\\]+$") or filename
end

local function join_path(parent, child)
  if parent == "." then
    return child
  end
  return parent .. "/" .. child
end

local function is_pruned_dir(dirname)
  local pruned_dirs = {
    appendix = true,
    dac = true,
    drawio = true,
    pandoc = true,
    reference = true,
    texmf = true,
    tool = true,
    [".git"] = true,
  }

  return pruned_dirs[basename(dirname)] or false
end

local function list_directory(dirname)
  if not system.list_directory then
    return nil
  end

  local ok, entries = pcall(system.list_directory, dirname)
  if not ok then
    return nil
  end
  return entries
end

local function find_image_file(root, image_name, legal_ext)
  local entries = list_directory(root)
  if not entries then
    return nil
  end

  table.sort(entries)
  for _, entry in ipairs(entries) do
    local candidate = join_path(root, entry)
    local candidate_base = basename(candidate)

    if not is_pruned_dir(candidate) then
      local child_entries = list_directory(candidate)
      if child_entries then
        local found = find_image_file(candidate, image_name, legal_ext)
        if found then
          return found
        end
      elseif candidate_base:sub(1, #image_name) == image_name and
             check_file_ext(candidate_base, legal_ext) then
        return candidate:gsub("^%./", "")
      end
    end
  end

  return nil
end

-- ============================================================================
-- RawInline Processing
-- ============================================================================

--- Fix LaTeX \ref commands by adding non-breaking space
--- 
--- In LaTeX, references should be preceded by a non-breaking space (~) to
--- prevent line breaks between the preceding word and the reference number.
--- This function adds the non-breaking space if it's missing.
---
--- @param rl table The RawInline element
--- @return table The fixed RawInline element
local function fix_rawinline(rl)
  if rl.format ~= 'latex' then
    return rl
  end

  -- Add a non-breaking space (~) before \ref commands
  if starts_with('\\ref', rl.text) then
    -- Remove existing spaces and tildes, then add a single tilde
    local fixed_text = string.gsub(rl.text, '[ ~]*(.*) *', '~%1')
    return latex(fixed_text)
  end

  return rl
end

-- ============================================================================
-- Image Processing
-- ============================================================================

--- Process images to automatically find and correct image paths
---
--- This function attempts to find image files in the document directory
--- structure when the image path doesn't include a directory. It searches
--- common directories (excluding certain directories like .git, texmf, etc.)
--- and updates the image path if a matching file is found.
---
--- Supported image formats: pdf, png, jpg, ps, fig, eps
---
--- @param image table The Image element
--- @return table|nil The processed Image element, or nil to skip
local function proc_image(image)
  if not image.src then
    return image
  end

  -- Prefer a pre-rendered PDF sibling for SVG sources when available. This
  -- keeps XeLaTeX builds deterministic and avoids requiring shell-escape and
  -- Inkscape merely to consume an SVG that the project already exports for
  -- EPUB/HTML.
  if image.src:lower():match('%.svg$') then
    local pdf_path = image.src:gsub('%.[sS][vV][gG]$', '.pdf')
    local pdf = io.open(pdf_path, 'rb')
    if pdf then
      pdf:close()
      image.src = pdf_path
      return image
    end

    -- XeLaTeX's svg package normally requires shell escape and writes
    -- converter side files beside the document. Convert an SVG without a
    -- checked-in PDF sibling into Pandoc's media bag instead. The hashed name
    -- is deterministic, keeps source trees clean, and lets the LaTeX writer
    -- consume a normal PDF image from its temporary media directory.
    if FORMAT:match('latex') then
      local fetched, mime, svg = pcall(pandoc.mediabag.fetch, image.src)
      if not fetched or not svg then
        error('Cannot read SVG image for PDF conversion: ' .. image.src ..
              '. Add the resource path or provide a same-basename PDF sibling.')
      end
      local converted, pdf_data = pcall(
        pandoc.pipe,
        'rsvg-convert',
        {'--format=pdf'},
        svg
      )
      if not converted then
        -- Some legitimate engineering diagrams embed high-resolution raster
        -- data and exceed libxml's default parser buffer. Retry only after the
        -- bounded conversion failed; this keeps the normal path conservative
        -- while still supporting those large, local source assets.
        converted, pdf_data = pcall(
          pandoc.pipe,
          'rsvg-convert',
          {'--unlimited', '--format=pdf'},
          svg
        )
      end
      if not converted or not pdf_data then
        error('Cannot convert SVG image for PDF output: ' .. image.src ..
              '. Install rsvg-convert or provide a same-basename PDF sibling.')
      end
      local generated = 'omnidoc-svg-' .. pandoc.sha1(svg) .. '.pdf'
      pandoc.mediabag.insert(generated, 'application/pdf', pdf_data)
      image.src = generated
      return image
    end
  end

  -- Skip images that are already in figures/ or images/ directories
  if image.src:match('figures?/', 1) or image.src:match('images?/') then
    return nil
  end

  -- Legal image file extensions
  local legal_ext = {'pdf', 'png', 'jpg', 'ps', 'fig', 'eps'}

  local image_name = image.src:match("[^/]*$")
  local resolved_path = find_image_file(".", image_name, legal_ext)
  if resolved_path then
    image.src = resolved_path
    return image
  end

  if check_file_ext(image.src, legal_ext) then
    local fh = io.open(image.src, "rb")
    if fh then
      fh:close()
      return image
    end
  end

  return image
end

-- ============================================================================
-- Citation and Reference Spacing
-- ============================================================================

--- Check if there's a space before a reference that should be removed
---
--- In LaTeX, spaces before citations and cross-references should typically
--- be removed to prevent awkward line breaks. This function checks if a
--- space element should be removed based on what follows it.
---
--- @param spc table The Space element (or any inline element)
--- @param ref table The reference element that follows
--- @return boolean True if the space should be removed
local function is_space_before_ref(spc, ref)
  -- Check if the first element is a Space
  if not (spc and spc.t == 'Space') then
    return false
  end

  -- Check if the second element is a reference that needs space removal
  if not ref then
    return false
  end

  -- Check for RawInline with tilde (non-breaking space reference)
  if ref.t == 'RawInline' and starts_with('~', ref.text) then
    return true
  end

  -- Check for citations with specific prefixes
  if ref.t == 'Cite' and ref.citations and #ref.citations > 0 then
    local cite_id = ref.citations[1].id
    if starts_with('fig:', cite_id) or
       starts_with('tbl:', cite_id) or
       starts_with('lst:', cite_id) or
       starts_with('sec:', cite_id) or
       starts_with('eq:', cite_id) then
      return true
    end
  end

  return false
end

--- Check if a space should be added after a reference
---
--- Sometimes a space is needed after a citation or reference to separate
--- it from the following text. This function checks if a space should be
--- added.
---
--- @param ref table The reference element
--- @param next table The next inline element
--- @return boolean True if a space should be added
local function no_space_after_ref(ref, next)
  if not ref then
    return false
  end

  -- Check if this is a reference that might need a space after it
  local is_ref = false
  if ref.t == 'RawInline' and starts_with('~', ref.text) then
    is_ref = true
  elseif ref.t == 'Cite' and ref.citations and #ref.citations > 0 then
    local cite_id = ref.citations[1].id
    if starts_with('fig:', cite_id) or
       starts_with('tbl:', cite_id) or
       starts_with('lst:', cite_id) or
       starts_with('sec:', cite_id) or
       starts_with('eq:', cite_id) then
      is_ref = true
    end
  end

  if not is_ref then
    return false
  end

  -- Add space if the next element is text that doesn't start with punctuation
  if next and next.t == 'Str' then
    -- Don't add space if the next character is punctuation
    if not next.text:match("^[,%.;:…%)，。、：；'\"》]") then
      return true
    end
  end

  return false
end

--- Process inline elements to fix spacing around citations and references
---
--- This function walks through inline elements and:
--- 1. Removes spaces before citations/references
--- 2. Adds spaces after citations/references when needed
---
--- @param inlines table List of inline elements
--- @return table The processed list of inline elements
local function proc_inlines(inlines)
  -- Process from end to beginning to avoid index issues when removing elements
  for i = #inlines - 1, 1, -1 do
    -- Remove spaces before references
    if is_space_before_ref(inlines[i], inlines[i + 1]) then
      inlines:remove(i)
    end

    -- Add spaces after references if needed
    if no_space_after_ref(inlines[i], inlines[i + 1]) then
      inlines:insert(i + 1, pandoc.Space())
    end
  end

  return inlines
end

-- ============================================================================
-- Filter Registration
-- ============================================================================

return {
  { RawInline = fix_rawinline },
  { Image = proc_image },
  { Inlines = proc_inlines },
}
