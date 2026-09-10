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
    /// Combat level required to equip (0 = none).
    #[serde(default)]
    pub equip_level: u32,
    /// Flavour text shown on "Examine".
    #[serde(default)]
    pub examine: String,
    /// Icon tint / material family used by the procedural item icons.
    #[serde(default)]
    pub tier: Option<MaterialTier>,
    /// HP restored when eaten (0 = not food).
    #[serde(default)]
    pub heals: u32,
}

/// Material families; drive item icon colours and gear progression order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTier {
    Scrap,
    Bronze,
    Iron,
    Steel,
    Mithral,
    Wood,
    Stone,
    Cloth,
    Leather,
    Food,
    Misc,
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
    /// Flavour text shown on "Examine".
    #[serde(default)]
    pub examine: String,
    /// Colour family for the procedural placeholder body when no GLB model
    /// is assigned (e.g. "bandit", "beast", "wisp", "brute").
    #[serde(default)]
    pub archetype: Option<String>,
}

impl NpcDef {
    /// Rough danger rating shown to players, RuneScape-style.
    pub fn combat_level(&self) -> u32 {
        (self.max_hp / 4 + self.prowess + self.fortitude).max(1)
    }
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
    /// How the client draws this object. Defaults are derived from
    /// `harvest_tag` when unset.
    #[serde(default)]
    pub model: Option<PropModel>,
    /// Interactive station (bank booth, furnace, ...). Stations are not
    /// harvested; interacting opens the matching interface.
    #[serde(default)]
    pub station: Option<StationTag>,
    /// Flavour text shown on "Examine".
    #[serde(default)]
    pub examine: String,
    /// Ticks until a depleted node respawns (0 = engine default).
    #[serde(default)]
    pub respawn_ticks: u32,
}

impl ObjectDef {
    /// Model to draw, falling back on the harvest tag.
    pub fn model_or_default(&self) -> PropModel {
        if let Some(m) = self.model {
            return m;
        }
        match (self.station, self.harvest_tag) {
            (Some(StationTag::Bank), _) => PropModel::BankBooth,
            (Some(StationTag::Furnace), _) => PropModel::Furnace,
            (Some(StationTag::Anvil), _) => PropModel::Anvil,
            (Some(StationTag::Market), _) => PropModel::MarketStall,
            (Some(StationTag::Cooking), _) => PropModel::Campfire,
            (Some(StationTag::Workbench), _) => PropModel::Workbench,
            (None, Some(crate::HarvestTag::Timber)) => PropModel::Tree,
            (None, Some(crate::HarvestTag::Ore)) => PropModel::OreVein,
            (None, Some(crate::HarvestTag::Water)) => PropModel::Pool,
            (None, Some(crate::HarvestTag::Flora)) => PropModel::Bush,
            (None, None) => PropModel::Crate,
        }
    }
}

/// Procedural prop shapes the client knows how to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropModel {
    Tree,
    Oak,
    DeadTree,
    Pine,
    Bush,
    Stump,
    Rock,
    Boulder,
    OreVein,
    Crystal,
    Pool,
    Anvil,
    Furnace,
    Workbench,
    BankBooth,
    MarketStall,
    Campfire,
    Crate,
    Barrel,
    Sign,
    Fence,
    Lamp,
    Well,
    Tent,
    Rubble,
    Mushroom,
    Cactus,
    Statue,
}

/// Interactive station types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationTag {
    Bank,
    Furnace,
    Anvil,
    Workbench,
    Cooking,
    Market,
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
    /// Station the player must be next to (None = anywhere).
    #[serde(default)]
    pub station: Option<StationTag>,
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
    #[serde(default)]
    pub kind: crate::TransitionKind,
    #[serde(default)]
    pub label: String,
}

/// Broad setting of a region; drives sky/ambient colours and the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RegionAmbience {
    #[default]
    Overworld,
    Cave,
    Dungeon,
    Mountain,
    Coast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionDef {
    pub id: RegionId,
    pub name: String,
    /// Derived from `layout` when one is given.
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// Derived from the `spawn: true` legend cell when a layout is given.
    #[serde(default)]
    pub spawn: TilePos,
    /// Byte per tile, row-major, see [`crate::TileKind`]. Derived from
    /// `layout` when one is given.
    #[serde(default)]
    pub tiles: Vec<u8>,
    #[serde(default)]
    pub objects: Vec<RegionObject>,
    #[serde(default)]
    pub npcs: Vec<RegionNpc>,
    #[serde(default)]
    pub transitions: Vec<RegionTransition>,
    /// Text-art rows; see [`crate::tiles`]. Expanded on load.
    #[serde(default)]
    pub layout: Vec<String>,
    #[serde(default)]
    pub legend: std::collections::BTreeMap<char, crate::LegendEntry>,
    #[serde(default)]
    pub ambience: RegionAmbience,
    /// Suggested combat level, shown on portals ("Wisp Warren (lvl 15+)").
    #[serde(default)]
    pub recommended_level: u32,
}

