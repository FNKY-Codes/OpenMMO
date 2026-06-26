use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    Endurance,
    Combat,
    Resilience,
    Scavenging,
    Fabrication,
    Electrics,
    Brotherhood,
    Scouting,
    Chemistry,
    Stealth,
    Barter,
    Wrangling,
    Ranching,
    Engineering,
}

impl Skill {
    pub const MVP: &[Skill] = &[
        Skill::Endurance,
        Skill::Combat,
        Skill::Resilience,
        Skill::Scavenging,
        Skill::Fabrication,
        Skill::Electrics,
    ];

    pub const BETA: &[Skill] = &[
        Skill::Brotherhood,
        Skill::Scouting,
        Skill::Chemistry,
        Skill::Stealth,
        Skill::Barter,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Skill::Endurance => "Endurance",
            Skill::Combat => "Combat",
            Skill::Resilience => "Resilience",
            Skill::Scavenging => "Scavenging",
            Skill::Fabrication => "Fabrication",
            Skill::Electrics => "Electrics",
            Skill::Brotherhood => "Brotherhood",
            Skill::Scouting => "Scouting",
            Skill::Chemistry => "Chemistry",
            Skill::Stealth => "Stealth",
            Skill::Barter => "Barter",
            Skill::Wrangling => "Wrangling",
            Skill::Ranching => "Ranching",
            Skill::Engineering => "Engineering",
        }
    }

    pub fn all() -> &'static [Skill] {
        &[
            Skill::Endurance,
            Skill::Combat,
            Skill::Resilience,
            Skill::Scavenging,
            Skill::Fabrication,
            Skill::Electrics,
            Skill::Brotherhood,
            Skill::Scouting,
            Skill::Chemistry,
            Skill::Stealth,
            Skill::Barter,
            Skill::Wrangling,
            Skill::Ranching,
            Skill::Engineering,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Calling {
    Survival,
    Outlands,
    Workshop,
    Operator,
}

impl Calling {
    pub fn name(&self) -> &'static str {
        match self {
            Calling::Survival => "Survival",
            Calling::Outlands => "Outlands",
            Calling::Workshop => "Workshop",
            Calling::Operator => "Operator",
        }
    }

    pub fn skills(&self) -> &'static [Skill] {
        match self {
            Calling::Survival => &[
                Skill::Endurance,
                Skill::Combat,
                Skill::Resilience,
                Skill::Brotherhood,
            ],
            Calling::Outlands => &[
                Skill::Scavenging,
                Skill::Scouting,
                Skill::Wrangling,
                Skill::Ranching,
            ],
            Calling::Workshop => &[Skill::Fabrication, Skill::Chemistry, Skill::Engineering],
            Calling::Operator => &[Skill::Electrics, Skill::Stealth, Skill::Barter],
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
        self.skills.entry(skill).or_default().add_xp(amount)
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
