--- display-math.lua - Add a semantic wrapper around standalone formulas.
---
--- Pandoc represents a display formula as a Para containing one DisplayMath
--- inline.  HTML MathML elements with display="block" commonly occupy the
--- full line, so text-align on the math element itself does not center its
--- contents consistently.  The wrapper lets OmniDoc's base CSS center only
--- standalone formulas without changing inline mathematics.

local is_portable_html = FORMAT:match('html') or FORMAT:match('epub')

function Para(para)
  if not is_portable_html or #para.content ~= 1 then
    return nil
  end

  local item = para.content[1]
  if item.t ~= 'Math' or item.mathtype ~= 'DisplayMath' then
    return nil
  end

  return pandoc.Div(
    { para },
    pandoc.Attr('', { 'omni-display-math' })
  )
end

return {
  { Para = Para }
}
