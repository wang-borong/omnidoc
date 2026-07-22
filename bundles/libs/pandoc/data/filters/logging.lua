--[[
logging.lua: Pandoc-aware logging functions

This module provides a comprehensive logging system for Pandoc Lua filters.
It can be used both within Pandoc filters and as a standalone module.

Features:
  - Multiple log levels: ERROR, WARNING, INFO, DEBUG, TRACE
  - Automatic log level detection from Pandoc verbosity settings
  - Pretty-printing of complex data structures (tables, userdata)
  - Pandoc-aware type detection
  - Color-coded output to stderr

Usage:
  local logging = require("logging")
  logging.info("Processing document...")
  logging.debug("Debug information:", variable)
  logging.error("Error occurred:", error_message)

Log Levels:
  - ERROR (-1): Only error messages
  - WARNING (0): Errors and warnings (default)
  - INFO (1): Errors, warnings, and info messages
  - DEBUG (2): All messages except trace
  - TRACE (3): All messages including trace

The log level is automatically set based on Pandoc's verbosity:
  - --quiet -> ERROR
  - default -> WARNING
  - --verbose -> INFO
  - --trace -> DEBUG or TRACE

Copyright: (c) 2022 William Lupton
License:     MIT - see LICENSE file for details
]]

-- ============================================================================
-- Pandoc Compatibility Layer
-- ============================================================================

-- Create a minimal pandoc global if running standalone
if not pandoc then
  _G.pandoc = {utils = {}}
end

-- Create pandoc.utils if it doesn't exist
if not pcall(require, 'pandoc.utils') then
  pandoc.utils = {}
end

-- Create pandoc.utils.type if it doesn't exist
if not pandoc.utils.type then
  pandoc.utils.type = function(value)
    local typ = type(value)
    if not ({table = 1, userdata = 1})[typ] then
      -- Basic types (nil, number, string, boolean, function, thread)
      return typ
    elseif value.__name then
      -- Userdata with __name (some Pandoc types)
      return value.__name
    elseif value.tag and value.t then
      -- Pandoc elements with tag property
      typ = value.tag
      -- Remove "Meta." prefix from Meta types
      if typ:match('^Meta%.') then
        typ = typ:sub(6)
      end
      -- Map Map to table
      if typ == 'Map' then
        typ = 'table'
      end
      return typ
    end
    return typ
  end
end

-- ============================================================================
-- Logging Module
-- ============================================================================

local logging = {}

-- ============================================================================
-- Type Detection
-- ============================================================================

--- Get a sensible type name for a value, handling Pandoc types
---
--- This function returns type names that are more meaningful for Pandoc
--- objects. It maps generic types like 'Inline' and 'Block' to their
--- specific tag names (e.g., 'Str', 'Para').
---
--- @param value any The value to get the type of
--- @return string The type name
logging.type = function(value)
  local typ = pandoc.utils.type(value)

  -- Replace spaces with periods in type names (e.g., "pandoc Row" -> "pandoc.Row")
  typ = typ:gsub(' ', '.')

  -- For Inline and Block types, return the specific tag name
  if ({Inline = 1, Block = 1})[typ] and value.tag then
    return value.tag
  end

  return typ
end

-- ============================================================================
-- Sorting Utilities
-- ============================================================================

--- Sorted pairs iterator for tables
---
--- Returns key-value pairs from a table in sorted order (alphabetical by key).
--- This ensures consistent output order for debugging.
---
--- Derived from: https://www.lua.org/pil/19.3.html pairsByKeys()
---
--- @param list table The table to iterate over
--- @param comp function|nil Optional comparison function for sorting
--- @return function Iterator function
logging.spairs = function(list, comp)
  local keys = {}
  for key, _ in pairs(list) do
    table.insert(keys, tostring(key))
  end
  table.sort(keys, comp)
  local i = 0
  local iter = function()
    i = i + 1
    return keys[i] and keys[i], list[keys[i]] or nil
  end
  return iter
end

