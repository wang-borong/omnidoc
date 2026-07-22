--[[
diagram-generator.lua – Create images and figures from code blocks

This Lua filter converts code blocks with specific classes into images or figures.
It supports multiple diagram generation engines:
  - PlantUML: UML diagrams and flowcharts
  - GraphViz: Graph visualization (dot, neato, etc.)
  - TikZ: LaTeX graphics
  - Python: Custom image generation scripts
  - Asymptote: Mathematical graphics
  - Bitfield: OmniDoc JSON bitfield descriptions

Features:
  - Automatic format selection based on output format (SVG, PNG, PDF)
  - Support for figure captions and cross-references
  - Configurable tool paths via metadata or environment variables
  - Automatic file caching using content hashing
  - Media bag integration for embedded resources

Usage:
  ```{.plantuml caption="UML Diagram"}
  @startuml
  Alice -> Bob: Hello
  @enduml
  ```

  ```{.graphviz caption="Graph"}
  digraph G {
    A -> B
  }
  ```

  ```{.bitfield caption="Register layout" width="100%"}
  {
    "bits": 8,
    "entries": [
      {"name": "VALUE", "bits": 7},
      {"name": "READY", "bits": 1}
    ]
  }
  ```

Metadata options:
  - plantuml_path / plantumlPath: Path to PlantUML executable
  - inkscape_path / inkscapePath: Path to Inkscape executable
  - python_path / pythonPath: Path to Python executable
  - activate_python_path / activatePythonPath: Python virtual environment activation script
  - java_path / javaPath: Path to Java executable
  - dot_path / dotPath: Path to Graphviz dot executable
  - pdflatex_path / pdflatexPath: Path to pdflatex executable
  - asymptote_path / asymptotePath: Path to Asymptote executable
  - omnidoc_path / omnidocPath: Path to the OmniDoc executable

Copyright: © 2018-2021 John MacFarlane <jgm@berkeley.edu>,
           2018 Florian Schätzig <florian@schaetzig.de>,
           2019 Thorsten Sommer <contact@sommer-engineering.com>,
           2019-2021 Albert Krewinkel <albert+pandoc@zeitkraut.de>
License:   MIT – see LICENSE file for details
]]

-- Module pandoc.system is required and was added in version 2.7.3
PANDOC_VERSION:must_be_at_least '2.7.3'

local system = require 'pandoc.system'
local utils = require 'pandoc.utils'
local path = require 'pandoc.path'

local depfile_path = nil
local diagram_dependencies = {}

local function record_dependency(file_path)
  if not file_path or file_path == '' or file_path:find('[\r\n]') then
    return
  end
  local resolved = file_path
  if path.is_relative(resolved) then
    resolved = path.join({system.get_working_directory(), resolved})
  end
  diagram_dependencies[path.normalize(resolved)] = true
end

local function write_depfile(doc)
  if not depfile_path or depfile_path == '' then
    return doc
  end
  local dependencies = {}
  for dependency, _ in pairs(diagram_dependencies) do
    table.insert(dependencies, dependency)
  end
  table.sort(dependencies)
  local file, err = io.open(depfile_path, 'w')
  if not file then
    io.stderr:write(string.format(
      "Warning: cannot write diagram depfile '%s': %s\n",
      depfile_path, tostring(err)
    ))
    return doc
  end
  file:write('# omnidoc-depfile-v1\n')
  for _, dependency in ipairs(dependencies) do
    file:write(dependency, '\n')
  end
  file:close()
  return doc
end

-- ============================================================================
-- Helper Functions
-- ============================================================================

--- Convert a value to string, handling both strings and pandoc objects
--- @param s string|table The value to convert
--- @return string The string representation
local function stringify(s)
  if type(s) == 'string' then
    return s
  end
  return utils.stringify(s)
end

-- ============================================================================
-- System Utilities
-- ============================================================================

local with_temporary_directory = system.with_temporary_directory
local with_working_directory = system.with_working_directory

-- ============================================================================
-- Configuration Variables
-- ============================================================================

