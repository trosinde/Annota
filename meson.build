project(
  'annota',
  'rust',
  version: '0.1.0',
  license: 'MIT',
  meson_version: '>= 1.0',
)

i18n = import('i18n')
gnome = import('gnome')

app_id = 'io.github.trosinde.Annota'
gettext_package = meson.project_name()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')
pkgdatadir = datadir / meson.project_name()

dependency('gtk4', version: '>= 4.14')
dependency('libadwaita-1', version: '>= 1.5')

cargo = find_program('cargo', required: true)
blueprint_compiler = find_program('blueprint-compiler', required: true)
glib_compile_resources = find_program('glib-compile-resources', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)

subdir('data')
subdir('po')
subdir('src')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
