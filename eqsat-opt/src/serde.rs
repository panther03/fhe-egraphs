use egg::*;
use indexmap::IndexMap;
use std::{collections::HashMap, fmt::Display, fs::File, io::{BufWriter, Write}};
use egraph_serialize::EnodeBits;

use crate::common::{Prop,PropId};

pub fn decode_op_string(op: &str) -> PropId {
    let op0 = op.chars().nth(0).unwrap();
    match op0 {
        '*' => PropId::And,
        '!' => PropId::Not,
        '^' => PropId::Xor,
        't' => PropId::Lit,
        'f' => PropId::Lit,
        _   => PropId::Sym,
    }
}

pub fn decode_enode(new_to_old: &HashMap<Id, Id>, enode: &egraph_serialize::Node) -> Option<Prop> {
    match crate::serde::decode_op_string(&enode.op) {
        PropId::And => {
            let a = Id::from(enode.children[0].class() as usize);
            let b = Id::from(enode.children[1].class() as usize);
            Some(Prop::And([*new_to_old.get(&a)?, *new_to_old.get(&b)?]))
        }
        PropId::Xor => {
            let a = Id::from(enode.children[0].class() as usize);
            let b = Id::from(enode.children[1].class() as usize);
            Some(Prop::Xor([*new_to_old.get(&a)?, *new_to_old.get(&b)?]))
        }
        PropId::Not => {
            let a = Id::from(enode.children[0].class() as usize);
            Some(Prop::Not(*new_to_old.get(&a)?))
        }
        PropId::Lit => {
            Some(Prop::Bool(enode.op.as_str() == "true"))
        }
        PropId::Sym => {
            Some(Prop::Symbol(enode.op.clone().into()))
        }
    }
}

/// Preconditions:
/// 1. `egraph` is the initial e-graph.
/// 
/// 2. `new_to_old` contains a mapping from the canonicalized class in the saturated network (which
/// should be what appears in the serialized e-graph) to a class in the original network, where it applies.
/// For example, the root e-class may have a different canonical ID, but it is sure to be the same thing.
///
/// 3. `egraph_ser` is the serialized graph from saturating `egraph`.
///
/// 4. `extraction_result` is a topologically sorted map (so, starting from PIs)
/// containing the selection of Classes -> Nodes. It also *only* contains e-classes that should
/// be in the final network.
pub fn deserialize_into_existing(
    egraph: &mut EGraph<Prop, ()>,
    new_to_old: &mut HashMap<Id, Id>,
    egraph_ser: egraph_serialize::EGraph,
    extraction_result: &IndexMap<egraph_serialize::ClassId,(usize,egraph_serialize::NodeId)>
) {
    for (eclassid,(_,enodeid)) in extraction_result.iter() {
        let enode = &egraph_ser[enodeid];
        //dbg!(eclassid);
        //dbg!(&enode.children);
        // This is the current e-class ID in the domain of the saturated e-graph.
        let newid = Id::from(eclassid.class() as usize);
        // First we need to deserialize the e-node into a Prop object (ADT containing e-class IDs as children.)
        // However, the e-class IDs are in the domain of the saturated e-graph.
        // We cannot add the node if it references unknown e-class IDs.
        // So, decode_enode also maps the saturated ("new") e-class IDs to ones
        // that exist in this e-graph.
        // Because it is topologically sorted, we are sure the children are all in the
        // e-graph. The node is safe to add.
        let id = egraph.add(decode_enode(new_to_old, enode).unwrap());
        // We may be adding a node that was part of a class that also exists in the original.
        // In this case, we should union the original class with the class we just added.
        match new_to_old.get(&newid) {
            Some(existing_id) => { egraph.union(id, *existing_id); }
            None => {}
        }
        // We need to update this mapping because we are now creating new e-classes in the original.
        // We have to be able to resolve references in the saturated network to the nodes we are creating.
        new_to_old.insert(newid, id);
    }
    
    /*else {
        for (eclassid, eclass) in egraph_ser.classes() {
            for enodeid in &eclass.nodes {
                let enode = &egraph_ser[enodeid];
                let id = egraph.add(decode_enode(enode));
                match new_to_old.get(&Id::from(eclassid.class() as usize)) {
                    Some(existing_id) => { dbg!("a"); egraph.union(id, *existing_id); dbg!("b"); }
                    None => {}
                }
            }
        }
    }*/

    //egraph_ser.root_eclasses
    //    .into_iter()
    //    .map(|c| {*new_to_old.get(&Id::from(c.class() as usize)).unwrap()})
    //    .collect()
}

