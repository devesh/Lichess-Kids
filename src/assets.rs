use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpinRules {
    pub game_rating_offset: i32,
    pub puzzle_rating_offset: i32,
    pub puzzles_per_spin: i32,
}

impl Default for SpinRules {
    fn default() -> Self {
        SpinRules {
            game_rating_offset: -100,
            puzzle_rating_offset: -100,
            puzzles_per_spin: 25,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseMetadata {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemMetadata {
    pub id: String,
    pub name: String,
    pub category: String,
    pub price: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetadataJson {
    pub bases: Vec<BaseMetadata>,
    pub items: Vec<ItemMetadata>,
    pub spin_rules: Option<SpinRules>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseWithSvg {
    pub id: String,
    pub name: String,
    pub svg: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemWithSvg {
    pub id: String,
    pub name: String,
    pub category: String,
    pub price: i32,
    pub svg: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AssetCatalog {
    pub bases: Vec<BaseWithSvg>,
    pub items: Vec<ItemWithSvg>,
    pub spin_rules: SpinRules,
    #[serde(skip)]
    pub bases_map: HashMap<String, String>,
    #[serde(skip)]
    pub items_map: HashMap<String, String>,
}

impl AssetCatalog {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, std::io::Error> {
        let dir = dir.as_ref();
        let metadata_path = dir.join("metadata.json");
        let metadata_content = fs::read_to_string(metadata_path)?;
        let metadata: MetadataJson = serde_json::from_str(&metadata_content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut bases = Vec::new();
        let mut bases_map = HashMap::new();
        for base in metadata.bases {
            let svg_path = dir.join("bases").join(format!("{}.svg", base.id));
            let svg_content = fs::read_to_string(svg_path)?;
            bases.push(BaseWithSvg {
                id: base.id.clone(),
                name: base.name,
                svg: svg_content.clone(),
            });
            bases_map.insert(base.id, svg_content);
        }

        let mut items = Vec::new();
        let mut items_map = HashMap::new();
        for item in metadata.items {
            let svg_path = dir.join("items").join(format!("{}.svg", item.id));
            let svg_content = fs::read_to_string(svg_path).unwrap_or_default();
            items.push(ItemWithSvg {
                id: item.id.clone(),
                name: item.name,
                category: item.category,
                price: item.price,
                svg: svg_content.clone(),
            });
            items_map.insert(item.id, svg_content);
        }

        let spin_rules = metadata.spin_rules.unwrap_or_default();

        Ok(AssetCatalog {
            bases,
            items,
            spin_rules,
            bases_map,
            items_map,
        })
    }
}
