--- ipa.lua – Convert IPA (International Phonetic Alphabet) spans to LaTeX
---
--- This filter converts Span elements with the 'ipa' class to LaTeX \ipa{}
--- commands, which are used for typesetting phonetic transcriptions.
---
--- Usage:
---   [Hello]{.ipa}
---
---   Output (LaTeX only):
---     \ipa{Hello}
---
--- Note: This filter only affects LaTeX/PDF output. For other formats,
---       the span is left unchanged.

-- ============================================================================
-- Span Processing
-- ============================================================================

--- Convert IPA spans to LaTeX \ipa commands
---
--- This function checks if a Span element has the 'ipa' class and converts
--- it to LaTeX \ipa{} command for LaTeX/PDF output. For other output formats,
--- the span is left unchanged.
---
--- @param el table The Span element
--- @return table|nil The processed span with LaTeX \ipa command, or nil if unchanged
function Span(el)
  -- Check if this is an IPA span
  if el.attr.classes[1] == "ipa" then
    -- Only convert for LaTeX output formats
    if FORMAT == "tex" or FORMAT == "latex" or FORMAT == "pdf" then
      return {
        pandoc.RawInline("latex", "\\ipa{"),
        el,
        pandoc.RawInline("latex", "}")
      }
    end
  end
  
  -- Return nil to leave unchanged for non-LaTeX formats or non-IPA spans
  return nil
end
