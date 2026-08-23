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
---   - Preserves Unicode quotes and readable math in PDF bookmarks
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
-- PDF Bookmark Processing
-- ============================================================================

local math_commands = {
  alpha = 'α', beta = 'β', gamma = 'γ', delta = 'δ', epsilon = 'ε',
  varepsilon = 'ϵ', zeta = 'ζ', eta = 'η', theta = 'θ', vartheta = 'ϑ',
  iota = 'ι', kappa = 'κ', lambda = 'λ', mu = 'μ', nu = 'ν', xi = 'ξ',
  omicron = 'ο', pi = 'π', varpi = 'ϖ', rho = 'ρ', varrho = 'ϱ',
  sigma = 'σ', varsigma = 'ς', tau = 'τ', upsilon = 'υ', phi = 'φ',
  varphi = 'ϕ', chi = 'χ', psi = 'ψ', omega = 'ω',
  Gamma = 'Γ', Delta = 'Δ', Theta = 'Θ', Lambda = 'Λ', Xi = 'Ξ', Pi = 'Π',
  Sigma = 'Σ', Upsilon = 'Υ', Phi = 'Φ', Psi = 'Ψ', Omega = 'Ω',
  pm = '±', mp = '∓', times = '×', cdot = '·', div = '÷', ast = '∗',
  le = '≤', leq = '≤', ge = '≥', geq = '≥', ne = '≠', neq = '≠',
  approx = '≈', sim = '∼', simeq = '≃', equiv = '≡', propto = '∝',
  to = '→', rightarrow = '→', leftarrow = '←', leftrightarrow = '↔',
  Rightarrow = '⇒', Leftarrow = '⇐', Leftrightarrow = '⇔', mapsto = '↦',
  infty = '∞', partial = '∂', nabla = '∇', sum = '∑', prod = '∏',
  int = '∫', oint = '∮', parallel = '∥', perp = '⊥', angle = '∠',
  ['in'] = '∈', notin = '∉', subset = '⊂', subseteq = '⊆',
  supset = '⊃', supseteq = '⊇', cup = '∪', cap = '∩',
  forall = '∀', exists = '∃', neg = '¬', land = '∧', lor = '∨',
  ell = 'ℓ', hbar = 'ℏ', degree = '°', prime = '′',
}

local transparent_math_commands = {
  mathrm = true, mathit = true, mathbf = true, mathsf = true, mathtt = true,
  mathcal = true, mathbb = true, mathfrak = true, operatorname = true,
  text = true, textrm = true, textit = true, textbf = true,
  boldsymbol = true, bm = true, ensuremath = true,
}

local accent_math_commands = {
  bar = '\204\132', overline = '\204\133', hat = '\204\130',
  tilde = '\204\131', dot = '\204\135', ddot = '\204\136', vec = '\226\131\151',
}

local subscript_chars = {
  ['0'] = '₀', ['1'] = '₁', ['2'] = '₂', ['3'] = '₃', ['4'] = '₄',
  ['5'] = '₅', ['6'] = '₆', ['7'] = '₇', ['8'] = '₈', ['9'] = '₉',
  ['+'] = '₊', ['-'] = '₋', ['='] = '₌', ['('] = '₍', [')'] = '₎',
  a = 'ₐ', e = 'ₑ', h = 'ₕ', i = 'ᵢ', j = 'ⱼ', k = 'ₖ', l = 'ₗ',
  m = 'ₘ', n = 'ₙ', o = 'ₒ', p = 'ₚ', r = 'ᵣ', s = 'ₛ', t = 'ₜ', x = 'ₓ',
}

local superscript_chars = {
  ['0'] = '⁰', ['1'] = '¹', ['2'] = '²', ['3'] = '³', ['4'] = '⁴',
  ['5'] = '⁵', ['6'] = '⁶', ['7'] = '⁷', ['8'] = '⁸', ['9'] = '⁹',
  ['+'] = '⁺', ['-'] = '⁻', ['='] = '⁼', ['('] = '⁽', [')'] = '⁾',
  a = 'ᵃ', b = 'ᵇ', c = 'ᶜ', d = 'ᵈ', e = 'ᵉ', f = 'ᶠ', g = 'ᵍ',
  h = 'ʰ', i = 'ⁱ', j = 'ʲ', k = 'ᵏ', l = 'ˡ', m = 'ᵐ', n = 'ⁿ',
  o = 'ᵒ', p = 'ᵖ', r = 'ʳ', s = 'ˢ', t = 'ᵗ', u = 'ᵘ', v = 'ᵛ',
  w = 'ʷ', x = 'ˣ', y = 'ʸ', z = 'ᶻ',
}

local bookmark_quotes = {
  ['“'] = '``',
  ['”'] = "''",
  ['‘'] = '`',
  ['’'] = "'",
}

local quote_characters = {'“', '”', '‘', '’'}

local function next_utf8_character(value, index)
  local first = value:byte(index)
  if not first then
    return nil, index
  end
  local length = 1
  if first >= 0xF0 then
    length = 4
  elseif first >= 0xE0 then
    length = 3
  elseif first >= 0xC0 then
    length = 2
  end
  return value:sub(index, index + length - 1), index + length
end

local function fallback_script_text(value, marker)
  -- PDF outline titles are plain strings and cannot apply arbitrary
  -- subscript/superscript styling. Script-position parentheses communicate
  -- the intended relationship without leaking raw TeX markers, including for
  -- glyphs that have no Unicode script form (C_C becomes C₍C₎ and r_\pi
  -- becomes r₍π₎).
  if marker == '_' then
    return '₍' .. value .. '₎'
  end
  return '⁽' .. value .. '⁾'
end

