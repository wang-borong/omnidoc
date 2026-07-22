--- admonition.lua - Render OmniDoc semantic blocks across output formats.
---
--- Canonical Markdown:
---   ::: {.admonition .warning title="Warning"}
---   Verify the supply polarity before powering the circuit.
---   :::
---
--- Supported kinds: note, tip, important, warning, error, question, answer,
--- example, exercise, and solution.

local is_latex = FORMAT:match('latex') or FORMAT:match('beamer')
local utils = pandoc.utils
local language = 'en'

local kinds = {
  'note', 'tip', 'important', 'warning', 'error', 'question', 'answer',
  'example', 'exercise', 'solution',
}

local titles = {
  en = {
    note = 'Note', tip = 'Tip', important = 'Important', warning = 'Warning',
    error = 'Error', question = 'Question', answer = 'Answer', example = 'Example',
    exercise = 'Exercise', solution = 'Solution',
  },
  zh = {
    note = '说明', tip = '提示', important = '重要', warning = '警告',
    error = '错误', question = '问题', answer = '回答', example = '示例',
    exercise = '练习', solution = '解答',
  },
}

local marks = {
  note = 'i', tip = '+', important = '*', warning = '!', error = '!',
  question = '?', answer = 'A', example = 'E', exercise = 'X', solution = 'S',
}

local supported_kind = {}
for _, kind in ipairs(kinds) do
  supported_kind[kind] = true
end

local function has_class(el, class_name)
  for _, class in ipairs(el.classes) do
    if class == class_name then
      return true
    end
  end
  return false
end

local function kind_for(el)
  if not has_class(el, 'admonition') then
    return nil
  end
  for _, class in ipairs(el.classes) do
    if supported_kind[class] then
      return class
    end
  end
  return 'note'
end

local function escape_latex(value)
  local replacements = {
    ['\\'] = '\\textbackslash{}', ['{'] = '\\{', ['}'] = '\\}',
    ['#'] = '\\#', ['$'] = '\\$', ['%'] = '\\%', ['&'] = '\\&',
    ['_'] = '\\_', ['^'] = '\\textasciicircum{}', ['~'] = '\\textasciitilde{}',
  }
  return (value:gsub('[\\{}#$%%&_^~]', replacements))
end

local function append_latex_package(meta)
  local package = pandoc.MetaBlocks({
    pandoc.RawBlock('latex', '\\usepackage{omni-blocks}')
  })
  local includes = meta['header-includes']
  if includes == nil then
    meta['header-includes'] = pandoc.MetaList({package})
  elseif utils.type(includes) == 'List' then
    table.insert(includes, package)
  else
    meta['header-includes'] = pandoc.MetaList({includes, package})
  end
end

function Meta(meta)
  local lang = meta.lang and utils.stringify(meta.lang):lower() or ''
  language = lang:match('^zh') and 'zh' or 'en'
  if is_latex then
    append_latex_package(meta)
  end
  return meta
end

function Div(el)
  local kind = kind_for(el)
  if not kind then
    return nil
  end

  local title = el.attributes.title or titles[language][kind]
  el.attributes.title = nil

  if is_latex then
    local blocks = {
      pandoc.RawBlock('latex', string.format(
        '\\begin{OmniAdmonition}{%s}{%s}', kind, escape_latex(title)
      )),
    }
    for _, block in ipairs(el.content) do
      table.insert(blocks, block)
    end
    table.insert(blocks, pandoc.RawBlock('latex', '\\end{OmniAdmonition}'))
    return blocks
  end

  local retained = {}
  for _, class in ipairs(el.classes) do
    if not supported_kind[class] and class ~= 'admonition' then
      table.insert(retained, class)
    end
  end
  el.classes = pandoc.List({'admonition', kind})
  for _, class in ipairs(retained) do
    el.classes:insert(class)
  end
  el.attributes['data-title'] = title
  el.attributes['data-kind'] = kind
  el.attributes['data-mark'] = marks[kind]
  return el
end

return {
  {Meta = Meta},
  {Div = Div},
}
