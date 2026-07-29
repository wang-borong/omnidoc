local inputs = {}
for index = 1, #arg do
  if arg[index] ~= "--" then
    table.insert(inputs, arg[index])
  end
end

if #inputs == 0 then
  io.stderr:write("word-count requires at least one Markdown input path\n")
  os.exit(2)
end

local function read_file(path)
  local handle, open_error = io.open(path, "rb")
  if handle == nil then
    error("cannot open " .. path .. ": " .. tostring(open_error))
  end
  local content = handle:read("*a")
  handle:close()
  return content
end

local function count_words(text)
  local count = 0
  for _ in text:gmatch("%S+") do
    count = count + 1
  end
  return count
end

for _, path in ipairs(inputs) do
  local document = pandoc.read(read_file(path), "markdown")
  local text = pandoc.utils.stringify(document)
  local characters = utf8.len(text) or #text
  io.write(string.format("%s\t%d\t%d\n", path, count_words(text), characters))
end
