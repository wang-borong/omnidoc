--- emoji.lua - Render Unicode emoji safely in LaTeX/PDF output.

if not FORMAT:match('latex') then
  return {}
end

local emoji_presentation_bmp = {
  {0x231A, 0x231B}, {0x23E9, 0x23EC}, {0x23F0, 0x23F0},
  {0x23F3, 0x23F3}, {0x25FD, 0x25FE}, {0x2614, 0x2615},
  {0x2648, 0x2653}, {0x267F, 0x267F}, {0x2693, 0x2693},
  {0x26A1, 0x26A1}, {0x26AA, 0x26AB}, {0x26BD, 0x26BE},
  {0x26C4, 0x26C5}, {0x26CE, 0x26CE}, {0x26D4, 0x26D4},
  {0x26EA, 0x26EA}, {0x26F2, 0x26F3}, {0x26F5, 0x26F5},
  {0x26FA, 0x26FA}, {0x26FD, 0x26FD}, {0x2705, 0x2705},
  {0x270A, 0x270B}, {0x2728, 0x2728}, {0x274C, 0x274C},
  {0x274E, 0x274E}, {0x2753, 0x2755}, {0x2757, 0x2757},
  {0x2795, 0x2797}, {0x27B0, 0x27B0}, {0x27BF, 0x27BF},
  {0x2B1B, 0x2B1C}, {0x2B50, 0x2B50}, {0x2B55, 0x2B55},
}

local function in_ranges(codepoint, ranges)
  for _, range in ipairs(ranges) do
    if codepoint >= range[1] and codepoint <= range[2] then
      return true
    end
  end
  return false
end

local function is_regional_indicator(codepoint)
  return codepoint >= 0x1F1E6 and codepoint <= 0x1F1FF
end

local function is_emoji_presentation(codepoint)
  return in_ranges(codepoint, emoji_presentation_bmp) or
         (codepoint >= 0x1F000 and codepoint <= 0x1FAFF)
end

local function is_text_emoji_candidate(codepoint)
  return codepoint == 0x00A9 or codepoint == 0x00AE or
         codepoint == 0x203C or codepoint == 0x2049 or
         codepoint == 0x2122 or codepoint == 0x2139 or
         (codepoint >= 0x2194 and codepoint <= 0x21FF) or
         (codepoint >= 0x2300 and codepoint <= 0x2BFF) or
         is_emoji_presentation(codepoint)
end

local function is_modifier(codepoint)
  return codepoint >= 0x1F3FB and codepoint <= 0x1F3FF
end

local function codepoints(value)
  local result = {}
  for _, codepoint in utf8.codes(value) do
    table.insert(result, codepoint)
  end
  return result
end

local function append_text(result, buffer)
  if #buffer > 0 then
    table.insert(result, pandoc.Str(table.concat(buffer)))
    for index = #buffer, 1, -1 do
      buffer[index] = nil
    end
  end
end

local function emoji_code(sequence, strip_variation_selector)
  local hex = {}
  for _, codepoint in ipairs(sequence) do
    if not strip_variation_selector or codepoint ~= 0xFE0F then
      table.insert(hex, string.format('%x', codepoint))
    end
  end
  return table.concat(hex, '-')
end

local function consume_suffix(points, index, sequence)
  while points[index] == 0xFE0F or is_modifier(points[index] or 0) do
    table.insert(sequence, points[index])
    index = index + 1
  end
  return index
end

local function consume_emoji(points, index)
  local first = points[index]
  local sequence = {}

  if (first == 0x23 or first == 0x2A or
      (first >= 0x30 and first <= 0x39)) then
    local next_index = index + 1
    if points[next_index] == 0xFE0F then
      next_index = next_index + 1
    end
    if points[next_index] == 0x20E3 then
      for cursor = index, next_index do
        table.insert(sequence, points[cursor])
      end
      return sequence, next_index + 1
    end
    return nil, index
  end

  if not is_emoji_presentation(first) and
     not (is_text_emoji_candidate(first) and points[index + 1] == 0xFE0F) then
    return nil, index
  end

  table.insert(sequence, first)
  index = consume_suffix(points, index + 1, sequence)

  if is_regional_indicator(first) and is_regional_indicator(points[index] or 0) then
    table.insert(sequence, points[index])
    return sequence, index + 1
  end

  if first == 0x1F3F4 then
    while points[index] and points[index] >= 0xE0020 and points[index] <= 0xE007E do
      table.insert(sequence, points[index])
      index = index + 1
    end
    if points[index] == 0xE007F then
      table.insert(sequence, points[index])
      index = index + 1
    end
  end

  while points[index] == 0x200D and points[index + 1] do
    table.insert(sequence, points[index])
    table.insert(sequence, points[index + 1])
    index = consume_suffix(points, index + 2, sequence)
  end

  return sequence, index
end

local function replace_emoji(element)
  local points = codepoints(element.text)
  local result = {}
  local buffer = {}
  local index = 1
  local replaced = false

  while index <= #points do
    local sequence, next_index = consume_emoji(points, index)
    if sequence then
      append_text(result, buffer)
      table.insert(result, pandoc.RawInline(
        'latex',
        '\\omnidocEmoji{' .. emoji_code(sequence, false) .. '}{' ..
          emoji_code(sequence, true) .. '}'
      ))
      replaced = true
      index = next_index
    else
      table.insert(buffer, utf8.char(points[index]))
      index = index + 1
    end
  end

  append_text(result, buffer)
  if replaced and #result > 0 then
    return result
  end
  return nil
end

return {{Str = replace_emoji}}
