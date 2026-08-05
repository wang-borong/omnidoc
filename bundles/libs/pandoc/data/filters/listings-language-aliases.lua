-- Supply LaTeX listings language names for fenced-code classes that Pandoc
-- otherwise emits without a language option. Explicit language attributes
-- always win, so projects can override these aliases when needed.

local aliases = {
  markdown = "Markdown",
  md = "Markdown",
  shell = "bash",
  yaml = "YAML",
  yml = "YAML",
}

function CodeBlock(block)
  if block.attributes.language ~= nil then
    return nil
  end

  for _, class in ipairs(block.classes) do
    local language = aliases[string.lower(class)]
    if language ~= nil then
      block.attributes.language = language
      return block
    end
  end

  return nil
end
