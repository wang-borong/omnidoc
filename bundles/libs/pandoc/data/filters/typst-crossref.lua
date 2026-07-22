-- Helper to identify our refs
function is_crossref(id)
  return id:match("^tbl:") or id:match("^fig:") or id:match("^lst:") or id:match("^sec:") or id:match("^eq:")
end

function Inlines(inlines)
  local new_inlines = {}

  -- Step 1: Process embedded references in Str elements
  for _, el in ipairs(inlines) do
    if el.t == 'Str' then
      local text = el.text
      local last_pos = 1
      -- Find pattern @prefix:id.
      -- Prefixes: tbl, fig, lst, sec, eq.
      -- capture start, end, full_match
      for s, match, e in text:gmatch("()(@[%w]+:[%w%-_:]+)()") do
         local id_part = match:sub(2)
         if is_crossref(id_part) then
             -- Append text before
             if s > last_pos then
               table.insert(new_inlines, pandoc.Str(text:sub(last_pos, s - 1)))
             end
             -- Append Ref
             table.insert(new_inlines, pandoc.RawInline('typst', match))
             last_pos = e
         end
      end
      -- Append remaining text
      if last_pos <= #text then
         table.insert(new_inlines, pandoc.Str(text:sub(last_pos)))
      end
    else
      table.insert(new_inlines, el)
    end
  end

  inlines = new_inlines
  new_inlines = {} -- reuse or just modify in place

  -- Step 2: Traverse backwards for Space removal and Math Labels
  -- We now operate on the expanded list where "参考@eq:1" is now [Str "参考", RawInline "@eq:1"]

  for i = #inlines, 1, -1 do
    local current = inlines[i]
    local next_el = inlines[i+1]

    local is_target_ref = false
    if next_el then
      if next_el.t == 'Cite' then
         if #next_el.citations == 1 and is_crossref(next_el.citations[1].id) then
           is_target_ref = true
         end
      elseif next_el.t == 'RawInline' and next_el.format == 'typst' then
         local txt = next_el.text
         if txt:match("^@tbl:") or txt:match("^@fig:") or txt:match("^@lst:") or txt:match("^@sec:") or txt:match("^@eq:") then
            is_target_ref = true
         end
      end
    end

    if (current.t == 'Space' or current.t == 'SoftBreak') and is_target_ref then
       local prev = inlines[i-1]
       if prev and prev.t == 'Str' then
          local text = prev.text
          if #text > 0 then
             local last_byte = string.byte(text, -1)
             if last_byte > 127 then
                  table.remove(inlines, i)
             end
          end
       end
    end

    -- Check for Math Labels {#eq:id}
    if current.t == 'Str' and current.text:match("^{#eq:[%w%-_:]+}$") then
       local prev = inlines[i-1]
       if prev then
          if prev.t == 'Math' then
             -- Math + Label
             inlines[i] = pandoc.RawInline('typst', '<' .. current.text:sub(3, -2) .. '>')
          elseif prev.t == 'Space' or prev.t == 'SoftBreak' then
             local prev2 = inlines[i-2]
             if prev2 and prev2.t == 'Math' then
                 -- Math + Space + Label --> Remove Space, Convert Label
                 table.remove(inlines, i-1)
                 inlines[i-1] = pandoc.RawInline('typst', '<' .. current.text:sub(3, -2) .. '>')
             end
          end
       end
    end
  end

  return inlines
end

function Cite(el)
  if #el.citations == 1 then
    local cite = el.citations[1]
    local id = cite.id
    if is_crossref(id) then
      return pandoc.RawInline('typst', '@' .. id)
    end
  end
  return nil
end

function CodeBlock(el)
  if el.attributes.caption then
    local caption = el.attributes.caption
    local id = el.identifier

    el.identifier = ""
    el.attributes.caption = nil

    return {
       pandoc.RawBlock('typst', '#figure(\n kind: raw,\n caption: [' .. caption .. '],\n)[\n'),
       el,
       pandoc.RawBlock('typst', '] <' .. id .. '>')
    }
  end
end
