--- bibexport.lua – Export bibliography references to metadata
---
--- This filter extracts all bibliography references from the document and
--- adds them to the document metadata as 'references'. This is useful for
--- processing or exporting bibliography data separately from the document.
---
--- Features:
---   - Extracts all bibliography references
---   - Adds references to document metadata
---   - Removes bibliography metadata key (if present)
---
--- Usage:
---   No special syntax needed - this filter automatically processes all
---   documents with bibliography references.
---
--- Note: This filter uses pandoc.utils.references() which requires
---       pandoc 2.11 or later.

-- ============================================================================
-- Document Processing
-- ============================================================================

--- Extract bibliography references and add to metadata
---
--- This function processes the document and extracts all bibliography
--- references using pandoc.utils.references(). The references are added
--- to the document metadata, and the bibliography metadata key is removed.
---
--- @param doc table The Pandoc document
--- @return table The document with updated metadata
function Pandoc(doc)
  -- Extract all bibliography references from the document
  doc.meta.references = pandoc.utils.references(doc)
  
  -- Remove the bibliography metadata key (if present)
  -- This prevents duplicate bibliography processing
  doc.meta.bibliography = nil
  
  return doc
end
