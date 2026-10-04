use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use smithay::utils::{Logical, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};

use crate::layout::{Layout, RemovedTile};
use crate::utils::transaction::Transaction;
use crate::window::Mapped;

pub type Key = (String, String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub pos: (f64, f64),
    pub size: (i32, i32),
}

#[derive(Debug, Default)]
pub struct FloatingMemory {
    path: Option<PathBuf>,
    entries: HashMap<String, HashMap<String, Geometry>>,
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
        self.entries.get(&key.0)?.get(&key.1).copied()
    }

    pub fn remember(
        &mut self,
        removed: &RemovedTile<Mapped>,
        size_before_unmap: Option<Size<i32, Logical>>,
    ) {
        let Some((pos, size)) = removed.floating_geometry() else {
            return;
        };
        let size = size_before_unmap.unwrap_or(size);
        if size.w <= 0 || size.h <= 0 {
            return;
        }
        let Some((app_id, title)) = removed.window().floating_memory_key.clone() else {
            return;
        };

        let geometry = Geometry {
            pos: (pos.x, pos.y),
            size: (size.w, size.h),
        };
        let previous = self
            .entries
            .entry(app_id)
            .or_default()
            .insert(title, geometry);
        if previous != Some(geometry) {
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
