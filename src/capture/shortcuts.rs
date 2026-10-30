//! Global capture shortcuts (spec: Global capture hotkey; CLAUDE.md §2 "Global hotkey").
//!
//! * With the GlobalShortcuts portal (GNOME ≥ 48) Annota binds its three shortcuts through the
//!   portal while it runs.
//! * Without it, Annota writes GNOME custom keybindings that run `annota --capture <mode>`.
//!   Per decision D9 the defaults are GNOME's own screenshot keys; GNOME's bindings for the
//!   same keys are moved aside (recorded in `moved-gnome-keys`) and given back when the
//!   Annota shortcut is disabled or changed — reversible, never deleted.
//!
//! Everything that decides *what* to write is pure and unit-tested below; the GSettings
//! writers are thin wrappers. Neither the portal nor the GNOME keybindings were exercised
//! against a live session during development (see `manual-checks.md`).

use std::cell::Cell;
use std::ffi::OsStr;
use std::path::Path;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};

use super::portal;
use crate::cli::CaptureMode;
use crate::config;

/// Schema holding the list of custom keybinding paths.
pub const MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
/// Relocatable schema of one custom keybinding.
pub const CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const CUSTOM_KEYBINDINGS_BASE: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";
/// GNOME Shell's own screenshot keys (the ones D9 takes over).
pub const GNOME_SHELL_KEYBINDINGS: &str = "org.gnome.shell.keybindings";
pub const GNOME_SCREENSHOT_KEYS: [&str; 3] =
    ["show-screenshot-ui", "screenshot-window", "screenshot"];
/// Other GNOME schemas whose keybindings count as conflicts.
const CONFLICT_SCHEMAS: [&str; 5] = [
    GNOME_SHELL_KEYBINDINGS,
    "org.gnome.desktop.wm.keybindings",
    "org.gnome.mutter.keybindings",
    "org.gnome.mutter.wayland.keybindings",
    MEDIA_KEYS_SCHEMA,
];

/// Annota settings key holding the accelerator for `mode` (GTK syntax, empty = disabled).
pub fn settings_key(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Region => "shortcut-region",
        CaptureMode::Window => "shortcut-window",
        CaptureMode::Screen => "shortcut-screen",
    }
}

/// Row title in Preferences, also the conflict name for Annota's own shortcuts.
pub fn mode_title(mode: CaptureMode) -> String {
    match mode {
        CaptureMode::Region => gettext("Region capture"),
        CaptureMode::Window => gettext("Window capture"),
        CaptureMode::Screen => gettext("Screen capture"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Modifier {
    Shift,
    Control,
    Alt,
    Super,
    Hyper,
    Meta,
}

impl Modifier {
    fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "shift" => Some(Self::Shift),
            "control" | "ctrl" | "ctl" | "primary" => Some(Self::Control),
            "alt" | "mod1" => Some(Self::Alt),
            "super" | "mod4" => Some(Self::Super),
            "hyper" => Some(Self::Hyper),
            "meta" => Some(Self::Meta),
            _ => None,
        }
    }

    fn gtk_name(self) -> &'static str {
        match self {
            Self::Shift => "<Shift>",
            Self::Control => "<Control>",
            Self::Alt => "<Alt>",
            Self::Super => "<Super>",
            Self::Hyper => "<Hyper>",
            Self::Meta => "<Meta>",
        }
    }

    /// Modifier names of the XDG "shortcuts" specification used by the portal.
    fn xdg_name(self) -> &'static str {
        match self {
            Self::Shift => "SHIFT",
            Self::Control => "CTRL",
            Self::Alt => "ALT",
            Self::Super | Self::Hyper | Self::Meta => "LOGO",
        }
    }
}

/// A parsed GTK accelerator such as `<Alt>Print`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accel {
    modifiers: Vec<Modifier>,
    key: String,
}