-- Tool paths (can be overridden via metadata or environment variables)
local plantuml_path = os.getenv("PLANTUML") or "plantuml"
local inkscape_path = os.getenv("INKSCAPE") or "inkscape"
local python_path = os.getenv("PYTHON") or "python"
local python_activate_path = os.getenv("PYTHON_ACTIVATE")
local java_path = os.getenv("JAVA_HOME")
if java_path then
  java_path = java_path .. package.config:sub(1, 1) .. "bin" ..
              package.config:sub(1, 1) .. "java"
else
  java_path = "java"
end
local dot_path = os.getenv("DOT") or "dot"
local pdflatex_path = os.getenv("PDFLATEX") or "pdflatex"
local asymptote_path = os.getenv("ASYMPTOTE") or "asy"
local omnidoc_path = os.getenv("OMNIDOC_BIN") or "omnidoc"
local ngspice_path = os.getenv("NGSPICE") or "ngspice"

-- Output format and MIME type
-- Default is SVG (vector graphics), but changes based on output format
local filetype = "svg"
local mimetype = "image/svg+xml"
local bitfield_filetype = "svg"
local bitfield_mimetype = "image/svg+xml"

-- Determine output format based on pandoc output format
-- Some formats don't support SVG well, so we use PNG or PDF instead
if FORMAT == "docx" or FORMAT == "pptx" or FORMAT == "rtf" then
  filetype = "png"
  mimetype = "image/png"
elseif FORMAT == "pdf" or FORMAT == "latex" then
  filetype = "pdf"
  mimetype = "application/pdf"
end

if FORMAT == "pdf" or FORMAT == "latex" then
  bitfield_filetype = "pdf"
  bitfield_mimetype = "application/pdf"
end

-- ============================================================================
-- Meta Processing
-- ============================================================================

--- Extract tool paths from document metadata
---
--- This function processes metadata to override default tool paths.
--- Supports both hyphenated (plantuml_path) and camelCase (plantumlPath) names.
---
--- @param meta table The document metadata
--- @return table|nil The metadata (or nil if unchanged)
function Meta(meta)
  local generic_depfile = meta['omnidoc-depfile-diagram-generator']
  depfile_path = generic_depfile and stringify(generic_depfile) or nil
  diagram_dependencies = {}
  -- Update tool paths from metadata if provided
  plantuml_path = stringify(
    meta.plantuml_path or meta.plantumlPath or plantuml_path
  )
  inkscape_path = stringify(
    meta.inkscape_path or meta.inkscapePath or inkscape_path
  )
  python_path = stringify(
    meta.python_path or meta.pythonPath or python_path
  )
  python_activate_path = meta.activate_python_path or
                         meta.activatePythonPath or
                         python_activate_path
  python_activate_path = python_activate_path and stringify(python_activate_path)
  java_path = stringify(
    meta.java_path or meta.javaPath or java_path
  )
  dot_path = stringify(
    meta.path_dot or meta.dotPath or dot_path
  )
  pdflatex_path = stringify(
    meta.pdflatex_path or meta.pdflatexPath or pdflatex_path
  )
  asymptote_path = stringify(
    meta.asymptote_path or meta.asymptotePath or asymptote_path
  )
  omnidoc_path = stringify(
    meta.omnidoc_path or meta.omnidocPath or omnidoc_path
  )
  ngspice_path = stringify(
    meta.ngspice_path or meta.ngspicePath or ngspice_path
  )
  
  return nil
end

local function helper_script(name)
  local filter_dir = path.directory(PANDOC_SCRIPT_FILE or '')
  return path.normalize(path.join({filter_dir, '..', 'scripts', name}))
end

local function python_helper(code, output_type, helper, suffix, extra_args)
  return with_temporary_directory("omnidoc-diagram", function(tmpdir)
    local source = path.join({tmpdir, 'source.' .. suffix})
    local output = path.join({tmpdir, 'output.' .. output_type})
    local source_file = assert(io.open(source, 'w'))
    source_file:write(code)
    source_file:close()
    local args = {helper_script(helper), source, output}
    for _, value in ipairs(extra_args or {}) do
      table.insert(args, value)
    end
    pandoc.pipe(python_path, args, '')
    local rendered = assert(io.open(output, 'rb'))
    local data = rendered:read('*all')
    rendered:close()
    return data
  end)
