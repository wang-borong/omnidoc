--- lineblock.lua – Convert LineBlock to LaTeX lineblock environment
---
--- This filter converts Pandoc LineBlock elements to LaTeX \begin{lineblock}
--- environments. LineBlock preserves line breaks in the original source,
--- which is useful for poetry, addresses, or other text where line breaks
--- are significant.
---
--- Usage:
---   No special syntax needed - any LineBlock in the document will be converted.
---
--- Example:
---   Input (Pandoc LineBlock):
---     Line 1
---     Line 2
---     Line 3
---
---   Output (LaTeX):
---     \begin{lineblock}
---     Line 1
---     Line 2
---     Line 3
---     \end{lineblock}

-- ============================================================================
-- LineBlock Processing
-- ============================================================================

--- Convert LineBlock to LaTeX lineblock environment
---
--- This function wraps a LineBlock element in LaTeX \begin{lineblock} and
--- \end{lineblock} commands. The content of the LineBlock is preserved
--- as-is between these commands.
---
--- @param block table The LineBlock element
--- @return table List of blocks containing the LaTeX environment
function LineBlock(block)
  return {
    pandoc.RawBlock("latex", "\\begin{lineblock}"),
    block,
    pandoc.RawBlock("latex", "\\end{lineblock}")
  }
end