impl Accel {
    pub fn parse(accel: &str) -> Option<Self> {
        let mut rest = accel.trim();
        let mut modifiers = Vec::new();
        while let Some(stripped) = rest.strip_prefix('<') {
            let end = stripped.find('>')?;
            modifiers.push(Modifier::parse(&stripped[..end])?);
            rest = &stripped[end + 1..];
        }
        if rest.is_empty() || rest.contains(['<', '>']) {
            return None;
        }
        modifiers.sort_unstable();
        modifiers.dedup();
        Some(Self {
            modifiers,
            key: rest.to_string(),
        })
    }

    /// Canonical GTK spelling, e.g. `<Shift><Alt>Print`.
    pub fn to_gtk(&self) -> String {
        let mut out: String = self.modifiers.iter().map(|m| m.gtk_name()).collect();
        out.push_str(&self.key);
        out
    }

    /// XDG shortcuts-spec trigger for the portal, e.g. `SHIFT+Print`.
    pub fn to_xdg_trigger(&self) -> String {
        let mut parts: Vec<&str> = self.modifiers.iter().map(|m| m.xdg_name()).collect();
        parts.dedup();
        parts.push(&self.key);
        parts.join("+")
    }

    fn matches(&self, other: &Accel) -> bool {
        self.modifiers == other.modifiers && self.key.eq_ignore_ascii_case(&other.key)
    }
}

/// Whether two GTK accelerator strings mean the same key combination.
pub fn same_accel(a: &str, b: &str) -> bool {
    match (Accel::parse(a), Accel::parse(b)) {
        (Some(a), Some(b)) => a.matches(&b),
        _ => false,
    }
}

/// One GNOME custom keybinding written by Annota.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingEntry {
    pub path: String,
    pub name: String,
    pub command: String,
    pub binding: String,
}

pub fn keybinding_path(mode: CaptureMode) -> String {
    format!("{CUSTOM_KEYBINDINGS_BASE}annota-{}/", mode.as_str())
}

/// The command a GNOME keybinding runs, e.g. `annota --capture region`.
pub fn capture_command(program: &str, mode: CaptureMode) -> String {
    format!("{program} --capture {}", mode.as_str())
}

/// How the keybinding refers to Annota: plain `annota` when the running binary is the one on
/// `PATH` (the packaged install), otherwise its absolute path (development builds).
pub fn program_for_keybinding(exe: Option<&Path>, path_env: Option<&OsStr>) -> String {
    let Some(exe) = exe else {
        return String::from("annota");
    };
    let on_path = exe.file_name() == Some(OsStr::new("annota"))
        && exe.parent().is_some_and(|dir| {
            path_env.is_some_and(|paths| std::env::split_paths(paths).any(|p| p == dir))
        });
    let plain = exe.to_str().is_some_and(|s| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    });
    if on_path {
        String::from("annota")
    } else if plain {
        exe.to_string_lossy().into_owned()
    } else {
        glib::shell_quote(exe).to_string_lossy().into_owned()
    }
}

/// Keybinding entries for all enabled shortcuts.
pub fn keybinding_entries(
    accels: &[(CaptureMode, String)],
    program: &str,
    name: impl Fn(CaptureMode) -> String,
) -> Vec<KeybindingEntry> {
    accels
        .iter()
        .filter_map(|(mode, accel)| {
            let accel = Accel::parse(accel)?;
            Some(KeybindingEntry {
                path: keybinding_path(*mode),
                name: name(*mode),
                command: capture_command(program, *mode),
                binding: accel.to_gtk(),
            })
        })
        .collect()
}

/// The `custom-keybindings` list after adding `add` and dropping `remove`, keeping the
/// user's other keybindings untouched and in order.
pub fn merge_paths(existing: &[String], add: &[String], remove: &[String]) -> Vec<String> {
    let mut out: Vec<String> = existing
        .iter()
        .filter(|p| !remove.contains(p))
        .cloned()
        .collect();
    for path in add {
        if !out.contains(path) {
            out.push(path.clone());
        }
    }
    out
}

