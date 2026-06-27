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
