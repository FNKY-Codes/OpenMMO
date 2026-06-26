use openmmo_common::{CombatStyle, ContentPack, NpcState, PlayerState, Skill};
use rand::Rng;

pub fn player_max_hit(player: &PlayerState, style: CombatStyle, content: &ContentPack) -> u32 {
    let combat = player.skills.level(Skill::Combat);
    let equipment_bonus = equipment_prowess_bonus(player, content);
    let base = match style {
        CombatStyle::Melee => combat + 1 + equipment_bonus,
        CombatStyle::Ranged => combat + equipment_bonus,
        CombatStyle::Magic => player.skills.level(Skill::Electrics) + equipment_bonus,
    };
    base.max(1)
}

pub fn equipment_prowess_bonus(player: &PlayerState, content: &ContentPack) -> u32 {
    let mut bonus = 0i32;
    for slot in [
        player.equipment.head.as_ref(),
        player.equipment.body.as_ref(),
        player.equipment.legs.as_ref(),
        player.equipment.weapon.as_ref(),
        player.equipment.shield.as_ref(),
    ] {
        if let Some(s) = slot {
            bonus += content.item(s.item_id).map(|i| i.prowess_bonus).unwrap_or(0);
        }
    }
    bonus.max(0) as u32
}

pub fn equipment_fortitude_bonus(player: &PlayerState, content: &ContentPack) -> u32 {
    let mut bonus = 0i32;
    for slot in [
        player.equipment.head.as_ref(),
        player.equipment.body.as_ref(),
        player.equipment.legs.as_ref(),
        player.equipment.weapon.as_ref(),
        player.equipment.shield.as_ref(),
    ] {
        if let Some(s) = slot {
            bonus += content.item(s.item_id).map(|i| i.fortitude_bonus).unwrap_or(0);
        }
    }
    bonus.max(0) as u32
}

pub fn accuracy_roll(attacker_prowess: u32, defender_fortitude: u32) -> bool {
    let mut rng = rand::thread_rng();
    let chance = (attacker_prowess as f32 + 10.0) / (defender_fortitude as f32 + 20.0);
    rng.gen::<f32>() < chance.clamp(0.1, 0.95)
}

pub fn roll_damage(max_hit: u32) -> u32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..=max_hit).max(1)
}

pub fn apply_damage_npc(npc: &mut NpcState, amount: u32) {
    npc.hp = npc.hp.saturating_sub(amount);
    if npc.hp == 0 {
        npc.alive = false;
        npc.respawn_ticks = npc.respawn_ticks.max(10);
    }
}

pub fn apply_damage_player(player: &mut PlayerState, amount: u32) {
    player.hp = player.hp.saturating_sub(amount);
}

pub fn xp_for_damage(amount: u32, style: CombatStyle) -> (Skill, u64) {
    let xp = amount as u64 * 4;
    match style {
        CombatStyle::Melee | CombatStyle::Ranged => (Skill::Combat, xp),
        CombatStyle::Magic => (Skill::Electrics, xp),
    }
}

pub fn combat_fortitude(player: &PlayerState, content: &ContentPack) -> u32 {
    player.skills.level(Skill::Resilience) + equipment_fortitude_bonus(player, content)
}

pub fn npc_attack_player(npc: &NpcState, player: &mut PlayerState, content: &ContentPack) -> Option<u32> {
    if accuracy_roll(npc.prowess, combat_fortitude(player, content)) {
        let dmg = roll_damage(npc.prowess / 2 + 1);
        apply_damage_player(player, dmg);
        player.skills.grant_xp(Skill::Resilience, dmg as u64 * 2);
        Some(dmg)
    } else {
        None
    }
}

pub fn player_attack_npc(
    player: &mut PlayerState,
    npc: &mut NpcState,
    style: CombatStyle,
    content: &ContentPack,
) -> Option<u32> {
    let max_hit = player_max_hit(player, style, content);
    if accuracy_roll(player.skills.level(Skill::Combat), npc.fortitude) {
        let dmg = roll_damage(max_hit);
        apply_damage_npc(npc, dmg);
        let (skill, xp) = xp_for_damage(dmg, style);
        player.skills.grant_xp(skill, xp);
        player.skills.grant_xp(Skill::Endurance, dmg as u64);
        Some(dmg)
    } else {
        None
    }
}

pub fn cast_spell(
    player: &mut PlayerState,
    npc: &mut NpcState,
    max_hit: u32,
    xp: u64,
) -> Option<u32> {
    let dmg = roll_damage(max_hit);
    apply_damage_npc(npc, dmg);
    player.skills.grant_xp(Skill::Electrics, xp);
    Some(dmg)
}
