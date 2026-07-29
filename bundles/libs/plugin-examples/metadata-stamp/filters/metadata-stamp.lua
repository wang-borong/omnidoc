function Pandoc(document)
  document.meta["omnidoc-plugin"] = pandoc.MetaString("metadata-stamp/1.0.0")
  if document.meta.generator == nil then
    document.meta.generator = pandoc.MetaString("OmniDoc")
  end
  return document
end
