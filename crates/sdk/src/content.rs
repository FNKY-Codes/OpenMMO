use serde::{Deserialize, Serialize};

use crate::{ItemId, NpcId, ObjectId, QuestId, RegionId, TilePos};

fn default_attack_ticks() -> u32 {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: String,
    pub stackable: bool,
    pub equip_slot: Option<EquipSlot>,
    pub prowess_bonus: i32,
    #[serde(default)]
    pub electrics_bonus: i32,
    pub fortitude_bonus: i32,
    pub tool_tag: Option<crate::ToolTag>,
    pub alchemy_value: u32,
    #[serde(default = "default_attack_ticks")]
    pub attack_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    Head,
    Body,
    Legs,
    Weapon,
    Shield,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item_id: ItemId,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Inventory {
    pub slots: Vec<Option<InventorySlot>>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            capacity,
        }
    }

    pub fn add_item(&mut self, item_id: ItemId, quantity: u32, stackable: bool) -> u32 {
        let mut remaining = quantity;
        if stackable {
            for s in self.slots.iter_mut().flatten() {
                if s.item_id == item_id {
                    s.quantity += remaining;
                    return 0;
                }
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() && remaining > 0 {
                let qty = if stackable { remaining } else { 1 };
                *slot = Some(InventorySlot {
                    item_id,
                    quantity: qty,
                });
                remaining -= qty;
                if !stackable {
                    continue;
                }
                if remaining == 0 {
                    return 0;
                }
            }
        }
        remaining
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcDef {
    pub id: NpcId,
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    pub max_hp: u32,
    pub prowess: u32,
    pub fortitude: u32,
    pub aggro_range: i32,
    pub respawn_ticks: u32,
    #[serde(default = "default_attack_ticks")]
    pub attack_ticks: u32,
    #[serde(default = "default_footprint_one")]
    pub footprint_w: u32,
    #[serde(default = "default_footprint_one")]
    pub footprint_h: u32,
    pub loot_table: Vec<LootEntry>,
}

fn default_footprint_one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootEntry {
    pub item_id: ItemId,
    pub min_qty: u32,
    pub max_qty: u32,
    pub rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDef {
    pub id: ObjectId,
    pub name: String,
    pub harvest_tag: Option<crate::HarvestTag>,
    pub scavenging_level: u32,
    pub scavenging_xp: u64,
    pub harvest_item: Option<ItemId>,
    pub scavenging_ticks: u32,
    pub depletes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinementRecipe {
    pub id: String,
    pub name: String,
    pub fabrication_level: u32,
    pub fabrication_xp: u64,
    pub inputs: Vec<RecipeInput>,
    pub output: ItemId,
    pub output_qty: u32,
    pub ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeInput {
    pub item_id: ItemId,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellDef {
    pub id: String,
    pub name: String,
    pub electrics_level: u32,
    pub max_hit: u32,
    pub xp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionTransition {
    pub position: TilePos,
    pub target_region: RegionId,
    pub target_spawn: TilePos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionDef {
    pub id: RegionId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub spawn: TilePos,
    pub tiles: Vec<u8>,
    pub objects: Vec<RegionObject>,
    pub npcs: Vec<RegionNpc>,
    #[serde(default)]
    pub transitions: Vec<RegionTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionObject {
    pub object_id: ObjectId,
    pub position: TilePos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionNpc {
    pub npc_id: NpcId,
    pub position: TilePos,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopDef {
    pub id: String,
    pub name: String,
    pub stock: Vec<ShopStock>,
    pub restock_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopStock {
    pub item_id: ItemId,
    pub price: u32,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueNode {
    pub id: String,
    pub text: String,
    pub options: Vec<DialogueOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueOption {
    pub label: String,
    pub next: Option<String>,
    pub action: Option<DialogueAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DialogueAction {
    StartQuest { quest_id: QuestId },
    AdvanceQuest { quest_id: QuestId, stage: u32 },
    OpenShop { shop_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDef {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    pub stages: Vec<QuestStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestStage {
    pub stage: u32,
    pub description: String,
    pub objective: QuestObjective,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestObjective {
    TalkToNpc { npc_id: NpcId },
    Harvest { tag: crate::HarvestTag, count: u32 },
    Refine { recipe_id: String, count: u32 },
    KillNpc { npc_id: NpcId, count: u32 },
    ReachSkillLevel { skill: crate::Skill, level: u32 },
    VisitTile { position: TilePos, radius: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDef {
    pub skill: crate::Skill,
    pub calling: crate::Calling,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecializationDef {
    pub skill: crate::Skill,
    pub branch: String,
    pub unlock_level: u32,
    pub description: String,
    pub xp_bonus_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentPack {
    pub items: Vec<ItemDef>,
    pub npcs: Vec<NpcDef>,
    pub objects: Vec<ObjectDef>,
    pub recipes: Vec<RefinementRecipe>,
    pub spells: Vec<SpellDef>,
    pub regions: Vec<RegionDef>,
    pub shops: Vec<ShopDef>,
    pub dialogues: Vec<DialogueNode>,
    pub quests: Vec<QuestDef>,
    pub skills: Vec<SkillDef>,
    pub specializations: Vec<SpecializationDef>,
}

impl ContentPack {
    pub fn load_dir(path: &std::path::Path) -> anyhow::Result<Self> {
        let mut pack = ContentPack::default();
        if path.join("pack.yaml").exists() {
            let data = std::fs::read_to_string(path.join("pack.yaml"))?;
            pack = serde_yaml::from_str(&data)?;
        } else {
            pack.items = load_yaml_dir(&path.join("items"))?;
            pack.npcs = load_yaml_dir(&path.join("npcs"))?;
            pack.objects = load_yaml_dir(&path.join("objects"))?;
            pack.recipes = load_yaml_dir(&path.join("recipes"))?;
            pack.spells = load_yaml_dir(&path.join("spells"))?;
            pack.regions = load_yaml_dir(&path.join("regions"))?;
            pack.shops = load_yaml_dir(&path.join("shops"))?;
            pack.dialogues = load_yaml_dir(&path.join("dialogues"))?;
            pack.quests = load_yaml_dir(&path.join("quests"))?;
            pack.skills = load_yaml_dir(&path.join("skills"))?;
            pack.specializations = load_yaml_dir(&path.join("specializations"))?;
        }
        Ok(pack)
    }

    pub fn item(&self, id: ItemId) -> Option<&ItemDef> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn npc(&self, id: NpcId) -> Option<&NpcDef> {
        self.npcs.iter().find(|n| n.id == id)
    }

    pub fn object(&self, id: ObjectId) -> Option<&ObjectDef> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn recipe(&self, id: &str) -> Option<&RefinementRecipe> {
        self.recipes.iter().find(|r| r.id == id)
    }

    pub fn spell(&self, id: &str) -> Option<&SpellDef> {
        self.spells.iter().find(|s| s.id == id)
    }

    pub fn region(&self, id: RegionId) -> Option<&RegionDef> {
        self.regions.iter().find(|r| r.id == id)
    }

    pub fn quest(&self, id: QuestId) -> Option<&QuestDef> {
        self.quests.iter().find(|q| q.id == id)
    }
}

fn load_yaml_dir<T: for<'de> Deserialize<'de>>(dir: &std::path::Path) -> anyhow::Result<Vec<T>> {
    let mut items = Vec::new();
    if !dir.exists() {
        return Ok(items);
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            let data = std::fs::read_to_string(&path)?;
            items.push(serde_yaml::from_str(&data)?);
        }
    }
    Ok(items)
}

use anyhow::Context;

pub fn load_content(path: &std::path::Path) -> anyhow::Result<ContentPack> {
    ContentPack::load_dir(path).with_context(|| format!("loading content from {}", path.display()))
}
