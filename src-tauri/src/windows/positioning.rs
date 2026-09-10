use serde::{Deserialize, Serialize};
use specta::Type;

const DOCK_SNAP_DISTANCE_PX: i32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

impl PhysicalPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

impl PhysicalSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn width_as_i32(self) -> i32 {
        if self.width > i32::MAX as u32 {
            i32::MAX
        } else {
            self.width as i32
        }
    }

    fn height_as_i32(self) -> i32 {
        if self.height > i32::MAX as u32 {
            i32::MAX
        } else {
            self.height as i32
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorkArea {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorWorkArea {
    pub name: String,
    pub work_area: WorkArea,
}

impl MonitorWorkArea {
    pub fn new(name: impl Into<String>, work_area: WorkArea) -> Self {
        Self {
            name: name.into(),
            work_area,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OrbPosition {
    pub monitor_name: String,
    pub physical_x: i32,
    pub physical_y: i32,
    pub edge: Option<DockEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DockDto {
    pub monitor_name: String,
    pub physical_x: i32,
    pub physical_y: i32,
    pub edge: Option<DockEdge>,
}

impl From<OrbPosition> for DockDto {
    fn from(position: OrbPosition) -> Self {
        Self {
            monitor_name: position.monitor_name,
            physical_x: position.physical_x,
            physical_y: position.physical_y,
            edge: position.edge,
        }
    }
}

impl OrbPosition {
    pub fn new(
        monitor_name: impl Into<String>,
        physical_x: i32,
        physical_y: i32,
        edge: Option<DockEdge>,
    ) -> Self {
        Self {
            monitor_name: monitor_name.into(),
            physical_x,
            physical_y,
            edge,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockedPosition {
    pub position: OrbPosition,
    pub edge: Option<DockEdge>,
}

pub fn clamp(position: PhysicalPoint, size: PhysicalSize, work_area: WorkArea) -> PhysicalPoint {
    let maximum_x = work_area
        .right
        .saturating_sub(size.width_as_i32())
        .max(work_area.left);
    let maximum_y = work_area
        .bottom
        .saturating_sub(size.height_as_i32())
        .max(work_area.top);

    PhysicalPoint::new(
        position.x.clamp(work_area.left, maximum_x),
        position.y.clamp(work_area.top, maximum_y),
    )
}

pub fn dock(
    position: PhysicalPoint,
    size: PhysicalSize,
    monitor_name: &str,
    work_area: WorkArea,
) -> DockedPosition {
    let clamped = clamp(position, size, work_area);
    let width = size.width_as_i32();
    let height = size.height_as_i32();
    let distances = [
        (DockEdge::Left, clamped.x.saturating_sub(work_area.left)),
        (
            DockEdge::Right,
            work_area
                .right
                .saturating_sub(clamped.x.saturating_add(width)),
        ),
        (DockEdge::Top, clamped.y.saturating_sub(work_area.top)),
        (
            DockEdge::Bottom,
            work_area
                .bottom
                .saturating_sub(clamped.y.saturating_add(height)),
        ),
    ];
    let mut nearest_edge = DockEdge::Left;
    let mut nearest_distance = i32::MAX;

    for (edge, distance) in distances {
        if distance < nearest_distance {
            nearest_edge = edge;
            nearest_distance = distance;
        }
    }
    let edge = (nearest_distance <= DOCK_SNAP_DISTANCE_PX).then_some(nearest_edge);
    let snapped = match edge {
        Some(DockEdge::Left) => PhysicalPoint::new(work_area.left, clamped.y),
        Some(DockEdge::Right) => {
            PhysicalPoint::new(work_area.right.saturating_sub(width), clamped.y)
        }
        Some(DockEdge::Top) => PhysicalPoint::new(clamped.x, work_area.top),
        Some(DockEdge::Bottom) => {
            PhysicalPoint::new(clamped.x, work_area.bottom.saturating_sub(height))
        }
        None => clamped,
    };

    DockedPosition {
        position: OrbPosition::new(monitor_name, snapped.x, snapped.y, edge),
        edge,
    }
}

pub fn resolve_saved_position(
    monitors: &[MonitorWorkArea],
    primary_monitor_name: &str,
    saved_position: Option<&OrbPosition>,
    size: PhysicalSize,
) -> Option<OrbPosition> {
    let monitor = saved_position
        .and_then(|saved| {
            monitors
                .iter()
                .find(|monitor| monitor.name == saved.monitor_name)
        })
        .or_else(|| {
            monitors
                .iter()
                .find(|monitor| monitor.name == primary_monitor_name)
        })?;
    let point = saved_position.map_or(
        PhysicalPoint::new(monitor.work_area.left, monitor.work_area.top),
        |saved| PhysicalPoint::new(saved.physical_x, saved.physical_y),
    );
    let clamped = clamp(point, size, monitor.work_area);

    Some(OrbPosition::new(
        &monitor.name,
        clamped.x,
        clamped.y,
        saved_position.and_then(|saved| saved.edge),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        clamp, dock, resolve_saved_position, DockEdge, MonitorWorkArea, OrbPosition, PhysicalPoint,
        PhysicalSize, WorkArea,
    };

    const ORB_SIZE: PhysicalSize = PhysicalSize::new(240, 44);

    fn primary() -> MonitorWorkArea {
        MonitorWorkArea::new("DISPLAY1", WorkArea::new(0, 0, 1_000, 800))
    }

    #[test]
    fn clamps_an_orb_to_the_work_area_instead_of_the_monitor_bounds() {
        let position = clamp(PhysicalPoint::new(999, 799), ORB_SIZE, primary().work_area);

        assert_eq!(position, PhysicalPoint::new(760, 756));
    }

    #[test]
    fn snaps_at_the_exact_docking_threshold() {
        let docked = dock(
            PhysicalPoint::new(64, 200),
            ORB_SIZE,
            &primary().name,
            primary().work_area,
        );

        assert_eq!(docked.edge, Some(DockEdge::Left));
        assert_eq!(docked.position.physical_x, 0);
    }

    #[test]
    fn snaps_one_pixel_inside_the_docking_threshold() {
        let docked = dock(
            PhysicalPoint::new(63, 200),
            ORB_SIZE,
            &primary().name,
            primary().work_area,
        );

        assert_eq!(docked.edge, Some(DockEdge::Left));
        assert_eq!(docked.position.physical_x, 0);
    }

    #[test]
    fn keeps_a_position_undocked_beyond_the_docking_threshold() {
        let docked = dock(
            PhysicalPoint::new(65, 200),
            ORB_SIZE,
            &primary().name,
            primary().work_area,
        );

        assert_eq!(docked.edge, None);
        assert_eq!(docked.position.physical_x, 65);
    }

    #[test]
    fn falls_back_to_the_primary_monitor_when_the_saved_monitor_is_missing() {
        let saved = OrbPosition::new("DISCONNECTED", 1_500, 10, Some(DockEdge::Right));

        let primary = primary();
        let resolved = resolve_saved_position(
            std::slice::from_ref(&primary),
            &primary.name,
            Some(&saved),
            ORB_SIZE,
        );

        assert_eq!(
            resolved,
            Some(OrbPosition::new("DISPLAY1", 760, 10, Some(DockEdge::Right)))
        );
    }

    #[test]
    fn returns_none_when_no_primary_monitor_is_available() {
        let resolved = resolve_saved_position(&[], "DISPLAY1", None, ORB_SIZE);

        assert_eq!(resolved, None);
    }
}
