local issues = {}
local has_level_one_heading = false

function Header(element)
  if element.level == 1 then
    has_level_one_heading = true
  end
end

function Str(element)
  if element.text:match("TODO") or element.text:match("FIXME") then
    table.insert(issues, "TODO/FIXME marker: " .. element.text)
  end
end

function Pandoc(document)
  if not has_level_one_heading then
    table.insert(issues, "document has no level-one heading")
  end

  for _, issue in ipairs(issues) do
    pandoc.log.warn("quality-gate: " .. issue)
  end
  document.meta["omnidoc-quality-issues"] =
    pandoc.MetaString(tostring(#issues))

  local fail = document.meta["quality-gate-fail"]
  if fail == true or pandoc.utils.stringify(fail):lower() == "true" then
    if #issues > 0 then
      error("quality-gate rejected the document with " .. #issues .. " issue(s)")
    end
  end
  return document
end