/// New values for GNOME keys: (key, accelerators).
pub type GnomeKeyUpdates = Vec<(String, Vec<String>)>;
/// GNOME bindings Annota moved aside: (key, accelerator).
pub type MovedKeys = Vec<(String, String)>;

/// New values for GNOME's own keys plus the bindings that were moved aside.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Takeover {
    pub updates: Vec<(String, Vec<String>)>,
    pub moved: Vec<(String, String)>,
}

/// Removes Annota's accelerators from GNOME's screenshot keys and records what was removed.
pub fn plan_takeover(gnome: &[(String, Vec<String>)], ours: &[String]) -> Takeover {
    let mut takeover = Takeover::default();
    for (key, accels) in gnome {
        let (taken, kept): (Vec<String>, Vec<String>) = accels
            .iter()
            .cloned()
            .partition(|a| ours.iter().any(|o| same_accel(a, o)));
        if !taken.is_empty() {
            takeover.updates.push((key.clone(), kept));
            takeover
                .moved
                .extend(taken.into_iter().map(|a| (key.clone(), a)));
        }
    }
    takeover
}

/// Gives GNOME back the moved bindings for `released` accelerators. Returns the GNOME key
/// updates and the moved bindings that remain with Annota.
pub fn plan_restore(
    gnome: &[(String, Vec<String>)],
    moved: &[(String, String)],
    released: &[String],
) -> (GnomeKeyUpdates, MovedKeys) {
    let mut updates: Vec<(String, Vec<String>)> = Vec::new();
    let mut remaining = Vec::new();
    for (key, accel) in moved {
        if !released.iter().any(|r| same_accel(r, accel)) {
            remaining.push((key.clone(), accel.clone()));
            continue;
        }
        let index = match updates.iter().position(|(k, _)| k == key) {
            Some(index) => index,
            None => {
                let current = gnome
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                updates.push((key.clone(), current));
                updates.len() - 1
            }
        };
        let list = &mut updates[index].1;
        if !list.iter().any(|a| same_accel(a, accel)) {
            list.push(accel.clone());
        }
    }
    (updates, remaining)
}

/// Who already uses an accelerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub name: String,
    /// One of GNOME's screenshot keys, which Annota moves aside (D9) — not a blocker.
    pub takeover: bool,
}

/// First binding in `bindings` (name, accelerators) that uses `accel`.
pub fn find_user<'a>(bindings: &'a [(String, Vec<String>)], accel: &str) -> Option<&'a str> {
    bindings
        .iter()
        .find(|(_, accels)| accels.iter().any(|a| same_accel(a, accel)))
        .map(|(name, _)| name.as_str())
}

// ---------------------------------------------------------------------------------------------
// GSettings access (thin; not exercised in tests)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("GNOME keyboard settings are not available on this system")]
    Unsupported,
    #[error("could not write GNOME keyboard settings: {0}")]
    Write(#[from] glib::BoolError),
}

fn schema(id: &str) -> Option<gio::SettingsSchema> {
    gio::SettingsSchemaSource::default()?.lookup(id, true)
}

/// Settings for `id`, or `None` if the schema is not installed (`gio::Settings::new` would
/// abort the process otherwise).
fn settings_if_installed(id: &str) -> Option<gio::Settings> {
    schema(id).map(|_| gio::Settings::new(id))
}

pub fn gnome_keybindings_supported() -> bool {
    schema(MEDIA_KEYS_SCHEMA).is_some() && schema(CUSTOM_KEYBINDING_SCHEMA).is_some()
}

/// Accelerators currently configured in Annota, one per mode (empty = disabled).
pub fn configured_accels(settings: &gio::Settings) -> Vec<(CaptureMode, String)> {
    CaptureMode::ALL
        .into_iter()
        .map(|mode| (mode, settings.string(settings_key(mode)).to_string()))
        .collect()
}

fn enabled_accels(settings: &gio::Settings) -> Vec<String> {
    configured_accels(settings)
        .into_iter()
        .map(|(_, accel)| accel)
        .filter(|accel| Accel::parse(accel).is_some())
        .collect()
}

