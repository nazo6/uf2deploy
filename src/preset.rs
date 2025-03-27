use std::{collections::HashMap, sync::LazyLock};

use crate::parse_int;

pub struct Uf2Preset {
    pub id: u32,
    pub description: String,
}

pub static UF2_PRESETS: LazyLock<HashMap<String, Uf2Preset>> = LazyLock::new(|| {
    let mut presets = HashMap::new();

    #[derive(serde::Deserialize)]
    struct Uf2FamiliyData {
        id: String,
        short_name: String,
        description: String,
    }
    let uf2_families =
        serde_json::from_str::<Vec<Uf2FamiliyData>>(include_str!("../uf2families.json")).unwrap();

    for family in uf2_families {
        let id = parse_int(&family.id).unwrap();
        presets.insert(
            family.short_name.to_lowercase(),
            Uf2Preset {
                id,
                description: family.description,
            },
        );
    }

    presets
});