local function script_text(value, characters, fallback_marker)
  local converted = {}
  local index = 1
  while index <= #value do
    local character
    character, index = next_utf8_character(value, index)
    local mapped = characters[character]
    if not mapped and character:match('^[A-Z]$') then
      mapped = characters[character:lower()]
    end
    if not mapped then
      return fallback_script_text(value, fallback_marker)
    end
    converted[#converted + 1] = mapped
  end
  return table.concat(converted)
end

local parse_math_sequence
local parse_math_atom

local function parse_math_command(value, index)
  local name = value:match('^([A-Za-z]+)', index)
  if not name then
    local escaped
    escaped, index = next_utf8_character(value, index)
    if escaped == ',' or escaped == '!' or escaped == ';' or escaped == ':' then
      return '', index
    elseif escaped == ' ' then
      return ' ', index
    end
    return escaped or '', index
  end

  index = index + #name
  if value:sub(index, index) == ' ' then
    index = index + 1
  end

  if math_commands[name] then
    return math_commands[name], index
  end

  if transparent_math_commands[name] then
    return parse_math_atom(value, index)
  end

  if name == 'frac' or name == 'dfrac' or name == 'tfrac' then
    local numerator
    numerator, index = parse_math_atom(value, index)
    local denominator
    denominator, index = parse_math_atom(value, index)
    return numerator .. '⁄' .. denominator, index
  end

  if name == 'sqrt' then
    if value:sub(index, index) == '[' then
      local close = value:find(']', index + 1, true)
      if close then
        index = close + 1
      end
    end
    local radicand
    radicand, index = parse_math_atom(value, index)
    return '√' .. radicand, index
  end

  if name == 'left' or name == 'right' then
    return '', index
  end

  if accent_math_commands[name] then
    local accented
    accented, index = parse_math_atom(value, index)
    return accented .. accent_math_commands[name], index
  end

  -- Unknown commands remain readable without leaking raw TeX syntax into the
  -- bookmark. Any following group is handled normally by the main parser.
  return name, index
end

parse_math_atom = function(value, index)
  while value:sub(index, index):match('%s') do
    index = index + 1
  end
  if value:sub(index, index) == '{' then
    return parse_math_sequence(value, index + 1, '}')
  end
  if value:sub(index, index) == '\\' then
    return parse_math_command(value, index + 1)
  end
  local character
  character, index = next_utf8_character(value, index)
  return character or '', index
end

parse_math_sequence = function(value, index, stop_character)
  local result = {}
  while index <= #value do
    local byte = value:sub(index, index)
    if stop_character and byte == stop_character then
      return table.concat(result), index + 1
    elseif byte == '\\' then
      local converted
      converted, index = parse_math_command(value, index + 1)
      result[#result + 1] = converted
    elseif byte == '_' or byte == '^' then
      local script
      script, index = parse_math_atom(value, index + 1)
      if byte == '_' then
        result[#result + 1] = script_text(script, subscript_chars, '_')
      else
        result[#result + 1] = script_text(script, superscript_chars, '^')
      end
    elseif byte == '{' then
      local group
      group, index = parse_math_sequence(value, index + 1, '}')
      result[#result + 1] = group
    elseif byte == '}' then
      return table.concat(result), index + 1
    elseif byte == '$' then
      index = index + 1
    elseif byte == '~' then
      result[#result + 1] = ' '
      index = index + 1
    else
      local character
      character, index = next_utf8_character(value, index)
      result[#result + 1] = character
    end
  end
  return table.concat(result), index
end

local function math_to_bookmark(value)
  local converted = parse_math_sequence(value, 1, nil)
  return converted:gsub('%s+', ' '):gsub('^%s+', ''):gsub('%s+$', '')
end

local function bookmark_inline(typeset, fallback)
  return pandoc.List({
    latex('\\texorpdfstring{' .. typeset .. '}{'),
    pandoc.Str(fallback),
    latex('}'),
  })
end

local function split_bookmark_quotes(value)
  local result = pandoc.List()
  local cursor = 1
  local changed = false
  while cursor <= #value do
    local quote
    local quote_start
    for _, candidate in ipairs(quote_characters) do
      local start = value:find(candidate, cursor, true)
      if start and (not quote_start or start < quote_start) then
        quote = candidate
        quote_start = start
      end
    end
    if not quote_start then
      result:insert(pandoc.Str(value:sub(cursor)))
      break
    end
    if quote_start > cursor then
      result:insert(pandoc.Str(value:sub(cursor, quote_start - 1)))
    end
    local wrapped = bookmark_inline(bookmark_quotes[quote], quote)
    for _, part in ipairs(wrapped) do
      result:insert(part)
    end
    cursor = quote_start + #quote
    changed = true
  end
  if not changed then
    return nil
  end
  return result
end

local function bookmark_math(math)
  return bookmark_inline('\\(' .. math.text .. '\\)', math_to_bookmark(math.text))
end

local function process_header_inlines(inlines)
  local result = pandoc.List()
  for _, inline in ipairs(inlines) do
    if inline.t == 'Math' then
      for _, part in ipairs(bookmark_math(inline)) do
        result:insert(part)
      end
    elseif inline.t == 'Str' then
      local split = split_bookmark_quotes(inline.text)
      if split then
        for _, part in ipairs(split) do
          result:insert(part)
        end
      else
        result:insert(inline)
      end
    else
      if inline.content then
        inline.content = process_header_inlines(inline.content)
      end
      result:insert(inline)
    end
  end
  return result
end

local function fix_pdf_bookmark(header)
  if not FORMAT:match('latex') then
    return nil
  end
  header.content = process_header_inlines(header.content)
  return header
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
  { Header = fix_pdf_bookmark },
  { RawInline = fix_rawinline },
  { Image = proc_image },
  { Inlines = proc_inlines },
}