fn read_gnome_screenshot_keys() -> Vec<(String, Vec<String>)> {
    let Some(shell) = settings_if_installed(GNOME_SHELL_KEYBINDINGS) else {
        return Vec::new();
    };
    let Some(schema) = schema(GNOME_SHELL_KEYBINDINGS) else {
        return Vec::new();
    };
    GNOME_SCREENSHOT_KEYS
        .iter()
        .filter(|key| schema.has_key(key))
        .map(|key| {
            let accels = shell.strv(key).iter().map(|s| s.to_string()).collect();
            (key.to_string(), accels)
        })
        .collect()
}

fn write_gnome_screenshot_keys(updates: &[(String, Vec<String>)]) -> Result<(), ShortcutError> {
    if updates.is_empty() {
        return Ok(());
    }
    let shell = settings_if_installed(GNOME_SHELL_KEYBINDINGS).ok_or(ShortcutError::Unsupported)?;
    for (key, accels) in updates {
        let refs: Vec<&str> = accels.iter().map(String::as_str).collect();
        shell.set_strv(key, refs.as_slice())?;
    }
    Ok(())
}

fn moved_keys(settings: &gio::Settings) -> Vec<(String, String)> {
    settings
        .value("moved-gnome-keys")
        .get::<Vec<(String, String)>>()
        .unwrap_or_default()
}

fn set_moved_keys(
    settings: &gio::Settings,
    moved: &[(String, String)],
) -> Result<(), ShortcutError> {
    settings.set_value("moved-gnome-keys", &moved.to_vec().to_variant())?;
    Ok(())
}

/// Moves GNOME's bindings for `accels` aside (D9) and remembers them for later restore.
///
/// Records the move *before* actually removing GNOME's bindings: if the record write failed
/// after GNOME's keys were already cleared, Annota would have no way to know what to give back,
/// losing the user's Print key for good. Recording first and removing second instead risks only
/// a stale record for a removal that never happened, which `give_back` handles as a harmless
/// no-op (it writes GNOME's key back to a value it never actually lost).
fn take_over(settings: &gio::Settings, accels: &[String]) -> Result<(), ShortcutError> {
    let takeover = plan_takeover(&read_gnome_screenshot_keys(), accels);
    if !takeover.moved.is_empty() {
        let mut moved = moved_keys(settings);
        for entry in &takeover.moved {
            if !moved.contains(entry) {
                moved.push(entry.clone());
            }
        }
        set_moved_keys(settings, &moved)?;
    }
    write_gnome_screenshot_keys(&takeover.updates)?;
    Ok(())
}

/// Gives GNOME back whatever Annota moved aside for `released` accelerators.
fn give_back(settings: &gio::Settings, released: &[String]) -> Result<(), ShortcutError> {
    let (updates, remaining) = plan_restore(
        &read_gnome_screenshot_keys(),
        &moved_keys(settings),
        released,
    );
    write_gnome_screenshot_keys(&updates)?;
    set_moved_keys(settings, &remaining)
}

fn write_custom_keybindings(
    entries: &[KeybindingEntry],
    remove: &[String],
) -> Result<(), ShortcutError> {
    if !gnome_keybindings_supported() {
        return Err(ShortcutError::Unsupported);
    }
    let media = gio::Settings::new(MEDIA_KEYS_SCHEMA);
    for entry in entries {
        let binding = gio::Settings::with_path(CUSTOM_KEYBINDING_SCHEMA, &entry.path);
        binding.set_string("name", &entry.name)?;
        binding.set_string("command", &entry.command)?;
        binding.set_string("binding", &entry.binding)?;
    }
    for path in remove {
        let binding = gio::Settings::with_path(CUSTOM_KEYBINDING_SCHEMA, path);
        for key in ["name", "command", "binding"] {
            binding.reset(key);
        }
    }
    let existing: Vec<String> = media
        .strv("custom-keybindings")
        .iter()
        .map(|s| s.to_string())
        .collect();
    let added: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let merged = merge_paths(&existing, &added, remove);
    let refs: Vec<&str> = merged.iter().map(String::as_str).collect();
    media.set_strv("custom-keybindings", refs.as_slice())?;
    Ok(())
}