-- ============================================================================
-- Value Dumping
-- ============================================================================

--- Recursively dump a value to a string representation
---
--- This function creates a human-readable string representation of a value,
--- handling nested structures, Pandoc objects, and various data types.
---
--- @param prefix string|nil Prefix for the current level
--- @param value any The value to dump
--- @param maxlen number|nil Maximum length for single-line format
--- @param level number|nil Current recursion level
--- @param add function|nil Function to add output lines
--- @return string The dumped representation
local function dump_(prefix, value, maxlen, level, add)
  local buffer = {}
  prefix = prefix or ''
  level = level or 0
  add = add or function(item) table.insert(buffer, item) end
  local indent = maxlen and '' or ('  '):rep(level)

  -- Get type name
  local typename = logging.type(value)

  -- Don't show type for obvious types
  local typ = (({boolean = 1, number = 1, string = 1, table = 1, userdata = 1})
               [typename] and '' or typename)

  -- Handle light userdata (pointers that can't be iterated)
  if type(value) == 'userdata' and not pcall(pairs, value) then
    value = tostring(value):gsub('userdata:%s*', '')
  elseif ({table = 1, userdata = 1})[type(value)] then
    -- Copy value, filtering out certain keys
    local valueCopy, numKeys, lastKey = {}, 0, nil
    for key, val in pairs(value) do
      -- Skip 'tag', nil values, and functions
      if key ~= 'tag' and val and type(val) ~= 'function' then
        valueCopy[key] = val
        numKeys = numKeys + 1
        lastKey = key
      end
    end

    -- Special formatting for simple cases
    if numKeys == 0 then
      -- Empty table or special types
      value = typename == 'Doc' and '|' .. value:render() .. '|' or
              typename == 'Space' and '' or
              '{}'
    elseif numKeys == 1 and lastKey == 'text' then
      -- Single 'text' key (e.g., Str, Code)
      typ = typename
      value = value[lastKey]
      typename = 'string'
    else
      value = valueCopy
      -- Add array size indicator
      if #value > 0 then
        typ = typ .. '[' .. #value .. ']'
      end
    end
  end

  -- Format the output
  local presep = #prefix > 0 and ' ' or ''
  local typsep = #typ > 0 and ' ' or ''
  local valtyp = type(value)

  if valtyp == 'nil' then
    add('nil')
  elseif ({boolean = 1, number = 1, string = 1})[valtyp] then
    typsep = #typ > 0 and valtyp == 'string' and #value > 0 and ' ' or ''
    local quo = typename == 'string' and '"' or ''
    add(string.format('%s%s%s%s%s%s%s%s', indent, prefix, presep, typ,
                      typsep, quo, value, quo))
  elseif valtyp == 'userdata' and not pcall(pairs, value) then
    add(string.format('%s%s%s%s %s', indent, prefix, presep, typ,
                      tostring(value):gsub('userdata:%s*', '')))
  elseif ({table = 1, userdata = 1})[valtyp] then
    add(string.format('%s%s%s%s%s{', indent, prefix, presep, typ, typsep))
    
    local first = true
    
    -- Print array elements first (except for Attr)
    if prefix ~= 'attributes:' and typ ~= 'Attr' then
      for i, val in ipairs(value) do
        local pre = maxlen and not first and ', ' or ''
        dump_(string.format('%s[%s]', pre, i), val, maxlen, level + 1, add)
        first = false
      end
    end
    
    -- Print key-value pairs in alphabetical order
    for key, val in logging.spairs(value) do
      local pre = maxlen and not first and ', ' or ''
      
      -- Skip special keys or handle them specially
      if key:match('^__') and type(val) ~= 'string' then
        add(string.format('%s%s: %s', pre, key, tostring(val)))
      elseif not tonumber(key) and key ~= 'tag' then
        dump_(string.format('%s%s:', pre, key), val, maxlen, level + 1, add)
      end
      first = false
    end
    
    add(string.format('%s}', indent))
  end

  return table.concat(buffer, maxlen and '' or '\n')
end

--- Dump a value to a string, with automatic formatting
---
--- If the value is too long for single-line format, it automatically
--- switches to multi-line format.
---
--- @param value any The value to dump
--- @param maxlen number|nil Maximum length for single-line format (default: 70)
--- @return string The dumped representation
logging.dump = function(value, maxlen)
  if maxlen == nil then
    maxlen = 70
  end
  local text = dump_(nil, value, maxlen)
  if #text > maxlen then
    text = dump_(nil, value, nil)  -- Switch to multi-line
  end
  return text
end

--- Output values to stderr with automatic formatting
---
--- @param ... any Variable number of arguments to output
logging.output = function(...)
  local need_newline = false
  for i, item in ipairs({...}) do
    local maybe_space = i > 1 and ' ' or ''
    local text = ({table = 1, userdata = 1})[type(item)] and
                 logging.dump(item) or
                 tostring(item)
    io.stderr:write(maybe_space, text)
    need_newline = text:sub(-1) ~= '\n'
  end
  if need_newline then
    io.stderr:write('\n')
  end
end

-- ============================================================================
-- Log Level Management
-- ============================================================================

--- Current log level
--- Levels: -1=ERROR, 0=WARNING, 1=INFO, 2=DEBUG, 3=TRACE
logging.loglevel = 0

--- Set the log level and return the previous level
---
--- @param loglevel number The new log level
--- @return number The previous log level
logging.setloglevel = function(loglevel)
  local oldlevel = logging.loglevel
  logging.loglevel = loglevel
  return oldlevel
end

-- ============================================================================
-- Log Level Detection from Pandoc
-- ============================================================================

-- Set log level based on Pandoc verbosity settings
if type(PANDOC_STATE) == 'nil' then
  -- Not running in Pandoc, use default
elseif PANDOC_STATE.trace then
  -- --trace flag set
  logging.loglevel = PANDOC_STATE.verbosity == 'INFO' and 3 or 2
elseif PANDOC_STATE.verbosity == 'INFO' then
  -- --verbose flag set
  logging.loglevel = 1
elseif PANDOC_STATE.verbosity == 'WARNING' then
  -- Default verbosity
  logging.loglevel = 0
elseif PANDOC_STATE.verbosity == 'ERROR' then
  -- --quiet flag set
  logging.loglevel = -1
end

-- ============================================================================
-- Logging Functions
-- ============================================================================

--- Log an error message
---
--- @param ... any Variable number of arguments to log
logging.error = function(...)
  if logging.loglevel >= -1 then
    logging.output('(E)', ...)
  end
end

--- Log a warning message
---
--- @param ... any Variable number of arguments to log
logging.warning = function(...)
  if logging.loglevel >= 0 then
    logging.output('(W)', ...)
  end
end

--- Log an info message
---
--- @param ... any Variable number of arguments to log
logging.info = function(...)
  if logging.loglevel >= 1 then
    logging.output('(I)', ...)
  end
end

--- Log a debug message
---
--- @param ... any Variable number of arguments to log
logging.debug = function(...)
  if logging.loglevel >= 2 then
    logging.output('(D)', ...)
  end
end

--- Log a debug2 message (deprecated, use trace instead)
---
--- @param ... any Variable number of arguments to log
logging.debug2 = function(...)
  if logging.loglevel >= 3 then
    logging.warning('debug2() is deprecated; use trace()')
    logging.output('(D2)', ...)
  end
end

--- Log a trace message
---
--- @param ... any Variable number of arguments to log
logging.trace = function(...)
  if logging.loglevel >= 3 then
    logging.output('(T)', ...)
  end
end

--- Temporary unconditional debug output
---
--- This function always outputs, regardless of log level.
--- Useful for temporary debugging.
---
--- @param ... any Variable number of arguments to log
logging.temp = function(...)
  logging.output('(#)', ...)
end

-- ============================================================================
-- Module Export
-- ============================================================================

return logging
