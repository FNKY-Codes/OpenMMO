use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    Vitality,
    Prowess,
    Fortitude,
    Harvest,
    Refinement,
    Arcana,
    Covenant,
    Pathfinding,
    Alchemy,
    Skulking,
    Ledger,
    Beastmastery,
    Husbandry,
    Hearthcraft,
}

impl Skill {
    pub const MVP: &[Skill] = &[
        Skill::Vitality,
        Skill::Prowess,
        Skill::Fortitude,
        Skill::Harvest,
        Skill::Refinement,
        Skill::Arcana,
    ];

    pub const BETA: &[Skill] = &[
        Skill::Covenant,
        Skill::Pathfinding,
        Skill::Alchemy,
        Skill::Skulking,
        Skill::Ledger,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Skill::Vitality => "Vitality",
            Skill::Prowess => "Prowess",
            Skill::Fortitude => "Fortitude",
            Skill::Harvest => "Harvest",
            Skill::Refinement => "Refinement",
            Skill::Arcana => "Arcana",
            Skill::Covenant => "Covenant",
            Skill::Pathfinding => "Pathfinding",
            Skill::Alchemy => "Alchemy",
            Skill::Skulking => "Skulking",
            Skill::Ledger => "Ledger",
            Skill::Beastmastery => "Beastmastery",
            Skill::Husbandry => "Husbandry",
            Skill::Hearthcraft => "Hearthcraft",
        }
    }

    pub fn all() -> &'static [Skill] {
        &[
            Skill::Vitality,
            Skill::Prowess,
            Skill::Fortitude,
            Skill::Harvest,
            Skill::Refinement,
            Skill::Arcana,
            Skill::Covenant,
            Skill::Pathfinding,
            Skill::Alchemy,
            Skill::Skulking,
            Skill::Ledger,
            Skill::Beastmastery,
            Skill::Husbandry,
            Skill::Hearthcraft,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Calling {
    War,
    Wild,
    Craft,
    Arcane,
}

impl Calling {
    pub fn skills(&self) -> &'static [Skill] {
        match self {
            Calling::War => &[Skill::Vitality, Skill::Prowess, Skill::Fortitude, Skill::Covenant],
            Calling::Wild => &[
                Skill::Harvest,
                Skill::Pathfinding,
                Skill::Beastmastery,
                Skill::Husbandry,
            ],
            Calling::Craft => &[Skill::Refinement, Skill::Alchemy, Skill::Hearthcraft],
            Calling::Arcane => &[Skill::Arcana, Skill::Skulking, Skill::Ledger],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillProgress {
    pub level: u32,
    pub xp: u64,
}

impl Default for SkillProgress {
    fn default() -> Self {
        Self { level: 1, xp: 0 }
    }
}

impl SkillProgress {
    pub fn xp_for_level(level: u32) -> u64 {
        if level <= 1 {
            return 0;
        }
        let l = level as f64;
        ((l - 1.0).powi(2) * 50.0 + (l - 1.0) * 100.0) as u64
    }

    pub fn add_xp(&mut self, amount: u64) -> Vec<u32> {
        self.xp = self.xp.saturating_add(amount);
        let mut levels = Vec::new();
        while self.level < 99 && self.xp >= Self::xp_for_level(self.level + 1) {
            self.level += 1;
            levels.push(self.level);
        }
        levels
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillBook {
    pub skills: std::collections::HashMap<Skill, SkillProgress>,
}

impl SkillBook {
    pub fn new_mvp() -> Self {
        let mut skills = std::collections::HashMap::new();
        for skill in Skill::MVP {
            skills.insert(*skill, SkillProgress::default());
        }
        Self { skills }
    }

    pub fn level(&self, skill: Skill) -> u32 {
        self.skills.get(&skill).map(|s| s.level).unwrap_or(1)
    }

    pub fn grant_xp(&mut self, skill: Skill, amount: u64) -> Vec<u32> {
        self.skills
            .entry(skill)
            .or_default()
            .add_xp(amount)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarvestTag {
    Timber,
    Ore,
    Water,
    Flora,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTag {
    Axe,
    Pick,
    Rod,
    Knife,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatStyle {
    Melee,
    Ranged,
    Magic,
}
