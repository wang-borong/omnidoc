--- Apply OmniDoc/theme metadata as defaults without overriding document data.
---
--- OmniDoc passes generic defaults as `omnidoc-default-<key>` metadata and a
--- Chinese cross-reference file as `omnidoc-zh-crossref-yaml`. This filter
--- runs before the other managed filters and removes the private transport
--- keys after filling only missing public metadata.

local utils = pandoc.utils
local default_prefix = 'omnidoc-default-'
local zh_crossref_key = 'omnidoc-zh-crossref-yaml'

local function Meta(meta)
  local defaults = {}
  local private_keys = {}

  for key, value in pairs(meta) do
    if key:sub(1, #default_prefix) == default_prefix then
      local target = key:sub(#default_prefix + 1)
      if target ~= '' and meta[target] == nil then
        defaults[target] = value
      end
      table.insert(private_keys, key)
    end
  end

  for key, value in pairs(defaults) do
    meta[key] = value
  end

  local zh_crossref = meta[zh_crossref_key]
  if zh_crossref ~= nil and meta.crossrefYaml == nil then
    local lang = meta.lang and utils.stringify(meta.lang):lower() or ''
    if lang:match('^zh') then
      meta.crossrefYaml = zh_crossref
    end
  end
  if zh_crossref ~= nil then
    table.insert(private_keys, zh_crossref_key)
  end

  for _, key in ipairs(private_keys) do
    meta[key] = nil
  end

  return meta
end

return {{Meta = Meta}}