fn program() -> String {
    program_for_keybinding(
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

/// The command Annota's GNOME keybinding for `mode` runs (shown in Preferences, mockup 10).
pub fn keybinding_command(mode: CaptureMode) -> String {
    capture_command(&program(), mode)
}

fn keybinding_name(mode: CaptureMode) -> String {
    format!("Annota: {}", mode_title(mode))
}

/// "Create GNOME Shortcuts" (mockup 09 → 10). Returns how many shortcuts were created.
///
/// Writes Annota's own keybindings before touching any of GNOME's: if `write_custom_keybindings`
/// fails (e.g. a locked dconf key), GNOME's Print bindings are never moved aside in the first
/// place, rather than removed with `gnome-shortcuts-created` never getting set true — which would
/// leave the UI's give-back path (gated on that flag) unable to reach them.
pub fn create_gnome_shortcuts(settings: &gio::Settings) -> Result<usize, ShortcutError> {
    if !gnome_keybindings_supported() {
        return Err(ShortcutError::Unsupported);
    }
    let accels = configured_accels(settings);
    let entries = keybinding_entries(&accels, &program(), keybinding_name);
    let disabled: Vec<String> = accels
        .iter()
        .filter(|(_, accel)| Accel::parse(accel).is_none())
        .map(|(mode, _)| keybinding_path(*mode))
        .collect();
    write_custom_keybindings(&entries, &disabled)?;
    take_over(settings, &enabled_accels(settings))?;
    settings.set_boolean("gnome-shortcuts-created", true)?;
    Ok(entries.len())
}

/// Applies a shortcut change made in the Set Shortcut dialog. `new_accel` empty disables it.
pub fn change_shortcut(
    settings: &gio::Settings,
    mode: CaptureMode,
    new_accel: &str,
) -> Result<(), ShortcutError> {
    let old_accel = settings.string(settings_key(mode)).to_string();
    settings.set_string(settings_key(mode), new_accel)?;
    if settings.boolean("gnome-shortcuts-created") {
        let still_used = enabled_accels(settings);
        if !old_accel.is_empty() && !still_used.iter().any(|a| same_accel(a, &old_accel)) {
            give_back(settings, &[old_accel])?;
        }
        match Accel::parse(new_accel) {
            Some(_) => {
                take_over(settings, &[new_accel.to_string()])?;
                let accels = [(mode, new_accel.to_string())];
                write_custom_keybindings(
                    &keybinding_entries(&accels, &program(), keybinding_name),
                    &[],
                )?;
            }
            None => write_custom_keybindings(&[], &[keybinding_path(mode)])?,
        }
    } else if PORTAL_ACTIVE.with(Cell::get) {
        restart_portal_shortcuts();
    }
    Ok(())
}

/// Who already uses `accel` (Set Shortcut dialog status line, mockup 11).
pub fn find_conflict(settings: &gio::Settings, mode: CaptureMode, accel: &str) -> Option<Conflict> {
    let own: Vec<(String, Vec<String>)> = configured_accels(settings)
        .into_iter()
        .filter(|(other, _)| *other != mode)
        .map(|(other, a)| (mode_title(other), vec![a]))
        .collect();
    if let Some(name) = find_user(&own, accel) {
        return Some(Conflict {
            name: name.to_string(),
            takeover: false,
        });
    }

    for id in CONFLICT_SCHEMAS {
        let (Some(schema), Some(values)) = (schema(id), settings_if_installed(id)) else {
            continue;
        };
        for key in schema.list_keys() {
            let key = key.as_str();
            let accels: Vec<String> = match schema.key(key).value_type().as_str() {
                "as" => values.strv(key).iter().map(|s| s.to_string()).collect(),
                "s" => vec![values.string(key).to_string()],
                _ => continue,
            };
            if accels.iter().any(|a| same_accel(a, accel)) {
                let name = schema
                    .key(key)
                    .summary()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| key.to_string());
                let takeover =
                    id == GNOME_SHELL_KEYBINDINGS && GNOME_SCREENSHOT_KEYS.contains(&key);
                return Some(Conflict { name, takeover });
            }
        }
    }

    // Other custom keybindings (not Annota's own).
    let media = settings_if_installed(MEDIA_KEYS_SCHEMA)?;
    schema(CUSTOM_KEYBINDING_SCHEMA)?;
    media
        .strv("custom-keybindings")
        .iter()
        .filter(|path| {
            !path
                .as_str()
                .starts_with(&format!("{CUSTOM_KEYBINDINGS_BASE}annota-"))
        })
        .find_map(|path| {
            let binding = gio::Settings::with_path(CUSTOM_KEYBINDING_SCHEMA, path.as_str());
            same_accel(&binding.string("binding"), accel).then(|| Conflict {
                name: binding.string("name").to_string(),
                takeover: false,
            })
        })
}

// ---------------------------------------------------------------------------------------------
// GlobalShortcuts portal
// ---------------------------------------------------------------------------------------------

thread_local! {
    static PORTAL_GENERATION: Cell<u64> = const { Cell::new(0) };
    static PORTAL_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static PORTAL_APP: std::cell::RefCell<Option<glib::WeakRef<adw::Application>>> =
        const { std::cell::RefCell::new(None) };
}

/// Whether the desktop offers the GlobalShortcuts portal (GNOME ≥ 48).
pub async fn portal_available() -> bool {
    portal::ensure_registered().await;
    match GlobalShortcuts::new().await {
        Ok(proxy) => portal_answers(&proxy).await,
        Err(_) => false,
    }
}

/// `ashpd` builds proxies even when nobody implements the interface (it assumes version 1),
/// so ask the portal for its `version` property to see whether it really exists.
async fn portal_answers(proxy: &GlobalShortcuts<'_>) -> bool {
    proxy.get_property::<u32>("version").await.is_ok()
}

/// Whether Annota's shortcuts are currently bound through the portal.
pub fn portal_active() -> bool {
    PORTAL_ACTIVE.with(Cell::get)
}

/// At startup: bind the shortcuts through the portal if it exists and the GNOME-keybinding
/// fallback is not in use. The desktop asks the user to confirm the first time.
pub fn start_portal_shortcuts(app: &adw::Application) {
    let settings = gio::Settings::new(config::APP_ID);
    if settings.boolean("gnome-shortcuts-created") {
        return;
    }
    PORTAL_APP.with(|cell| cell.replace(Some(app.downgrade())));
    restart_portal_shortcuts();
}

fn restart_portal_shortcuts() {
    let generation = PORTAL_GENERATION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    });
    let Some(app) = PORTAL_APP.with(|cell| cell.borrow().clone()) else {
        return;
    };
    glib::spawn_future_local(async move {
        if let Err(err) = run_portal_shortcuts(app, generation).await {
            eprintln!("annota: global shortcuts portal not used: {err}");
        }
        if PORTAL_GENERATION.with(Cell::get) == generation {
            PORTAL_ACTIVE.with(|active| active.set(false));
        }
    });
}

