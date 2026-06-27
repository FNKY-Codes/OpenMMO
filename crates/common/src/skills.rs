pub use openmmo_sdk::skills::{Calling, HarvestTag, Skill, ToolTag};

use serde::{Deserialize, Serialize};

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
pub enum CombatStyle {
    Melee,
    Ranged,
    Magic,
}
