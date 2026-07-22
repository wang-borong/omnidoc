--- Append OmniDoc-managed LaTeX header files without replacing project metadata.

if not FORMAT:match('latex') and not FORMAT:match('beamer') then
  return {}
end

local utils = pandoc.utils

local function append_header(meta, file_path)
  local file, err = io.open(file_path, 'r')
  if not file then
    error(string.format("cannot read OmniDoc LaTeX header '%s': %s", file_path, tostring(err)))
  end
  local content = file:read('*all')
  file:close()
  local header = pandoc.MetaBlocks({pandoc.RawBlock('latex', content)})
  local includes = meta['header-includes']
  if includes == nil then
    meta['header-includes'] = pandoc.MetaList({header})
  elseif utils.type(includes) == 'List' then
    table.insert(includes, header)
  else
    meta['header-includes'] = pandoc.MetaList({includes, header})
  end
end

local function Meta(meta)
  local headers = {}
  for key, value in pairs(meta) do
    if key == 'omnidoc-default-latex-header' or
       key:match('^omnidoc%-theme%-latex%-header%-%d+$') then
      table.insert(headers, {key, utils.stringify(value)})
    end
  end
  table.sort(headers, function(left, right) return left[1] < right[1] end)
  for _, entry in ipairs(headers) do
    append_header(meta, entry[2])
    meta[entry[1]] = nil
  end
  return meta
end

return {{Meta = Meta}}
