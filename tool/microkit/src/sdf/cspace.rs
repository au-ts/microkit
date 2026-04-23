//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use super::consts::*;
use super::util::{check_attributes, checked_lookup, loc_string, sdf_parse_number, value_error};
use super::{SdfLocation, SdfNode, XmlSystemDescription};

use crate::util::str_to_bool;

#[derive(Debug, PartialEq, Eq)]
pub struct CNode {
    pub name: String,
    pub size_bits: u8,
    pub post_capdl_untypeds: bool,
}

impl CNode {
    pub(super) fn from_xml(xml_sdf: &XmlSystemDescription, node: &dyn SdfNode) -> Result<CNode, String> {
        check_attributes(xml_sdf, node, &["name", "size_bits", "post_capdl_untypeds"])?;

        let name = checked_lookup(xml_sdf, node, "name")?.to_string();

        let post_capdl_untypeds = if let Some(xml_post_capdl_untypeds) = node.attribute("post_capdl_untypeds") {
            match str_to_bool(xml_post_capdl_untypeds) {
                Some(val) => val,
                None => {
                    return Err(value_error(
                        xml_sdf,
                        node,
                        "post_capdl_untypeds must be 'true' or 'false'".to_string(),
                    ))
                }
            }
        } else {
            false
        };

        let size_bits = sdf_parse_number(checked_lookup(xml_sdf, node, "size_bits")?, node)? as u8;

        Ok(CNode {
            name,
            size_bits,
            post_capdl_untypeds,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CapMapSource{
    Pd(String),
    CNode(String),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum CapMapType {
    Tcb,
    Sc,
    VSpace,
    CSpace,
    CNode,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CapMap {
    pub cap_type: CapMapType,
    pub source: CapMapSource,
    // The destination "slot" in the CSpace: note that this is "opaque" and
    // can be shifted depending on the location in the CSpace to work as the CPtr,
    // but here it is given as the index into the CNode.
    pub slot: u64,
    /// Location in the parsed SDF file
    pub text_pos: SdfLocation,
}

#[derive(Debug)]
pub struct CSpace {
    pub cap_maps: Vec<CapMap>,
}

impl CapMap {
    fn from_xml(
        cap_type: CapMapType,
        xml_sdf: &XmlSystemDescription,
        node: &dyn SdfNode,
    ) -> Result<CapMap, String> {
        check_attributes(xml_sdf, node, &["slot", "pd", "cnode_name"])?;

        let pd_name_maybe = node.attribute("pd").map(|pd_name_str| pd_name_str.to_string());

        let cnode_name_maybe = node.attribute("cnode_name").map(|cnode_name_str| cnode_name_str.to_string());

        let source = match (pd_name_maybe, cnode_name_maybe) {
            (Some(pd_name), None) => {
                if cap_type == CapMapType::CNode {
                    return Err(value_error(
                        xml_sdf,
                        node,
                        format!("invalid parameter 'pd' for target CapMapType"),
                    ))
                }

                CapMapSource::Pd(pd_name)
            },
            (None, Some(cnode_name)) => {
                if cap_type != CapMapType::CNode {
                    return Err(value_error(
                        xml_sdf,
                        node,
                        format!("invalid parameter 'cnode_name' for target CapMapType"),
                    ))
                }

                CapMapSource::CNode(cnode_name)
            },
            (Some(_), Some(_)) => return Err(value_error(
                xml_sdf,
                node,
                format!("'pd' and 'cnode_name' cannot be both specified"),
            )),
            (None, None) => return Err(value_error(
                xml_sdf,
                node,
                format!("Either 'pd' or 'cnode_name' should be specified"),
            )),
        };

        let slot = sdf_parse_number(checked_lookup(xml_sdf, node, "slot")?, node)?;

        if slot == 0 {
            return Err(value_error(
                xml_sdf,
                node,
                ("The destination slot 0 has been reserved for Microkit CNode").to_string(),
            ));
        }

        // TODO: Rework this so that we don't have a fixed upper limit.
        if slot >= CAP_MAP_MAX_SLOT {
            return Err(value_error(
                xml_sdf,
                node,
                format!("There are only {CAP_MAP_MAX_SLOT} destination cspace slots available."),
            ));
        }

        Ok(CapMap {
            cap_type,
            source,
            slot,
            text_pos: node.range().start,
        })

    }
}

impl CSpace {
    pub(super) fn from_xml(
        xml_sdf: &XmlSystemDescription,
        node: &dyn SdfNode,
    ) -> Result<Self, String> {
        check_attributes(xml_sdf, node, &[])?;

        let mut cap_maps = vec![];

        for child in node.children() {
            cap_maps.push(match child.tag_name() {
                "cap_tcb" => CapMap::from_xml(CapMapType::Tcb, xml_sdf, &*child)?,
                "cap_sc" => CapMap::from_xml(CapMapType::Sc, xml_sdf, &*child)?,
                "cap_vspace" => CapMap::from_xml(CapMapType::VSpace, xml_sdf, &*child)?,
                "cap_cspace" => CapMap::from_xml(CapMapType::CSpace, xml_sdf, &*child)?,
                "cap_cnode" => CapMap::from_xml(CapMapType::CNode, xml_sdf, &*child)?,
                child_name => {
                    let location = loc_string(xml_sdf, child.range().start);
                    if let Some(type_name) = child_name.strip_prefix("cap_") {
                        return Err(format!("Cap type: '{type_name}' is not supported at '{location}'"));
                    } else {
                        return Err(format!("Element '{child_name}' is not supported in a <cspace> element at '{location}'"));
                    }
                }
            })
        }

        Ok(CSpace { cap_maps })
    }
}