end

local function circuit(code, output_type)
  return python_helper(code, output_type, 'render-circuit.py', 'py')
end

local function spiceplot(code, output_type)
  local ok, spec = pcall(pandoc.json.decode, code)
  if ok and type(spec) == 'table' and type(spec.netlist) == 'string' then
    record_dependency(spec.netlist)
  end
  return python_helper(
    code, output_type, 'render-spiceplot.py', 'json',
    {'--ngspice', ngspice_path}
  )
end

-- ============================================================================
-- Diagram Generators
-- ============================================================================

--- Generate image from PlantUML code
---
--- @param puml string The PlantUML source code
--- @param filetype string The output file type (svg, png, pdf)
--- @return string|nil The generated image data, or nil on error
local function plantuml(puml, filetype)
  return pandoc.pipe(
    "plantuml",
    {"-t" .. filetype, "-pipe", "-charset", "UTF8"},
    puml
  )
end

--- Generate image from GraphViz code
---
--- @param code string The GraphViz/DOT source code
--- @param filetype string The output file type
--- @return string|nil The generated image data, or nil on error
local function graphviz(code, filetype)
  return pandoc.pipe(dot_path, {"-T" .. filetype}, code)
end

--- Render an OmniDoc bitfield JSON code block through the native renderer.
--- Reusing the CLI keeps fenced blocks and standalone JSON files on the same
--- schema, validation, and SVG layout implementation.
local function bitfield(code, output_type)
  return with_temporary_directory("omnidoc-bitfield", function(tmpdir)
    local source = tmpdir .. "/bitfield.json"
    local output = tmpdir .. "/bitfield." .. output_type
    local file = assert(io.open(source, "wb"))
    file:write(code)
    file:close()

    local command = omnidoc_path
    local args = {
      "figure", "bitfield", source,
      "--format", output_type,
      "--output", tmpdir,
      "--force"
    }
    if package.config:sub(1, 1) == "/" then
      command = "env"
      args = {
        "-u", "OMNIDOC_LATEX_RECORDER_ENGINE",
        "-u", "OMNIDOC_LATEX_RECORDER_DEPFILE",
        omnidoc_path,
        table.unpack(args)
      }
    end
    pandoc.pipe(command, args, "")

    local rendered = assert(io.open(output, "rb"))
    local data = rendered:read("*all")
    rendered:close()
    return data
  end)
end

--- LaTeX template for TikZ compilation
local tikz_template = [[
\documentclass{standalone}
\usepackage{tikz}
%% begin: additional packages
%s
%% end: additional packages
\begin{document}
%s
\end{document}
]]

--- Create Inkscape converter function for PDF to other formats
---
--- This function returns a converter function that uses Inkscape to convert
--- PDF files to other formats (SVG, PNG). It handles both Inkscape 1.x and
--- older versions with different command-line syntax.
---
--- @param filetype string Target format (svg or png)
--- @return function|nil Converter function(pdf_file, outfile), or nil if unsupported
local function convert_with_inkscape(filetype)
  -- Check Inkscape version to determine command syntax
  local ok, inkscape_v_string = pcall(
    pandoc.pipe,
    inkscape_path,
    {"--version"},
    ""
  )
  if not ok or not inkscape_v_string then
    return nil
  end
  
  local inkscape_v_major = inkscape_v_string:gmatch("([0-9]*)%.")()
  local isv1 = (tonumber(inkscape_v_major) or 0) >= 1

  local function build_args(pdf_file, outfile)
    if isv1 and filetype == 'png' then
      return {pdf_file, "-o", outfile, "--export-type=png", "--export-dpi=300"}
    end
    if isv1 and filetype == 'svg' then
      return {pdf_file, "-o", outfile, "--export-type=svg", "--export-plain-svg"}
    end
    if filetype == 'png' then
      return {"--without-gui", "--file=" .. pdf_file, "--export-png=" .. outfile, "--export-dpi=300"}
    end
    if filetype == 'svg' then
      return {"--without-gui", "--file=" .. pdf_file, "--export-plain-svg=" .. outfile}
    end
    return nil
  end

  if filetype == 'png' then
    -- Supported.
  elseif filetype == 'svg' then
    -- Supported.
  else
    return nil  -- Unsupported format
  end

  -- Return converter function
  return function(pdf_file, outfile)
    local args = build_args(pdf_file, outfile)
    if not args then
      error(string.format("Don't know how to convert pdf to %s.", filetype))
    end

    local success, err = pcall(pandoc.pipe, inkscape_path, args, "")
    if not success then
      error("Inkscape conversion failed: " .. tostring(err))
    end
  end
