--- fonts-and-alignment.lua - Sets fonts and alignment in LaTeX documents
---
--- This filter applies LaTeX font and alignment commands to Span, Div, and Link
--- elements based on their CSS classes. It supports a wide range of LaTeX
--- font commands, font sizes, text alignments, and ulem package styles.
---
--- Features:
---   - Font types: bold, italic, monospace, sans-serif, serif, smallcaps, etc.
---   - Font sizes: tiny, small, normal, large, huge, etc.
---   - Text alignments: center, flushleft, flushright, raggedleft, raggedright
---   - Ulem styles: underline, strikeout, dashuline, dotuline, etc. (optional)
---   - Supports both full names and abbreviations (e.g., "bf" = "bold")
---
--- Usage:
---   [Bold text]{.bold}
---   [Italic text]{.italic}
---   [Monospace]{.monospace}
---
---   :::{.center}
---   Centered content
---   :::
---
---   [Underlined]{.uline}
---
--- Metadata options:
---   - ulem_styles: Set to true to enable ulem package styles (underline, strikeout, etc.)
---                  Requires: \usepackage[normalem]{ulem} in LaTeX preamble
---
--- Copyright: © 2021-2022 Nandakumar Chandrasekhar
--- License:   MIT - see LICENSE for details

-- Requires pandoc 2.17 or later
PANDOC_VERSION:must_be_at_least '2.17'

-- ============================================================================
-- LaTeX Command Definitions
-- ============================================================================

-- LaTeX font type commands
-- Format: {class_name = {inline_command, block_command}}
-- First value is for inline elements (Span), second for block elements (Div)
LATEX_FONT_TYPES = {
  bold = {'textbf', 'bfseries'},
  emphasis = {'emph', 'em'},
  italic = {'textit', 'itshape'},
  lower = {'lowercase', nil},
  medium = {'textmd', 'mdseries'},
  monospace = {'texttt', 'ttfamily'},
  normalfont = {'textnormal', 'normalfont'},
  sans = {'textsf', 'sffamily'},
  serif = {'textrm', 'rmfamily'},
  slanted = {'textsl', 'slshape'},
  smallcaps = {'textsc', 'scshape'},
  upper = {'uppercase', nil},
  upright = {'textup', 'upshape'},
  -- Abbreviations
  bf = {'textbf', 'bfseries'},
  em = {'emph', 'em'},
  it = {'textit', 'itshape'},
  md = {'textmd', 'mdseries'},
  tt = {'texttt', 'ttfamily'},
  nf = {'textnormal', 'normalfont'},
  sf = {'textsf', 'sffamily'},
  rm = {'textrm', 'rmfamily'},
  sc = {'textsc', 'scshape'},
  sl = {'textsl', 'slshape'},
  up = {'textup', 'upshape'},
}

-- LaTeX font size commands
-- Format: {class_name = {inline_command, block_command}}
LATEX_FONT_SIZES = {
  tiny = {'tiny', 'tiny'},
  xxsmall = {'scriptsize', 'scriptsize'},
  xsmall = {'footnotesize', 'footnotesize'},
  small = {'small', 'small'},
  normal = {'normalsize', 'normalsize'},
  large = {'large', 'large'},
  xlarge = {'Large', 'Large'},
  xxlarge = {'LARGE', 'LARGE'},
  huge = {'huge', 'huge'},
}

-- LaTeX text alignment commands
-- Format: {class_name = {inline_command, block_command}}
-- Note: Alignment typically only applies to block elements
LATEX_TEXT_ALIGNMENTS = {
  center = {nil, 'center'},
  flushright = {nil, 'flushright'},
  flushleft = {nil, 'flushleft'},
  centering = {nil, 'centering'},
  raggedleft = {nil, 'raggedleft'},
  raggedright = {nil, 'raggedright'},
}

-- LaTeX ulem package styles (underline, strikeout, etc.)
-- Format: {class_name = {inline_command, block_command}}
-- Note: ulem styles only apply to inline elements
LATEX_ULEM_STYLES = {
  dashuline = {'dashuline', nil},
  dotuline = {'dotuline', nil},
  uline = {'uline', nil},
  uuline = {'uuline', nil},
  uwave = {'uwave', nil},
  sout = {'sout', nil},
  xout = {'xout', nil},
  -- Abbreviations
  dau = {'dashuline', nil},
  dou = {'dotuline', nil},
  so = {'sout', nil},
  u = {'uline', nil},
  uu = {'uuline', nil},
  uw = {'uwave', nil},
  xo = {'xout', nil},
}

-- Pandoc element type to raw code function mapping
RAW_CODE_FUNCTION = {
  Span = pandoc.RawInline,
  Div = pandoc.RawBlock
}

-- ============================================================================
-- Library Validation
-- ============================================================================

-- Validate that required pandoc libraries are available
local p = assert(pandoc, "Cannot find the pandoc library")
if not ('table' == type(p)) then
  error("Expected variable pandoc to be a table")
end
local utils = assert(pandoc.utils, "Cannot find the pandoc.utils library")
local List = assert(pandoc.List, "Cannot find the pandoc.List class")

-- ============================================================================
-- Helper Functions
-- ============================================================================

--- Convert a value to string, handling pandoc objects
---
--- @param val any The value to convert
--- @param accept_bool boolean Whether to accept boolean values
--- @return string The string representation
local function stringify(val, accept_bool)
  -- Try using pandoc.utils.stringify first
  local status, retval = pcall(utils.stringify, val)
  if status and retval then
    return retval
  end
  
  -- Fallback to type checking
  local val_type = utils.type(val)
  if ((val_type == "string") or (val_type == "number") or
      (accept_bool and (val_type == "boolean"))) then
    return tostring(val)
  else
    error("Cannot convert to string " .. val_type)
  end
