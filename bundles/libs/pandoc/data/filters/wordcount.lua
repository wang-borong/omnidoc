--- wordcount.lua – Count words and characters in a document
---
--- This filter counts words and characters in a document, with special
--- handling for Chinese and other non-ASCII characters.
---
--- Features:
---   - Counts total words (ASCII and non-ASCII)
---   - Counts characters (with and without spaces)
---   - Separates counts for ASCII and non-ASCII words
---   - Supports different counting modes via metadata
---
--- Usage:
---   Run with: pandoc --lua-filter=wordcount.lua document.md
---
--- Metadata options:
---   - wordcount: Set to "process-anyway", "process", or "convert" to continue
---                processing after counting (default: exits after counting)
---
--- Output:
---   Prints word and character counts to stdout and exits (unless configured otherwise)

-- ============================================================================
-- Configuration Variables
-- ============================================================================

--- Total word count (ASCII and non-ASCII combined)
local words = 0

--- Non-ASCII character count (e.g., Chinese characters)
local noascii = 0

--- ASCII word count (English words)
local ascii_words = 0

--- Total character count (excluding spaces)
local characters = 0

--- Total character count (including spaces)
local characters_and_spaces = 0

--- Whether to continue processing after counting
local process_anyway = false

-- ============================================================================
-- Helper Functions
-- ============================================================================

--- Count non-ASCII characters (e.g., Chinese characters) in text
---
--- This function uses UTF-8 encoding detection to count multi-byte characters.
--- In UTF-8, Chinese and other CJK characters are encoded as 2-4 byte sequences.
---
--- @param text string The text to count
--- @return number The count of non-ASCII characters
local function count_noascii(text)
  local count = 0
  -- Match UTF-8 multi-byte sequences (Chinese, Japanese, Korean, etc.)
  -- Pattern matches: [194-244][128-191] (2-byte), [224-244][128-191][128-191] (3-byte), etc.
  for _ in text:gmatch("[%z\194-\244][\128-\191]*") do
    count = count + 1
  end
  return count
end

--- Count ASCII words (English words) in text
---
--- This function counts sequences of alphanumeric characters, hyphens,
--- underscores, and digits as words.
---
--- @param text string The text to count
--- @return number The count of ASCII words
local function count_ascii_words(text)
  local count = 0
  -- Match sequences of letters, hyphens, digits, and underscores
  for _ in text:gmatch("[%a%-%d_]+") do
    count = count + 1
  end
  return count
end

-- ============================================================================
-- Counting Functions
-- ============================================================================

--- Word counting filter
local wordcount = {
  --- Process Str (text) elements
  --- @param el table The Str element
  Str = function(el)
    -- Count words (only if the text contains non-punctuation characters)
    if el.text:match("%P") then
      words = words + 1
    end
    
    -- Count characters
    characters = characters + utf8.len(el.text)
    characters_and_spaces = characters_and_spaces + utf8.len(el.text)

    -- Count non-ASCII and ASCII words separately
    noascii = noascii + count_noascii(el.text)
    ascii_words = ascii_words + count_ascii_words(el.text)
  end,

  --- Process Space elements
  Space = function()
    characters_and_spaces = characters_and_spaces + 1
  end,

  --- Process Code (inline code) elements
  --- @param el table The Code element
  Code = function(el)
    -- Count words in code (non-whitespace sequences)
    local _, word_count = el.text:gsub("%S+", "")
    words = words + word_count
    
    -- Count characters (excluding spaces)
    local text_nospace = el.text:gsub("%s", "")
    characters = characters + utf8.len(text_nospace)
    
    -- Count characters including spaces
    characters_and_spaces = characters_and_spaces + utf8.len(el.text)
    
    -- Count non-ASCII and ASCII words
    noascii = noascii + count_noascii(el.text)
    ascii_words = ascii_words + count_ascii_words(el.text)
  end,

  --- Process CodeBlock elements
  --- @param el table The CodeBlock element
  CodeBlock = function(el)
    -- Count words in code block (non-whitespace sequences)
    local _, word_count = el.text:gsub("%S+", "")
    words = words + word_count
    
    -- Count characters (excluding spaces)
    local text_nospace = el.text:gsub("%s", "")
    characters = characters + utf8.len(text_nospace)
    
    -- Count characters including spaces
    characters_and_spaces = characters_and_spaces + utf8.len(el.text)
    
    -- Count non-ASCII and ASCII words
    noascii = noascii + count_noascii(el.text)
    ascii_words = ascii_words + count_ascii_words(el.text)
  end
}

-- ============================================================================
-- Meta Processing
-- ============================================================================

--- Check if wordcount metadata indicates processing should continue
--- @param meta table The document metadata
function Meta(meta)
  if meta.wordcount and (
    meta.wordcount == "process-anyway" or
    meta.wordcount == "process" or
    meta.wordcount == "convert"
  ) then
    process_anyway = true
  end
end

-- ============================================================================
-- Document Processing
-- ============================================================================

--- Process the document and print word counts
--- @param el table The Pandoc document element
--- @return table The document (if process_anyway is true)
function Pandoc(el)
  -- Walk through all blocks and count words
  -- Skip metadata, just count body content
  pandoc.walk_block(pandoc.Div(el.blocks), wordcount)
  
  -- Print word and character counts
  print('非英文字数：', noascii)
  print('英文字数：', ascii_words)
  print('合计总字数：', noascii + ascii_words)
  
  -- Exit unless configured to continue processing
  if not process_anyway then
    os.exit(0)
  end
  
  -- Return document unchanged if continuing
  return el
end