end

--- Compile TikZ code to an image
---
--- This function compiles TikZ LaTeX code to an image by:
--- 1. Creating a standalone LaTeX document
--- 2. Compiling it with pdflatex
--- 3. Converting the PDF to the target format using Inkscape
---
--- @param src string The TikZ source code
--- @param filetype string The output file type
--- @param additional_packages string|nil Additional LaTeX packages to include
--- @return string|nil The generated image data, or nil on error
local function tikz2image(src, filetype, additional_packages)
  local convert = convert_with_inkscape(filetype)
  if not convert then
    error(string.format("Don't know how to convert pdf to %s.", filetype))
  end
  
  return with_temporary_directory("tikz2image", function(tmpdir)
    return with_working_directory(tmpdir, function()
      -- Define file names
      local file_template = "%s/tikz-image.%s"
      local tikz_file = file_template:format(tmpdir, "tex")
      local pdf_file = file_template:format(tmpdir, "pdf")
      local outfile = file_template:format(tmpdir, filetype)

      -- Build and write the LaTeX document
      local f = io.open(tikz_file, 'w')
      if not f then
        error("Could not open TikZ file for writing")
      end
      f:write(tikz_template:format(additional_packages or '', src))
      f:close()

      -- Execute the LaTeX compiler
      pandoc.pipe(pdflatex_path, {'-output-directory', tmpdir, tikz_file}, '')

      -- Convert PDF to target format
      convert(pdf_file, outfile)

      -- Read the generated image
      local img_data
      local r = io.open(outfile, 'rb')
      if r then
        img_data = r:read("*all")
        r:close()
      else
        error("Could not read generated image file")
      end

      return img_data
    end)
  end)
end

--- Generate image from Python code
---
--- This function executes Python code to generate an image. The Python code
--- should use $FORMAT$ and $DESTINATION$ placeholders which will be replaced
--- with the actual format and output file path.
---
--- @param code string The Python source code
--- @param filetype string The output file type
--- @return string|nil The generated image data, or nil on error
local function py2image(code, filetype)
  -- Define temporary files
  local outfile = string.format('%s.%s', os.tmpname(), filetype)
  local pyfile = os.tmpname()

  -- Replace placeholders in Python code
  local extended_code = string.gsub(code, "%$FORMAT%$", filetype)
  extended_code = string.gsub(extended_code, "%$DESTINATION%$", outfile)

  -- Write the Python code to a file
  local f = io.open(pyfile, 'w')
  if not f then
    error("Could not open Python file for writing")
  end
  f:write(extended_code)
  f:close()

  if python_activate_path then
    io.stderr:write(
      "Warning: activate_python_path is ignored by diagram-generator.lua; " ..
      "set python_path to the virtualenv Python executable instead.\n"
    )
  end

  local success, py_err = pcall(pandoc.pipe, python_path, {pyfile}, "")
  if not success then
    os.remove(pyfile)
    os.remove(outfile)
    error("Python diagram generation failed: " .. tostring(py_err))
  end

  -- Read the generated image
  local r = io.open(outfile, 'rb')
  local img_data = nil

  if r then
    img_data = r:read("*all")
    r:close()
  else
    io.stderr:write(string.format("File '%s' could not be opened", outfile))
    error('Could not create image from python code.')
  end

  -- Clean up temporary files
  os.remove(pyfile)
  os.remove(outfile)

  return img_data
end