end

--- Convert a value to boolean
---
--- Supports: "true", "false", "yes", "no" (case-insensitive)
---
--- @param val any The value to convert
--- @return boolean|nil The boolean value, or nil if invalid
local get_bool_val = {
  ["true"] = true,
  ["false"] = false,
  yes = true,
  no = false
}

local function boolify(val)
  local str = stringify(val, true):lower()  -- Case insensitive
  return get_bool_val[str]
end

-- ============================================================================
-- LaTeX Command Construction
-- ============================================================================

--- Table to hold constructed LaTeX commands, organized by element type
local latex_cmd_for_tags = {
  Span = {},
  Div = {}
}

--- Construct LaTeX command strings for a class
---
--- @param class string The CSS class name
--- @param span_code string|nil LaTeX command for inline elements
--- @param div_code string|nil LaTeX command for block elements
--- @param span_end_code boolean Whether span command needs closing brace
local function construct_latex_cmd(class, span_code, div_code, span_end_code)
  -- Construct inline (Span) command
  if span_code then
    if not span_end_code then
      -- Command without braces (e.g., \bfseries)
      latex_cmd_for_tags.Span[class] = {'\\' .. span_code .. ' ', nil}
    else
      -- Command with braces (e.g., \textbf{...})
      latex_cmd_for_tags.Span[class] = {'\\' .. span_code .. '{', '}'}
    end
  end
  
  -- Construct block (Div) command
  if div_code then
    latex_cmd_for_tags.Div[class] = {
      '\\begin{' .. div_code .. '}',
      '\\end{' .. div_code .. '}'
    }
  end
end

--- Create LaTeX commands from a style definition table
---
--- @param styles_list table Style definitions (font types, sizes, etc.)
--- @param span_end_code boolean Whether span commands need closing braces
local function create_latex_codes(styles_list, span_end_code)
  -- Default span_end_code to false if not specified
  span_end_code = (span_end_code == nil) and false or span_end_code
  
  for class, latex_codes in pairs(styles_list) do
    -- Skip empty entries
    if next(latex_codes) then
      local span_code = latex_codes[1]
      local div_code = latex_codes[2]
      construct_latex_cmd(class, span_code, div_code, span_end_code)
    end
  end
end

-- Initialize LaTeX commands for font types, sizes, and alignments
create_latex_codes(LATEX_FONT_TYPES, true)      -- Font types need braces
create_latex_codes(LATEX_FONT_SIZES)            -- Font sizes don't need braces
create_latex_codes(LATEX_TEXT_ALIGNMENTS)       -- Alignments don't need braces

-- ============================================================================
-- Element Processing
-- ============================================================================

--- Apply LaTeX commands to an element based on its classes
---
--- This function processes Span, Div, and Link elements and applies LaTeX
--- font and alignment commands based on their CSS classes. Commands are
--- applied in reverse order to maintain the original class order in output.
---
--- @param elem table The element to process (Span, Div, or Link)
--- @return table The processed element with LaTeX commands applied
local function handler(elem)
  local tag = elem.tag

  -- Links can use Span styling
  if tag == "Link" then
    tag = "Span"
  end

  -- Get the appropriate raw code function and command table
  local raw = RAW_CODE_FUNCTION[tag]
  local code_for_class = latex_cmd_for_tags[tag]
  local classes = elem.classes

  -- Process classes in reverse order to maintain original order in output
  for i = #classes, 1, -1 do
    if code_for_class[classes[i]] then
      local code = code_for_class[classes[i]]
      local begin_code = code[1]  -- LaTeX code to insert before content
      local end_code = code[2]    -- LaTeX code to insert after content

      -- Insert begin code at the start
      table.insert(elem.content, 1, raw('latex', begin_code))
      
      -- Insert end code at the end (if needed)
      if end_code then
        table.insert(elem.content, raw('latex', end_code))
      end
    end
  end
  
  return elem
end

-- ============================================================================
-- Meta Processing
-- ============================================================================

--- Process metadata to enable ulem styles if requested
---
--- If ulem_styles is enabled, this function:
--- 1. Adds \usepackage[normalem]{ulem} to header-includes
--- 2. Creates LaTeX commands for ulem styles
---
--- @param meta table The document metadata
--- @return table|nil The metadata (or nil if unchanged)
local function Meta(meta)
  -- Get ulem_styles setting from metadata
  local uline_styles_metavar = meta.ulem_styles

  -- Default to false if not specified
  if uline_styles_metavar == nil then
    uline_styles_metavar = false
  end

  -- Convert to boolean
  uline_styles_metavar = boolify(uline_styles_metavar)

  -- Validate the result
  if uline_styles_metavar == nil then
    error("Expected meta.ulem_styles should be 'true', 'false', or unset")
  end

  -- If ulem styles are requested, set them up
  if uline_styles_metavar then
    -- Get or create header-includes list
    local includes = meta['header-includes']
    includes = includes or List({})
    
    -- Ensure it's a List
    if 'List' ~= utils.type(includes) then
      includes = List({includes})
    end
    
    -- Add ulem package
    includes:insert(p.RawBlock('latex', "\\usepackage[normalem]{ulem}"))
    meta['header-includes'] = includes

    -- Create LaTeX commands for ulem styles
    create_latex_codes(LATEX_ULEM_STYLES, true)

    return meta
  end
  
  -- No ulem styles requested, return nil to leave metadata unchanged
  return nil
end

-- ============================================================================
-- Filter Registration
-- ============================================================================

-- Meta must be processed first to set up ulem styles if needed
-- Then process Div, Link, and Span elements
return {
  {Meta = Meta},
  {Div = handler,
   Link = handler,
   Span = handler}
}
