use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use smithay::utils::{Logical, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};

use crate::layout::{Layout, RemovedTile};
use crate::utils::transaction::Transaction;
use crate::window::{Mapped, ResolvedWindowRules};

pub type Key = (String, String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub pos: (f64, f64),
    pub size: (i32, i32),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pos: Option<(f64, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<(i32, i32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    floating: Option<bool>,
}

#[derive(Debug, Default)]
pub struct FloatingMemory {
    path: Option<PathBuf>,
    entries: HashMap<String, HashMap<String, Entry>>,
}

impl FloatingMemory {
    pub fn load(path: PathBuf) -> Self {
        let entries = match fs::read(&path) {
            Ok(data) => serde_json::from_slice(&data).unwrap_or_else(|err| {
                let bad = path.with_extension("json.bad");
                warn!("error parsing {path:?}, moving it to {bad:?}: {err:?}");
                let _ = fs::rename(&path, bad);
                HashMap::new()
            }),
            Err(_) => HashMap::new(),
        };

        Self {
            path: Some(path),
            entries,
        }
    }

    pub fn key(toplevel: &ToplevelSurface) -> Option<Key> {
        if toplevel.parent().is_some() {
            return None;
        }

        with_states(toplevel.wl_surface(), |states| {
            let role = states
                .data_map
                .get::<XdgToplevelSurfaceData>()?
                .lock()
                .unwrap();
            let app_id = role.app_id.clone()?;
            let title = role.title.clone().unwrap_or_default();
            Some((app_id, title))
        })
    }

    pub fn get(&self, key: &Key) -> Option<Geometry> {
        let entry = self.entries.get(&key.0)?.get(&key.1)?;
        Some(Geometry {
            pos: entry.pos?,
            size: entry.size?,
        })
    }

    pub fn open_floating(&self, rules: &ResolvedWindowRules, toplevel: &ToplevelSurface) -> bool {
        if rules.open_floating.is_none() {
            let remembered = Self::key(toplevel).and_then(|key| self.remembered_floating(&key));
            if let Some(floating) = remembered {
                return floating;
            }
        }

        rules.compute_open_floating(toplevel)
    }

    fn remembered_floating(&self, key: &Key) -> Option<bool> {
        self.entries.get(&key.0)?.get(&key.1)?.floating
    }

    pub fn remember(
        &mut self,
        removed: &RemovedTile<Mapped>,
        size_before_unmap: Option<Size<i32, Logical>>,
    ) {
        let mapped = removed.window();
        let Some((app_id, title)) = mapped.floating_memory_key.clone() else {
            return;
        };

        let previous = self
            .entries
            .get(&app_id)
            .and_then(|titles| titles.get(&title))
            .copied();
        let mut entry = previous.unwrap_or_default();

        if removed.is_floating() != mapped.opened_floating {
            entry.floating = Some(removed.is_floating());
        }

        if let Some((pos, size)) = removed.floating_geometry() {
            let size = size_before_unmap.unwrap_or(size);
            if size.w > 0 && size.h > 0 {
                entry.pos = Some((pos.x, pos.y));
                entry.size = Some((size.w, size.h));
            }
        }

        if previous != Some(entry) && entry != Entry::default() {
            self.entries.entry(app_id).or_default().insert(title, entry);
            self.save();
        }
    }

    pub fn remember_all(&mut self, layout: &mut Layout<Mapped>) {
        let windows: Vec<_> = layout
            .windows()
            .map(|(_, mapped)| mapped.window.clone())
            .collect();
        for window in windows {
            if let Some(removed) = layout.remove_window(&window, Transaction::new()) {
                self.remember(&removed, None);
            }
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };

        let data = serde_json::to_vec_pretty(&self.entries).unwrap();
        let tmp = path.with_extension("json.tmp");
        let res = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&tmp, data))
            .and_then(|()| fs::rename(&tmp, path));
        if let Err(err) = res {
            warn!("error saving {path:?}: {err:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use xshell::Shell;

    use super::*;

    #[test]
    fn load_malformed_and_old_files() {
        let sh = Shell::new().unwrap();
        let dir = sh.create_temp_dir().unwrap();
        let key = ("a".to_string(), "t".to_string());
        for (name, data, expect) in [
            ("junk", &b"\x00\xff not json"[..], false),
            ("empty", &b""[..], false),
            (
                "truncated",
                &br#"{"a":{"t":{"pos":[0.1,0.2],"size":[3"#[..],
                false,
            ),
            (
                "old",
                &br#"{"a":{"t":{"pos":[0.1,0.2],"size":[300,200]}}}"#[..],
                true,
            ),
            ("flagonly", &br#"{"a":{"t":{"floating":true}}}"#[..], false),
            (
                "badtype",
                &br#"{"a":{"t":{"pos":[0.1,0.2],"size":[300,200],"floating":"yes"}}}"#[..],
                false,
            ),
            (
                "extra",
                &br#"{"a":{"t":{"pos":[0.1,0.2],"size":[300,200],"future":1}}}"#[..],
                true,
            ),
            ("array", &b"[]"[..], false),
        ] {
            let path = dir.path().join(name);
            fs::write(&path, data).unwrap();
            let memory = FloatingMemory::load(path);
            assert_eq!(memory.get(&key).is_some(), expect, "{name}");
        }

        let memory = FloatingMemory::load(dir.path().join("missing"));
        assert!(memory.get(&key).is_none());
    }
}