--- Generate image from Asymptote code
---
--- @param code string The Asymptote source code
--- @param filetype string The output file type (svg or png)
--- @return string|nil The generated image data, or nil on error
local function asymptote(code, filetype)
  if filetype ~= 'svg' and filetype ~= 'png' then
    error(string.format("Conversion to %s not implemented", filetype))
  end
  
  return with_temporary_directory(
    "asymptote",
    function(tmpdir)
      return with_working_directory(
        tmpdir,
        function()
          local asy_file = "pandoc_diagram.asy"
          local svg_file = "pandoc_diagram.svg"
          local f = io.open(asy_file, 'w')
          if not f then
            error("Could not open Asymptote file for writing")
          end
          f:write(code)
          f:close()

          -- Generate SVG
          pandoc.pipe(asymptote_path, {"-f", "svg", "-o", "pandoc_diagram", asy_file}, "")

          -- Read result (SVG or converted PNG)
          local r
          if filetype == 'svg' then
            r = io.open(svg_file, 'rb')
          else
            local png_file = "pandoc_diagram.png"
            convert_with_inkscape("png")(svg_file, png_file)
            r = io.open(png_file, 'rb')
          end

          local img_data
          if r then
            img_data = r:read("*all")
            r:close()
          else
            error("could not read asymptote result file")
          end
          return img_data
        end
      )
    end
  )
end

-- ============================================================================
-- File Management
-- ============================================================================

--- Write binary data to a file
---
--- @param file_path string Path to the file
--- @param binary_data string The binary data to write
--- @return boolean True on success, false on error
local function write_binary_data_to_file(file_path, binary_data)
  local file, err = io.open(file_path, "wb")
  if not file then
    error(string.format("Error opening file '%s': %s", file_path, err or "unknown error"))
    return false
  end

  file:write(binary_data)
  file:close()
  return true
end

--- Create directory if needed and write file when its content changed
---
--- This function creates the figures directory if it doesn't exist and
--- avoids rewriting identical image data. Identifier-based filenames are
--- stable, so merely checking for existence would leave stale diagrams after
--- the source block changes.
---
--- @param directory string Directory path
--- @param file_name string File name
--- @param binary_data string Binary data to write
--- @return boolean True on success, false on error
local function create_directory_and_write_file(directory, file_name, binary_data)
  local full_path = directory .. "/" .. file_name

  -- Create directory if necessary
  local created = false
  if system.make_directory then
    created = pcall(system.make_directory, directory, true)
  end
  if not created then
    local ok, mkdir_err = pcall(pandoc.pipe, "mkdir", {"-p", directory}, "")
    if not ok then
      error("Could not create directory '" .. directory .. "': " .. tostring(mkdir_err))
    end
  end

  -- Preserve the existing file only when it already contains the rendered
  -- bytes. A fixed Pandoc identifier deliberately produces a fixed filename,
  -- but it must not turn that filename into a stale content cache.
  local existing = io.open(full_path, "rb")
  if existing then
    local existing_data = existing:read("*all")
    existing:close()
    if existing_data == binary_data then
      return true
    end
  end

  if not write_binary_data_to_file(full_path, binary_data) then
    return false
  end

  return true
end

-- ============================================================================
-- Code Block Processing
-- ============================================================================

--- Map of supported diagram generators
local converters = {
  plantuml = plantuml,
  graphviz = graphviz,
  tikz = tikz2image,
  py2image = py2image,
  asymptote = asymptote,
  bitfield = bitfield,
  circuit = circuit,
  spiceplot = spiceplot,
}

--- Get the converter function for a code block class
---
--- @param class_name string The class name from the code block
--- @return function|nil The converter function, or nil if not found
local function get_converter(class_name)
  return converters[class_name]
end

--- Generate image and save it to file
---
--- @param block table The CodeBlock element
--- @param converter function The converter function to use
--- @return string|nil Binary image data, or nil on error
local function generate_image(block, converter, output_type)
  -- Call the converter to generate the image
  local success, img = pcall(converter, block.text,
                             output_type, block.attributes["additionalPackages"] or nil)

  -- Handle errors
  if not (success and img) then
    io.stderr:write(tostring(img or "no image data has been returned."))
    io.stderr:write('\n')
    error('Image conversion failed. Aborting.')
  end

  return img