async fn run_portal_shortcuts(
    app: glib::WeakRef<adw::Application>,
    generation: u64,
) -> Result<(), ashpd::Error> {
    portal::ensure_registered().await;
    let proxy = GlobalShortcuts::new().await?;
    if !portal_answers(&proxy).await {
        // No GlobalShortcuts portal (GNOME < 48): Preferences offers GNOME shortcuts instead.
        return Ok(());
    }
    let settings = gio::Settings::new(config::APP_ID);
    let shortcuts: Vec<NewShortcut> = configured_accels(&settings)
        .into_iter()
        .filter_map(|(mode, accel)| {
            let accel = Accel::parse(&accel)?;
            let trigger = accel.to_xdg_trigger();
            Some(
                NewShortcut::new(mode.as_str(), mode_title(mode))
                    .preferred_trigger(trigger.as_str()),
            )
        })
        .collect();
    if shortcuts.is_empty() {
        return Ok(());
    }
    let session = proxy.create_session().await?;
    proxy
        .bind_shortcuts(&session, &shortcuts, None)
        .await?
        .response()?;
    PORTAL_ACTIVE.with(|active| active.set(true));

    let mut activated = proxy.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if PORTAL_GENERATION.with(Cell::get) != generation {
            break;
        }
        let (Some(app), Some(mode)) = (app.upgrade(), CaptureMode::parse(event.shortcut_id()))
        else {
            continue;
        };
        app.activate_action("capture", Some(&mode.as_str().to_variant()));
    }
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn accelerators_normalise() {
        assert!(same_accel("<Alt>Print", "<alt>print"));
        assert!(same_accel("<Shift><Control>s", "<Primary><Shift>S"));
        assert!(same_accel("<Ctrl>a", "<Control>a"));
        assert!(!same_accel("<Shift>Print", "Print"));
        assert!(!same_accel("", ""));
        assert_eq!(
            Accel::parse("<Super><Shift>w").unwrap().to_gtk(),
            "<Shift><Super>w"
        );
        assert!(Accel::parse("<Bogus>x").is_none());
        assert!(Accel::parse("<Shift>").is_none());
    }

    #[test]
    fn xdg_triggers_for_portal() {
        assert_eq!(Accel::parse("Print").unwrap().to_xdg_trigger(), "Print");
        assert_eq!(
            Accel::parse("<Alt>Print").unwrap().to_xdg_trigger(),
            "ALT+Print"
        );
        assert_eq!(
            Accel::parse("<Super><Shift>s").unwrap().to_xdg_trigger(),
            "SHIFT+LOGO+s"
        );
    }

    #[test]
    fn d9_defaults_are_gnome_print_keys() {
        let schema = include_str!("../../data/io.github.trosinde.Annota.gschema.xml");
        for (mode, default) in [
            (CaptureMode::Region, "'Print'"),
            (CaptureMode::Window, "'&lt;Alt&gt;Print'"),
            (CaptureMode::Screen, "'&lt;Shift&gt;Print'"),
        ] {
            let key = format!("<key name=\"{}\" type=\"s\">", settings_key(mode));
            let at = schema.find(&key).expect("shortcut key in schema");
            let default_tag = format!("<default>{default}</default>");
            let next_key = schema[at + key.len()..]
                .find("<key ")
                .map_or(schema.len(), |i| at + key.len() + i);
            assert!(
                schema[at..next_key].contains(&default_tag),
                "{} should default to {default}",
                settings_key(mode)
            );
        }
    }

    #[test]
    fn gnome_keybinding_runs_annota_capture() {
        // Scenario "Hotkey on GNOME 46": a GNOME custom keybinding for Print is created and
        // starts a region capture.
        let accels = vec![
            (CaptureMode::Region, s("Print")),
            (CaptureMode::Window, s("<Alt>Print")),
            (CaptureMode::Screen, s("")),
        ];
        let entries = keybinding_entries(&accels, "annota", |m| format!("Annota: {}", m.as_str()));
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            KeybindingEntry {
                path: s("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/annota-region/"),
                name: s("Annota: region"),
                command: s("annota --capture region"),
                binding: s("Print"),
            }
        );
        assert_eq!(entries[1].command, "annota --capture window");
        assert_eq!(entries[1].binding, "<Alt>Print");
        // The command is accepted by Annota's own command line.
        let mode = entries[0].command.rsplit(' ').next().unwrap();
        assert_eq!(CaptureMode::parse(mode), Some(CaptureMode::Region));
    }

    #[test]
    fn program_name_prefers_path() {
        let path_env = std::ffi::OsString::from("/usr/local/bin:/usr/bin");
        assert_eq!(
            program_for_keybinding(Some(Path::new("/usr/bin/annota")), Some(&path_env)),
            "annota"
        );
        assert_eq!(
            program_for_keybinding(
                Some(Path::new("/home/u/dev/_build/src/annota")),
                Some(&path_env)
            ),
            "/home/u/dev/_build/src/annota"
        );
        assert_eq!(
            program_for_keybinding(Some(Path::new("/home/u/my dev/annota")), Some(&path_env)),
            "'/home/u/my dev/annota'"
        );
        assert_eq!(program_for_keybinding(None, None), "annota");
    }

    #[test]
    fn custom_keybinding_list_is_merged_not_replaced() {
        let existing = vec![s("/custom0/"), s("/annota-region/")];
        let merged = merge_paths(
            &existing,
            &[s("/annota-region/"), s("/annota-window/")],
            &[s("/annota-screen/")],
        );
        assert_eq!(
            merged,
            vec![s("/custom0/"), s("/annota-region/"), s("/annota-window/")]
        );
        let removed = merge_paths(&merged, &[], &[s("/annota-window/")]);
        assert_eq!(removed, vec![s("/custom0/"), s("/annota-region/")]);
    }

    fn gnome_defaults() -> Vec<(String, Vec<String>)> {
        vec![
            (s("show-screenshot-ui"), vec![s("Print")]),
            (s("screenshot-window"), vec![s("<Alt>Print")]),
            (s("screenshot"), vec![s("<Shift>Print"), s("<Super>F12")]),
        ]
    }

    #[test]
    fn takeover_moves_gnome_bindings_aside() {
        let ours = vec![s("Print"), s("<Alt>Print"), s("<Shift>Print")];
        let takeover = plan_takeover(&gnome_defaults(), &ours);
        assert_eq!(
            takeover.updates,
            vec![
                (s("show-screenshot-ui"), vec![]),
                (s("screenshot-window"), vec![]),
                (s("screenshot"), vec![s("<Super>F12")]),
            ]
        );
        assert_eq!(
            takeover.moved,
            vec![
                (s("show-screenshot-ui"), s("Print")),
                (s("screenshot-window"), s("<Alt>Print")),
                (s("screenshot"), s("<Shift>Print")),
            ]
        );
    }

    #[test]
    fn takeover_leaves_unrelated_keys_alone() {
        let takeover = plan_takeover(&gnome_defaults(), &[s("<Control><Alt>c")]);
        assert_eq!(takeover, Takeover::default());
    }

    #[test]
    fn restore_gives_keys_back() {
        // Reversible (D9): disabling Annota's Print shortcut gives GNOME its binding back.
        let ours = vec![s("Print"), s("<Shift>Print")];
        let takeover = plan_takeover(&gnome_defaults(), &ours);
        let after: Vec<(String, Vec<String>)> = gnome_defaults()
            .into_iter()
            .map(|(k, v)| {
                let updated = takeover.updates.iter().find(|(uk, _)| *uk == k);
                (k, updated.map(|(_, nv)| nv.clone()).unwrap_or(v))
            })
            .collect();
        let (updates, remaining) = plan_restore(&after, &takeover.moved, &[s("Print")]);
        assert_eq!(updates, vec![(s("show-screenshot-ui"), vec![s("Print")])]);
        assert_eq!(remaining, vec![(s("screenshot"), s("<Shift>Print"))]);
        let (updates, remaining) = plan_restore(&after, &remaining, &[s("<shift>print")]);
        assert_eq!(
            updates,
            vec![(s("screenshot"), vec![s("<Super>F12"), s("<Shift>Print")])]
        );
        assert!(remaining.is_empty());
    }

    #[test]
    fn conflicts_are_found_by_accelerator() {
        let bindings = vec![
            (s("Switch windows"), vec![s("<Alt>Tab")]),
            (s("Take a screenshot"), vec![s("<Shift>Print")]),
        ];
        assert_eq!(
            find_user(&bindings, "<shift>print"),
            Some("Take a screenshot")
        );
        assert_eq!(find_user(&bindings, "<Super>w"), None);
    }
}
