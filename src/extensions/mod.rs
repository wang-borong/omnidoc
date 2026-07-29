mod package;
mod plugin;
mod theme;

pub(crate) use package::extension_store_roots;
pub use package::{
    acquire_extension_store_read_locks, discover_packages, ensure_pandoc_compatible,
    install_package, package_spec, uninstall_package, ExtensionResource, ExtensionStoreReadLocks,
    InstallPackageReport, InstallPackageRequest, PackageInspection, PackageKind, PackageManifest,
    PackageScope, PackageSpec, ResolvedPackageIdentity, UninstallPackageReport,
    PACKAGE_MANIFEST_FILE,
};
pub(crate) use plugin::plugin_trust_path;
pub(crate) use plugin::resolve_plugin_manifest;
pub use plugin::{
    enabled_plugin_resources, enabled_plugins, is_plugin_trusted, plugin_catalog,
    plugin_filters_for_output, resolve_plugin_request, revoke_plugin_trust, run_plugin_command,
    trust_plugin, validate_plugin_lua, PluginCatalogEntry, ResolvedPlugin, ResolvedPluginFilter,
};
pub(crate) use theme::resolve_theme_manifest;
pub use theme::{
    materialize_theme_tokens, resolve_selected_theme, resolve_theme_request, theme_catalog,
    GeneratedThemeAssets, ResolvedTheme, ThemeCatalogEntry, ThemeMetadata, ThemeRequirements,
    ThemeResources,
};
