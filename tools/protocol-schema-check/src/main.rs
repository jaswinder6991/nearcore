/// Tool which is able to iterate over all structs and check their hashes.
/// Iteration is done by `ProtocolStructInfo`s generated by `ProtocolStruct`
/// macro.
// TODO (#11755): add to CI.

// Needed because otherwise tool doesn't notice `ProtocolStructInfo`s from
// other crates.
#[allow(unused_imports)]
use near_primitives::*;

use near_stable_hasher::StableHasher;
use near_structs_checker_lib::ProtocolStructInfo;
use std::any::TypeId;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

#[allow(unused)]
fn compute_hash(
    info: &ProtocolStructInfo,
    structs: &BTreeMap<TypeId, &'static ProtocolStructInfo>,
) -> u32 {
    let mut hasher = StableHasher::new();
    match info {
        ProtocolStructInfo::Struct { name, type_id: _, fields } => {
            name.hash(&mut hasher);
            for (field_name, field_type_id) in *fields {
                field_name.hash(&mut hasher);
                compute_type_hash(*field_type_id, structs, &mut hasher);
            }
        }
        ProtocolStructInfo::Enum { name, type_id: _, variants } => {
            name.hash(&mut hasher);
            for (variant_name, variant_fields) in *variants {
                variant_name.hash(&mut hasher);
                if let Some(fields) = variant_fields {
                    for (field_name, field_type_id) in *fields {
                        field_name.hash(&mut hasher);
                        compute_type_hash(*field_type_id, structs, &mut hasher);
                    }
                }
            }
        }
    }
    hasher.finish() as u32
}

fn compute_type_hash(
    type_id: TypeId,
    structs: &BTreeMap<TypeId, &'static ProtocolStructInfo>,
    hasher: &mut StableHasher,
) {
    if let Some(nested_info) = structs.get(&type_id) {
        compute_hash(nested_info, structs).hash(hasher);
    } else {
        // Likely a primitive or external type.
        // TODO (#11755): proper implementation for generics. Or require a
        // separate type for them.
        0.hash(hasher);
    }
}

fn main() {
    let file_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("res").join("protocol_structs.toml");

    let stored_hashes: BTreeMap<String, u32> = if file_path.exists() {
        toml::from_str(&fs::read_to_string(&file_path).unwrap_or_else(|_| "".to_string())).unwrap()
    } else {
        BTreeMap::new()
    };

    let structs: BTreeMap<TypeId, &'static ProtocolStructInfo> =
        inventory::iter::<ProtocolStructInfo>
            .into_iter()
            .map(|info| (info.type_id(), info))
            .collect();

    println!("Loaded {} structs", structs.len());

    let mut current_hashes: BTreeMap<String, u32> = Default::default();
    for info in inventory::iter::<ProtocolStructInfo> {
        let hash = compute_hash(info, &structs);
        current_hashes.insert(info.name().to_string(), hash);
    }

    let mut has_changes = false;
    for (name, hash) in &current_hashes {
        match stored_hashes.get(name) {
            Some(stored_hash) if stored_hash != hash => {
                println!("Hash mismatch for {}: stored {}, current {}", name, stored_hash, hash);
                has_changes = true;
            }
            None => {
                println!("New struct: {} with hash {}", name, hash);
                has_changes = true;
            }
            _ => {}
        }
    }

    let current_keys: HashSet<_> = current_hashes.keys().collect();
    let stored_keys: HashSet<_> = stored_hashes.keys().collect();
    for removed in stored_keys.difference(&current_keys) {
        println!("Struct removed: {}", removed);
        has_changes = true;
    }

    if has_changes {
        fs::write(&file_path, toml::to_string_pretty(&current_hashes).unwrap()).unwrap();
        println!("Updated {}", file_path.display());
    } else {
        println!("No changes detected in protocol structs");
    }

    current_hashes.clear();
}
