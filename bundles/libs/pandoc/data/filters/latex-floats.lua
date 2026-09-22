--- Keep LaTeX figures close to their source without forcing every figure to H.
---
--- The default "bounded" policy source-anchors OmniDoc-generated diagrams and
--- places barriers before level-1/2 headings. Ordinary figures still use
--- LaTeX's top/bottom/page placement inside that bounded region.

if not FORMAT:match('latex') and not FORMAT:match('beamer') then
  return {}
end

local utils = pandoc.utils
local policy = 'bounded'
local barrier_level = 2

local function stringify(value)
  if type(value) == 'string' then
    return value
  end
  return utils.stringify(value)
end

local function normalized_policy(value)
  local candidate = stringify(value):lower():gsub('^%s+', ''):gsub('%s+$', '')
  local valid = {
    bounded = true,
    strict = true,
    section = true,
    diagram = true,
    off = true,
    none = true,
    ['false'] = true,
    ['true'] = true,
  }
  if valid[candidate] then
    if candidate == 'none' or candidate == 'false' then
      return 'off'
    end
    return candidate == 'true' and 'bounded' or candidate
  end
  io.stderr:write(string.format(
    "Warning: unknown omnidoc-float-policy '%s'; using bounded\n",
    candidate
  ))
  return 'bounded'
end

local function has_class(element, expected)
  for _, class_name in ipairs(element.classes or {}) do
    if class_name == expected then
      return true
    end
  end
  return false
end

local function barrier()
  return pandoc.RawBlock('latex', '\\FloatBarrier')
end

local function anchored_figure(figure)
  return {
    pandoc.RawBlock('latex', '\\OmniDiagramFloatBegin'),
    figure,
    pandoc.RawBlock('latex', '\\OmniDiagramFloatEnd'),
  }
end

local function Meta(meta)
  if meta['omnidoc-float-policy'] ~= nil then
    policy = normalized_policy(meta['omnidoc-float-policy'])
  end
  if meta['omnidoc-float-barrier-level'] ~= nil then
    local configured = tonumber(stringify(meta['omnidoc-float-barrier-level']))
    if configured and configured >= 0 then
      barrier_level = math.floor(configured)
    else
      io.stderr:write(
        'Warning: omnidoc-float-barrier-level must be a non-negative integer; using 2\n'
      )
      barrier_level = 2
    end
  end
  return nil
end

local function Header(header)
  if policy == 'off' or policy == 'diagram' or barrier_level == 0 then
    return nil
  end
  if header.level <= barrier_level then
    return {barrier(), header}
  end
  return nil
end

local function Figure(figure)
  if policy == 'off' or policy == 'section' or has_class(figure, 'float-unbounded') then
    return nil
  end
  local is_diagram = has_class(figure, 'omnidoc-diagram') or
                     (figure.attributes and
                      figure.attributes['data-omnidoc-diagram'] ~= nil)
  if is_diagram then
    return anchored_figure(figure)
  end
  if policy == 'strict' then
    return {figure, barrier()}
  end
  return nil
end

return {
  {Meta = Meta},
  {Header = Header, Figure = Figure},
}
