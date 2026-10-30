use crate::display_preset::{DisplayPlan, DisplayPoint};
use std::collections::{BTreeSet, HashSet};

// Swift: DisplayConfigurationOperations.Operation
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    Unmirror {
        display_id: u32,
    },
    Mirror {
        display_id: u32,
        master_id: u32,
    },
    SetOrigin {
        display_id: u32,
        origin: DisplayPoint,
    },
}

// Swift: DisplayConfigurationOperations
pub struct DisplayConfigurationOperations;

// Swift: DisplayConfigurationOperations.build(from:)
pub fn build(plan: &DisplayPlan) -> Vec<Operation> {
    DisplayConfigurationOperations::build(plan)
}

impl DisplayConfigurationOperations {
    // Swift: DisplayConfigurationOperations.build(from:)
    pub fn build(plan: &DisplayPlan) -> Vec<Operation> {
        let outside_display_ids = plan
            .outside_placements
            .iter()
            .map(|placement| placement.display_id)
            .collect::<HashSet<_>>();
        let unmirror_operations = plan
            .unmirror_display_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|display_id| Operation::Unmirror { display_id });
        let mut mirrors = plan.mirrors.clone();
        mirrors.sort_by_key(|mirror| (mirror.slave_id, mirror.master_id));
        let mirror_operations = mirrors.into_iter().map(|mirror| Operation::Mirror {
            display_id: mirror.slave_id,
            master_id: mirror.master_id,
        });
        let mut preset_display_ids = plan
            .origins_by_master
            .keys()
            .filter(|display_id| !outside_display_ids.contains(display_id))
            .copied()
            .collect::<Vec<_>>();
        preset_display_ids.sort_unstable();
        let preset_origin_operations = preset_display_ids.into_iter().filter_map(|display_id| {
            plan.origins_by_master
                .get(&display_id)
                .copied()
                .map(|origin| Operation::SetOrigin { display_id, origin })
        });
        let outside_origin_operations =
            plan.outside_placements
                .iter()
                .map(|placement| Operation::SetOrigin {
                    display_id: placement.display_id,
                    origin: placement.origin,
                });

        unmirror_operations
            .chain(mirror_operations)
            .chain(preset_origin_operations)
            .chain(outside_origin_operations)
            .collect()
    }
}
