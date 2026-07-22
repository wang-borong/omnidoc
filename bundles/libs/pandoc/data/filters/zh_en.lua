--- zh_en.lua – Handle Chinese/English bilingual content
---
--- This filter handles bilingual documents with Chinese and English content.
--- It can filter content based on the language specified in metadata, or
--- show both languages based on configuration.
---
--- Features:
---   - Filters Div blocks and Headers by language class
---   - Supports 'zh' (Chinese) and 'en' (English) classes
---   - Can show both languages or filter to one
---   - Configurable via metadata
---
--- Usage:
---   :::{.zh}
---  这是中文内容。
---   :::
---
---   :::{.en}
---  This is English content.
---   :::
---
---   # Heading {.zh}
---
---   # Heading {.en}
---
--- Metadata options:
---   - ext-zh-en: Set to 'zh' (Chinese only), 'en' (English only), or 'both' (both languages)
---                Default behavior depends on the setting.

-- ============================================================================
-- Helper Functions
-- ============================================================================

--- Check if a value is in a table
--- @param t table The table to search
--- @param val any The value to find
--- @return boolean True if the value is in the table
local function in_table(t, val)
  for _, v in pairs(t) do
    if v == val then
      return true
    end
  end
  return false
end

-- ============================================================================
-- Supported Language Classes
-- ============================================================================

local zh_en_classes = {'zh', 'en'}

-- ============================================================================
-- Document Processing
-- ============================================================================

--- Process the document and filter content by language
---
--- This function processes the document and filters Div blocks and Headers
--- based on their language class and the metadata setting. It wraps language-specific
--- Div blocks in LaTeX environments and filters Headers based on the language setting.
---
--- @param doc table The Pandoc document
--- @return table The processed document with filtered content
function Pandoc(doc)
  local new_blocks = {}
  local language_setting = doc.meta['ext-zh-en']
  
  -- Process each block in the document
  for _, el in pairs(doc.blocks) do
    -- Handle Div blocks with language classes
    if el.t == "Div" and in_table(zh_en_classes, el.attr.classes[1]) then
      local lang_class = el.attr.classes[1]
      
      -- Wrap the Div in LaTeX environment
      table.insert(new_blocks, pandoc.RawBlock("latex", 
        string.format("\\begin{%s}", lang_class)))
      table.insert(new_blocks, el)
      table.insert(new_blocks, pandoc.RawBlock("latex", 
        string.format("\\end{%s}", lang_class)))
    
    -- Handle Headers with language classes
    elseif el.t == "Header" and in_table(zh_en_classes, el.attr.classes[1]) then
      local lang_class = el.attr.classes[1]
      
      -- Determine if this header should be included
      local should_include = false
      
      if language_setting == 'both' then
        -- For 'both', prefer Chinese headers (both languages use same structure)
        if lang_class == 'zh' then
          should_include = true
        end
      elseif lang_class == language_setting then
        -- Include if language matches the setting
        should_include = true
      end
      
      if should_include then
        table.insert(new_blocks, el)
      end
      -- If should_include is false, the header is skipped
    
    -- All other blocks are included as-is
    else
      table.insert(new_blocks, el)
    end
  end
  
  -- Return the processed document with filtered blocks
  return pandoc.Pandoc(new_blocks, doc.meta)
end
