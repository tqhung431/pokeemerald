//! Overworld grid: object placements from a map layout plus the asset
//! manifest become a walkable grid with blocking, doors and interactions.
//!
//! Coordinates are tiles; `x` grows east and `y` grows south. A placement's
//! `(x, y)` is the north-west tile of its footprint.

use core_data::{AssetManifest, DataError, Interaction};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub object: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub variant: Option<String>,
    /// Text shown by `read` interactions.
    #[serde(default)]
    pub text: Option<String>,
    /// Warp destination for buildings with doors, e.g. `"house_small_1f"`.
    #[serde(default)]
    pub warp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapLayout {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub spawn: [i32; 2],
    pub placements: Vec<Placement>,
}

impl MapLayout {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DataError> {
        let path = path.as_ref();
        let text =
            std::fs::read_to_string(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
        serde_json::from_str(&text).map_err(|e| DataError::Parse(path.display().to_string(), e))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    North,
    South,
    East,
    West,
}

impl Dir {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::North => (0, -1),
            Dir::South => (0, 1),
            Dir::East => (1, 0),
            Dir::West => (-1, 0),
        }
    }
}

/// Result of trying to step one tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Moved {
        x: i32,
        y: i32,
    },
    Blocked,
    /// Walked into a door tile of this placement.
    Door {
        placement: usize,
    },
}

/// What the player finds when interacting with the tile they face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interact<'a> {
    pub placement: usize,
    pub object: &'a str,
    pub interaction: &'a Interaction,
    pub text: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    /// The blocking placement covering this tile, if any.
    solid: Option<usize>,
    door: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct World {
    layout: MapLayout,
    manifest: AssetManifest,
    cells: Vec<Cell>,
    removed: Vec<bool>,
}

impl World {
    /// Builds the grid, returning every problem in the layout at once.
    pub fn build(layout: MapLayout, manifest: AssetManifest) -> Result<Self, Vec<String>> {
        let mut problems = Vec::new();
        if layout.width <= 0 || layout.height <= 0 {
            return Err(vec![format!("map '{}' has no area", layout.id)]);
        }
        let mut world = World {
            cells: vec![Cell::default(); (layout.width * layout.height) as usize],
            removed: vec![false; layout.placements.len()],
            layout,
            manifest,
        };
        for i in 0..world.layout.placements.len() {
            if let Err(e) = world.stamp(i) {
                problems.push(e);
            }
        }
        let [sx, sy] = world.layout.spawn;
        if !world.walkable(sx, sy) {
            problems.push(format!("spawn ({sx}, {sy}) is not walkable"));
        }
        if problems.is_empty() {
            Ok(world)
        } else {
            Err(problems)
        }
    }

    fn stamp(&mut self, i: usize) -> Result<(), String> {
        let p = &self.layout.placements[i];
        let ctx = format!("placement {i} '{}' at ({}, {})", p.object, p.x, p.y);
        let o = self
            .manifest
            .objects
            .get(&p.object)
            .ok_or_else(|| format!("{ctx}: unknown object"))?;
        if let Some(v) = &p.variant {
            if !o.variants.contains_key(v) {
                return Err(format!("{ctx}: unknown variant '{v}'"));
            }
        }
        let [w, d] = o.footprint;
        let (w, d) = (w as i32, d as i32);
        if p.x < 0 || p.y < 0 || p.x + w > self.layout.width || p.y + d > self.layout.height {
            return Err(format!("{ctx}: footprint leaves the map"));
        }
        if !o.blocking {
            return Ok(());
        }
        let doors: Vec<(i32, i32)> = o
            .doors
            .iter()
            .map(|dr| (p.x + dr.x as i32, p.y + dr.y as i32))
            .collect();
        for y in p.y..p.y + d {
            for x in p.x..p.x + w {
                let idx = self.index(x, y);
                if let Some(other) = self.cells[idx].solid.or(self.cells[idx].door) {
                    return Err(format!("{ctx}: overlaps placement {other} at ({x}, {y})"));
                }
                if doors.contains(&(x, y)) {
                    self.cells[idx].door = Some(i);
                } else {
                    self.cells[idx].solid = Some(i);
                }
            }
        }
        Ok(())
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.layout.width + x) as usize
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.layout.width && y < self.layout.height
    }

    pub fn layout(&self) -> &MapLayout {
        &self.layout
    }
    pub fn manifest(&self) -> &AssetManifest {
        &self.manifest
    }
    pub fn is_removed(&self, placement: usize) -> bool {
        self.removed[placement]
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.cells[self.index(x, y)].solid.is_none()
    }

    pub fn step(&self, x: i32, y: i32, dir: Dir) -> Step {
        let (dx, dy) = dir.delta();
        let (nx, ny) = (x + dx, y + dy);
        if !self.walkable(nx, ny) {
            return Step::Blocked;
        }
        match self.cells[self.index(nx, ny)].door {
            Some(placement) => Step::Door { placement },
            None => Step::Moved { x: nx, y: ny },
        }
    }

    /// The interactive object on the tile in front of `(x, y)`.
    pub fn facing(&self, x: i32, y: i32, dir: Dir) -> Option<Interact<'_>> {
        let (dx, dy) = dir.delta();
        let (fx, fy) = (x + dx, y + dy);
        if !self.in_bounds(fx, fy) {
            return None;
        }
        let placement = self.cells[self.index(fx, fy)].solid?;
        let p = &self.layout.placements[placement];
        let interaction = self.manifest.objects[&p.object].interaction.as_ref()?;
        Some(Interact {
            placement,
            object: &p.object,
            interaction,
            text: p.text.as_deref(),
        })
    }

    /// Removes a cut, smashed or picked-up object, freeing its tile.
    pub fn remove(&mut self, placement: usize) -> bool {
        if self.removed.get(placement) != Some(&false) {
            return false;
        }
        self.removed[placement] = true;
        for c in &mut self.cells {
            if c.solid == Some(placement) {
                c.solid = None;
            }
            if c.door == Some(placement) {
                c.door = None;
            }
        }
        true
    }

    /// Pushes a 1x1 pushable object one tile. Returns its new position.
    pub fn push(&mut self, placement: usize, dir: Dir) -> Option<(i32, i32)> {
        let p = self.layout.placements.get(placement)?;
        let o = &self.manifest.objects[&p.object];
        if self.removed[placement] || o.interaction != Some(Interaction::Push) {
            return None;
        }
        let (dx, dy) = dir.delta();
        let (nx, ny) = (p.x + dx, p.y + dy);
        if !self.walkable(nx, ny) || self.cells[self.index(nx, ny)].door.is_some() {
            return None;
        }
        let old = self.index(p.x, p.y);
        let new = self.index(nx, ny);
        self.cells[old].solid = None;
        self.cells[new].solid = Some(placement);
        let p = &mut self.layout.placements[placement];
        (p.x, p.y) = (nx, ny);
        Some((nx, ny))
    }
}
