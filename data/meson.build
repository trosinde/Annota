# UI (Blueprint → .ui) and GResource bundle
blueprints = custom_target(
  'blueprints',
  input: files(
    'ui/window.blp',
    'ui/preferences.blp',
    'ui/help-overlay.blp',
  ),
  output: '.',
  command: [blueprint_compiler, 'batch-compile', '@OUTPUT@', '@CURRENT_SOURCE_DIR@', '@INPUT@'],
)

resources = gnome.compile_resources(
  'resources',
  'resources.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
  dependencies: blueprints,
)

# Desktop file
desktop_file = i18n.merge_file(
  input: app_id + '.desktop.in',
  output: app_id + '.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: datadir / 'applications',
)
if desktop_file_validate.found()
  test('validate-desktop', desktop_file_validate, args: [desktop_file])
endif

# AppStream metainfo
metainfo_file = i18n.merge_file(
  input: app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: datadir / 'metainfo',
)
if appstreamcli.found()
  test('validate-metainfo', appstreamcli, args: ['validate', '--no-net', '--explain', metainfo_file])
endif

# GSettings schema
install_data(app_id + '.gschema.xml', install_dir: datadir / 'glib-2.0' / 'schemas')
test('validate-schema', glib_compile_schemas, args: ['--strict', '--dry-run', meson.current_source_dir()])

# Icons and manual page
install_subdir('icons/hicolor', install_dir: datadir / 'icons')
install_man('annota.1')
