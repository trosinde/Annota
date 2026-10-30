conf = configuration_data()
conf.set_quoted('APP_ID', app_id)
conf.set_quoted('VERSION', meson.project_version())
conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)

configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: conf,
)

# Copy the generated config.rs into the source tree so cargo can find it.
run_command(
  'cp',
  meson.project_build_root() / 'src' / 'config.rs',
  meson.project_source_root() / 'src' / 'config.rs',
  check: true,
)

if get_option('profile') == 'release'
  cargo_profile_args = ['--release']
  rust_target = 'release'
else
  cargo_profile_args = []
  rust_target = 'debug'
endif

cargo_target_dir = meson.project_build_root() / 'target'

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  command: [
    'env',
    'CARGO_TARGET_DIR=' + cargo_target_dir,
    cargo, 'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--locked',
    cargo_profile_args,
    '&&',
    'cp', cargo_target_dir / rust_target / meson.project_name(), '@OUTPUT@',
  ],
)

test(
  'cargo-test',
  cargo,
  args: [
    'test',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--locked',
    '--target-dir', cargo_target_dir,
  ],
  timeout: 600,
)