impl RegionDef {
    /// Expand `layout`/`legend` into `tiles`, `objects`, `npcs`, `spawn` and
    /// `transitions`. No-op for regions authored with explicit `tiles`.
    /// Entities listed explicitly are kept and the layout's are appended.
    pub fn expand_layout(&mut self) -> Result<(), crate::LayoutError> {
        if self.layout.is_empty() {
            return Ok(());
        }
        let expanded = crate::expand_layout(&self.name, &self.layout, &self.legend)?;
        if (self.width != 0 && self.width != expanded.width)
            || (self.height != 0 && self.height != expanded.height)
        {
            return Err(crate::LayoutError::SizeMismatch {
                region: self.name.clone(),
                w: self.width,
                h: self.height,
                lw: expanded.width,
                lh: expanded.height,
            });
        }
        self.width = expanded.width;
        self.height = expanded.height;
        self.tiles = expanded.tiles;
        if let Some(spawn) = expanded.spawn {
            self.spawn = spawn;
        }
        self.objects
            .extend(
                expanded
                    .objects
                    .into_iter()
                    .map(|(object_id, position)| RegionObject {
                        object_id,
                        position,
                    }),
            );
        self.npcs.extend(
            expanded
                .npcs
                .into_iter()
                .map(|(npc_id, position)| RegionNpc { npc_id, position }),
        );
        self.transitions
            .extend(
                expanded
                    .transitions
                    .into_iter()
                    .map(|(position, portal)| RegionTransition {
                        position,
                        target_region: portal.region,
                        target_spawn: TilePos::new(portal.spawn[0], portal.spawn[1]),
                        kind: portal.kind,
                        label: portal.label,
                    }),
            );
        // The expanded form is canonical; drop the source so a re-expansion
        // (e.g. hot reload of an already-loaded pack) can't double-append.
        self.layout.clear();
        self.legend.clear();
        Ok(())
    }

    pub fn tile_at(&self, pos: TilePos) -> Option<crate::TileKind> {
        if pos.x < 0 || pos.y < 0 || pos.x as u32 >= self.width || pos.y as u32 >= self.height {
            return None;
        }
        let idx = (pos.y as u32 * self.width + pos.x as u32) as usize;
        self.tiles
            .get(idx)
            .copied()
            .and_then(crate::TileKind::from_byte)
    }

    pub fn is_walkable(&self, pos: TilePos) -> bool {
        self.tile_at(pos).is_some_and(crate::TileKind::walkable)
    }

    pub fn transition_at(&self, pos: TilePos) -> Option<&RegionTransition> {
        self.transitions
            .iter()
            .find(|t| t.position.x == pos.x && t.position.y == pos.y)
    }
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
    /// Items every new character starts with (`starter_kit.yaml`).
    #[serde(default)]
    pub starter_kit: Vec<RecipeInput>,
}

impl ContentPack {
    pub fn load_dir(path: &std::path::Path) -> anyhow::Result<Self> {
        let mut pack = ContentPack::default();
        if path.join("pack.yaml").exists() {
            let data = std::fs::read_to_string(path.join("pack.yaml"))?;
            pack = serde_yaml::from_str(&data)?;
        } else {
            let kit = path.join("starter_kit.yaml");
            if kit.exists() {
                pack.starter_kit = serde_yaml::from_str(&std::fs::read_to_string(&kit)?)?;
            }
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
        for region in &mut pack.regions {
            region.expand_layout()?;
        }
        // Directory order is by filename, which is meaningless for gameplay.
        // `regions.first()` is treated as the starting region by the server
        // (new-player spawn) and the client (initial map), so order by id.
        pack.regions.sort_by_key(|r| r.id.0);
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

// Defaults exist so tests and tools can build fixtures with
// `..Default::default()` instead of naming every field.

impl Default for ItemDef {
    fn default() -> Self {
        Self {
            id: ItemId(0),
            name: String::new(),
            stackable: false,
            equip_slot: None,
            prowess_bonus: 0,
            electrics_bonus: 0,
            fortitude_bonus: 0,
            tool_tag: None,
            alchemy_value: 0,
            attack_ticks: default_attack_ticks(),
            equip_level: 0,
            examine: String::new(),
            tier: None,
            heals: 0,
        }
    }
}

impl Default for NpcDef {
    fn default() -> Self {
        Self {
            id: NpcId(0),
            name: String::new(),
            model: None,
            max_hp: 1,
            prowess: 0,
            fortitude: 0,
            aggro_range: 0,
            respawn_ticks: 10,
            attack_ticks: default_attack_ticks(),
            footprint_w: 1,
            footprint_h: 1,
            loot_table: Vec::new(),
            examine: String::new(),
            archetype: None,
        }
    }
}

impl Default for ObjectDef {
    fn default() -> Self {
        Self {
            id: ObjectId(0),
            name: String::new(),
            harvest_tag: None,
            scavenging_level: 1,
            scavenging_xp: 0,
            harvest_item: None,
            scavenging_ticks: 1,
            depletes: false,
            model: None,
            station: None,
            examine: String::new(),
            respawn_ticks: 0,
        }
    }
}

impl Default for RegionTransition {
    fn default() -> Self {
        Self {
            position: TilePos::default(),
            target_region: RegionId(0),
            target_spawn: TilePos::default(),
            kind: crate::TransitionKind::default(),
            label: String::new(),
        }
    }
}

impl Default for RegionDef {
    fn default() -> Self {
        Self {
            id: RegionId(0),
            name: String::new(),
            width: 0,
            height: 0,
            spawn: TilePos::default(),
            tiles: Vec::new(),
            objects: Vec::new(),
            npcs: Vec::new(),
            transitions: Vec::new(),
            layout: Vec::new(),
            legend: Default::default(),
            ambience: RegionAmbience::default(),
            recommended_level: 0,
        }
    }
}

/// A YAML file in a content directory holds either one definition or a list
/// of them, so related definitions (all the bronze gear, say) can share a file.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
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
            let parsed: OneOrMany<T> = serde_yaml::from_str(&data)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            match parsed {
                OneOrMany::Many(v) => items.extend(v),
                OneOrMany::One(v) => items.push(v),
            }
        }
    }
    Ok(items)
}

use anyhow::Context;

pub fn load_content(path: &std::path::Path) -> anyhow::Result<ContentPack> {
    ContentPack::load_dir(path).with_context(|| format!("loading content from {}", path.display()))
}
