//
// Copyright 2026, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use super::memory_region::{Map, MapPerms, SysIOMapPerms};
use super::util::{
    check_attributes, checked_lookup, loc_string, sdf_parse_required_attribute, value_error,
};
use super::{SdfLocation, SdfNode, SystemDescriptionFile};
use crate::Config;

pub(super) const MAX_GPA: u64 = 1 << 41;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RiscvIODevice {
    pub iommu_paddr: u64,
    pub device_id: u64,
}
#[derive(Debug)]
pub struct RiscvIOAddressSpace {
    pub name: String,
    pub iomaps: Vec<RiscvIOMap>,
}

#[derive(Debug)]
pub struct RiscvIOMap {
    pub mr: String,
    pub iovaddr: u64,
    pub perms: SysIOMapPerms,
    pub text_pos: Option<SdfLocation>,
}

impl Map for RiscvIOMap {
    type Perms = SysIOMapPerms;
    fn mr_name(&self) -> &str {
        &self.mr
    }

    fn addr(&self) -> u64 {
        self.iovaddr
    }

    fn text_pos(&self) -> Option<SdfLocation> {
        self.text_pos
    }

    fn element(&self) -> &'static str {
        "iomap"
    }

    fn addr_name(&self) -> &'static str {
        "iovaddr"
    }

    fn range_name(&self) -> &'static str {
        "io address range"
    }

    fn perms(&self) -> Self::Perms {
        self.perms
    }

    fn cached(&self) -> bool {
        false
    }
}

impl RiscvIOAddressSpace {
    pub(super) fn from_xml(
        config: &Config,
        xml_sdf: &SystemDescriptionFile,
        node: &dyn SdfNode,
        names: &mut BTreeSet<String>,
        space_index: usize,
        devices: &mut BTreeMap<RiscvIODevice, usize>,
    ) -> Result<Self, String> {
        if !config.iommu {
            return Err(format!(
                "Error: io address space requires seL4 to be built with IOMMU support: {}",
                loc_string(xml_sdf, node.range().start)
            ));
        }
        check_attributes(xml_sdf, node, &["name"])?;
        let name = checked_lookup(xml_sdf, node, "name")?.to_string();
        if !names.insert(name.clone()) {
            return Err(value_error(
                xml_sdf,
                node,
                format!("duplicate name '{name}'"),
            ));
        }
        let mut has_device = false;
        let mut iomaps = Vec::new();
        for child in node.children() {
            match child.tag_name() {
                "device" => {
                    check_attributes(xml_sdf, &*child, &["iommu", "peripheral_id"])?;
                    if let Some(nested) = child.children().next() {
                        return Err(value_error(
                            xml_sdf,
                            &*nested,
                            "device cannot contain child elements".into(),
                        ));
                    }
                    let device = RiscvIODevice {
                        iommu_paddr: sdf_parse_required_attribute(xml_sdf, &*child, "iommu")?,
                        device_id: sdf_parse_required_attribute(xml_sdf, &*child, "peripheral_id")?,
                    };
                    if device.device_id >= (1 << 24) || !device.iommu_paddr.is_multiple_of(4096) {
                        return Err(value_error(
                            xml_sdf,
                            &*child,
                            "Max 24 bit device ID on RISCV and page aligned IOMMU MMIO region"
                                .into(),
                        ));
                    }
                    match devices.entry(device) {
                        Entry::Occupied(entry) => {
                            let device = entry.key();
                            return Err(value_error(
                                xml_sdf,
                                &*child,
                                format!(
                                    "Duplicate device peripheral_id 'IOMMU {:#x} device {:#x}'",
                                    device.iommu_paddr, device.device_id
                                ),
                            ));
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(space_index);
                        }
                    }
                    has_device = true;
                }
                "iomap" => {
                    check_attributes(xml_sdf, &*child, &["mr", "iovaddr", "perms"])?;
                    let mr = checked_lookup(xml_sdf, &*child, "mr")?.to_string();
                    let iovaddr = sdf_parse_required_attribute(xml_sdf, &*child, "iovaddr")?;
                    if iovaddr >= MAX_GPA {
                        return Err(value_error(
                            xml_sdf,
                            &*child,
                            format!("iovaddr ({iovaddr:#x}) must be less than {MAX_GPA:#x}"),
                        ));
                    }
                    let perms = SysIOMapPerms::from_str(child.attribute("perms").unwrap_or("rw"))
                        .map_err(|_| {
                        value_error(
                            xml_sdf,
                            &*child,
                            "perms for io mapped memory must only be a combination of 'r' and 'w'"
                                .into(),
                        )
                    })?;
                    if !perms.read() {
                        return Err(value_error(
                            xml_sdf,
                            &*child,
                            "Sv39x4 does not support write only mappings".into(),
                        ));
                    }
                    iomaps.push(RiscvIOMap {
                        mr,
                        iovaddr,
                        perms,
                        text_pos: Some(child.range().start),
                    });
                }
                _ => {
                    let pos = child.range().start;
                    return Err(format!(
                        "Error: invalid XML element '{}': {}",
                        child.tag_name(),
                        loc_string(xml_sdf, pos)
                    ));
                }
            }
        }
        if !has_device {
            return Err(value_error(
                xml_sdf,
                node,
                "io_address_space requires at least one device".into(),
            ));
        }
        Ok(Self { name, iomaps })
    }
}