end

--- Generate filename for the image
---
--- @param block table The CodeBlock element
--- @param img_data string Binary image data
--- @return string The filename
local function generate_filename(block, img_data, output_type)
  -- Use identifier if present, otherwise hash
  if block.identifier and block.identifier ~= "" then
    local id = block.identifier:gsub('.*:', '', 1)
    return id .. '.' .. output_type
  else
    return pandoc.sha1(img_data) .. "." .. output_type
  end
end

--- Save image and store in media bag
---
--- @param fname string The filename
--- @param img_data string Binary image data
local function save_image(fname, img_data, media_type)
  -- Write image file to figures directory
  create_directory_and_write_file('figures', fname, img_data)

  -- Store in media bag for embedded resources
  pandoc.mediabag.insert(fname, media_type, img_data)
end

--- Create image/figure element for Pandoc 2.x
---
--- @param block table The CodeBlock element
--- @param fname string The filename
--- @param caption table Blocks representing the caption
--- @param alt table Inlines representing the alt text
--- @return table Para block containing the image
local function create_image_pandoc2(block, fname, caption, alt)
  local title = #caption > 0 and "fig:" or ""
  local img_attr = {
    id = block.identifier,
    name = block.attributes.name,
    width = block.attributes.width,
    height = block.attributes.height
  }
  local img_obj = pandoc.Image(alt, fname, title, img_attr)
  return pandoc.Para{img_obj}
end

--- Create figure element for Pandoc 3.x
---
--- @param block table The CodeBlock element
--- @param fname string The filename
--- @param caption table Blocks representing the caption
--- @param alt table Inlines representing the alt text
--- @return table Figure element
local function create_figure_pandoc3(block, fname, caption, alt)
  local fig_attr = {
    id = block.identifier,
    name = block.attributes.name,
  }
  local img_attr = {
    width = block.attributes.width,
    height = block.attributes.height,
  }
  local img_obj = pandoc.Image(alt, fname, "", img_attr)
  return pandoc.Figure(pandoc.Plain{img_obj}, caption, fig_attr)
end

--- Process code blocks and convert them to images
---
--- This function processes code blocks with diagram generator classes and
--- converts them to images or figures. Supported classes:
--- - plantuml: PlantUML diagrams
--- - graphviz: GraphViz/DOT graphs
--- - tikz: TikZ graphics
--- - py2image: Python image generation
--- - asymptote: Asymptote graphics
--- - bitfield: OmniDoc bitfield JSON
---
--- @param block table The CodeBlock element
--- @return table|nil The processed block (Image or Figure), or nil if unchanged
function CodeBlock(block)
  -- Get converter for this code block class
  local converter = get_converter(block.classes[1])
  if not converter then
    return nil  -- Not a diagram code block, leave unchanged
  end

  -- Generate the image
  local is_bitfield = block.classes[1] == "bitfield"
  local output_type = is_bitfield and bitfield_filetype or filetype
  local media_type = is_bitfield and bitfield_mimetype or mimetype
  local img_data = generate_image(block, converter, output_type)
  if not img_data then
    return nil
  end

  -- Generate filename and save image
  local fname = generate_filename(block, img_data, output_type)
  save_image(fname, img_data, media_type)

  -- Process caption
  local caption = block.attributes.caption and
                  pandoc.read(block.attributes.caption).blocks or
                  pandoc.Blocks{}
  local alt = pandoc.utils.blocks_to_inlines(caption)

  -- Handle different Pandoc versions
  if PANDOC_VERSION < 3 then
    return create_image_pandoc2(block, fname, caption, alt)
  else
    return create_figure_pandoc3(block, fname, caption, alt)
  end
end

-- ============================================================================
-- Filter Registration
-- ============================================================================

-- Meta must be processed first to get tool paths
-- Then code blocks are processed
return {
  {Meta = Meta},
  {CodeBlock = CodeBlock},
  {Pandoc = write_depfile},
}