#[allow(unused)]
pub fn serialize_in_mem<L, A>(egraph: EGraph<L, A>, root_eclasses: &Vec<Id>) -> egraph_serialize::EGraph
where
    L: Language + Display,
    A: Analysis<L>,
{
    use egraph_serialize::*;
    let mut out = EGraph::default();
    for class in egraph.classes() {
        for (i, node) in class.nodes.iter().enumerate() {
            out.add_node(
                format!("{}.{}", class.id, i),
                Node {
                    op: node.to_string(),
                    children: node
                        .children()
                        .iter()
                        .map(|id| NodeId::from(format!("{}.0", id)))
                        .collect(),
                    eclass: ClassId::from(format!("{}", class.id)),
                    cost: Cost::new(1.0).unwrap(), //
                    subsumed: false
                },
            )
        }
    }
    out.root_eclasses = root_eclasses.iter().map(|x| x.to_string().into()).collect();
    out
}

/*fn serialize_number_varlen(mut n: u32, writer: &mut impl Write) {
    let bytecount = (4 - n.leading_zeros() / 8).max(1);
    writer.write(&[bytecount as u8]);
    // little endian
    for _ in 0..bytecount {
        writer.write(&[(n & 0xFF) as u8]);
        n >>= 8;
    }
}*/

fn serialize_number(n: u32, writer: &mut impl Write) {
    writer.write(&n.to_le_bytes()).unwrap();
}

pub fn serialize_to_binfile<'a, L, F, A>(egraph: &EGraph<L, A>, root_eclasses: impl ExactSizeIterator<Item=&'a Id>, out_file: &mut File, cost_function: F) -> std::io::Result<()>
where
    L: Language + Display,
    F: Fn(&L) -> f64,
    A: Analysis<L>
{
    let mut writer = BufWriter::new(out_file);
    serialize_number(root_eclasses.len() as u32, &mut writer);
    for root_eclass in root_eclasses {
        let ecid: usize = (*root_eclass).into();
        serialize_number(ecid as u32, &mut writer);
    }
    for class in egraph.classes() {
        let ecid: usize = (class.id).into();
        for (i, node) in class.nodes.iter().enumerate() {
            let mut metadata: u8 = node.children().len() as u8;
            if metadata >= 32 {
                continue;
                panic!();
            }
            if cost_function(node) > 0.0 {
                metadata |= 1 << 6;
            }
            let node_string = node.to_string();
            let ser_enode = EnodeBits {
                eclass: ecid as u32,
                enode: i as u32,
                op: node_string.as_str(),
                children: node.children().iter().map(|c| -> u32 {let d: usize = (*c).into(); d as u32} ).collect(),
                metadata: metadata 
            };
            let encoded: Vec<u8> = bitcode::encode(&ser_enode);
            // uses 1 byte if 127 bytes or less (common case)
            // otherwise just uses full u32
            if encoded.len() >= 0x80 {
                let fixed_len = encoded.len() as u32;
                let fixed_len = ((fixed_len & 0xFFFFFF80) << 1) + 0x80 + (fixed_len & 0x7F);
                serialize_number(fixed_len, &mut writer);
            } else {
                writer.write(&[encoded.len() as u8])?;
            }
            writer.write(&encoded)?;
        }
    }
    Ok(())
}