use openmmo_common::{BossState, CombatStyle, ContentPack, NpcState, PlayerState, Skill};
use rand::Rng;

#[derive(Debug, Clone)]
pub struct XpGrant {
    pub skill: Skill,
    pub amount: u64,
    pub levels_gained: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct CombatHit {
    pub damage: u32,
    pub xp_grants: Vec<XpGrant>,
}

fn grant_xp_grant(player: &mut PlayerState, skill: Skill, amount: u64) -> XpGrant {
    let levels_gained = player.skills.grant_xp(skill, amount);
    XpGrant {
        skill,
        amount,
        levels_gained,
    }
}

pub fn player_max_hit(player: &PlayerState, style: CombatStyle, content: &ContentPack) -> u32 {
    let combat = player.skills.level(Skill::Combat);
    let prowess_bonus = equipment_prowess_bonus(player, content);
    let electrics_bonus = equipment_electrics_bonus(player, content);
    let base = match style {
        CombatStyle::Melee => combat + 1 + prowess_bonus,
        CombatStyle::Ranged => combat + prowess_bonus,
        CombatStyle::Magic => player.skills.level(Skill::Electrics) + electrics_bonus,
    };
    base.max(1)
}

pub fn player_attack_stat(player: &PlayerState, style: CombatStyle, content: &ContentPack) -> u32 {
    match style {
        CombatStyle::Melee | CombatStyle::Ranged => {
            player.skills.level(Skill::Combat) + equipment_prowess_bonus(player, content)
        }
        CombatStyle::Magic => {
            player.skills.level(Skill::Electrics) + equipment_electrics_bonus(player, content)
        }
    }
}

pub fn equipment_prowess_bonus(player: &PlayerState, content: &ContentPack) -> u32 {
    equipment_bonus_from_slots(player, content, |item| item.prowess_bonus)
}

pub fn equipment_electrics_bonus(player: &PlayerState, content: &ContentPack) -> u32 {
    equipment_bonus_from_slots(player, content, |item| item.electrics_bonus)
}

pub fn equipment_fortitude_bonus(player: &PlayerState, content: &ContentPack) -> u32 {
    equipment_bonus_from_slots(player, content, |item| item.fortitude_bonus)
}

fn equipment_bonus_from_slots(
    player: &PlayerState,
    content: &ContentPack,
    bonus: impl Fn(&openmmo_common::ItemDef) -> i32,
) -> u32 {
    let mut total = 0i32;
    for slot in [
        player.equipment.head.as_ref(),
        player.equipment.body.as_ref(),
        player.equipment.legs.as_ref(),
        player.equipment.weapon.as_ref(),
        player.equipment.shield.as_ref(),
    ] {
        if let Some(s) = slot {
            total += content.item(s.item_id).map(&bonus).unwrap_or(0);
        }
    }
    total.max(0) as u32
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
        npc.aggro_target = None;
    }
}

pub fn apply_damage_player(player: &mut PlayerState, amount: u32) {
    player.hp = player.hp.saturating_sub(amount);
}

pub const DEFAULT_ATTACK_TICKS: u32 = 4;

pub fn player_attack_ticks(player: &PlayerState, content: &ContentPack) -> u32 {
    if let Some(weapon) = &player.equipment.weapon {
        content
            .item(weapon.item_id)
            .map(|item| item.attack_ticks)
            .unwrap_or(DEFAULT_ATTACK_TICKS)
    } else {
        DEFAULT_ATTACK_TICKS
    }
}

pub fn npc_attack_ticks(npc_id: openmmo_common::NpcId, content: &ContentPack) -> u32 {
    content
        .npc(npc_id)
        .map(|npc| npc.attack_ticks)
        .unwrap_or(DEFAULT_ATTACK_TICKS)
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

pub fn npc_attack_player(
    npc: &NpcState,
    player: &mut PlayerState,
    content: &ContentPack,
) -> Option<CombatHit> {
    if accuracy_roll(npc.prowess, combat_fortitude(player, content)) {
        let dmg = roll_damage(npc.prowess / 2 + 1);
        apply_damage_player(player, dmg);
        let xp = grant_xp_grant(player, Skill::Resilience, dmg as u64 * 2);
        Some(CombatHit {
            damage: dmg,
            xp_grants: vec![xp],
        })
    } else {
        None
    }
}

pub fn boss_attack_player(
    boss: &BossState,
    player: &mut PlayerState,
    content: &ContentPack,
) -> Option<CombatHit> {
    if accuracy_roll(boss.prowess, combat_fortitude(player, content)) {
        let dmg = roll_damage(boss.prowess / 2 + 1);
        apply_damage_player(player, dmg);
        let xp = grant_xp_grant(player, Skill::Resilience, dmg as u64 * 2);
        Some(CombatHit {
            damage: dmg,
            xp_grants: vec![xp],
        })
    } else {
        None
    }
}

pub fn player_attack_npc(
    player: &mut PlayerState,
    npc: &mut NpcState,
    style: CombatStyle,
    content: &ContentPack,
) -> Option<CombatHit> {
    let attack_stat = player_attack_stat(player, style, content);
    let max_hit = player_max_hit(player, style, content);
    if accuracy_roll(attack_stat, npc.fortitude) {
        let dmg = roll_damage(max_hit);
        apply_damage_npc(npc, dmg);
        let (skill, xp_amount) = xp_for_damage(dmg, style);
        let primary = grant_xp_grant(player, skill, xp_amount);
        let endurance = grant_xp_grant(player, Skill::Endurance, dmg as u64);
        Some(CombatHit {
            damage: dmg,
            xp_grants: vec![primary, endurance],
        })
    } else {
        None
    }
}

pub fn player_attack_boss(
    player: &mut PlayerState,
    boss: &mut BossState,
    style: CombatStyle,
    content: &ContentPack,
) -> Option<CombatHit> {
    let attack_stat = player_attack_stat(player, style, content);
    let max_hit = player_max_hit(player, style, content);
    if accuracy_roll(attack_stat, boss.fortitude) {
        let dmg = roll_damage(max_hit);
        boss.hp = boss.hp.saturating_sub(dmg);
        let (skill, xp_amount) = xp_for_damage(dmg, style);
        let primary = grant_xp_grant(player, skill, xp_amount);
        let endurance = grant_xp_grant(player, Skill::Endurance, dmg as u64);
        Some(CombatHit {
            damage: dmg,
            xp_grants: vec![primary, endurance],
        })
    } else {
        None
    }
}

pub fn use_gadget(
    player: &mut PlayerState,
    npc: &mut NpcState,
    max_hit: u32,
    xp: u64,
    content: &ContentPack,
) -> Option<CombatHit> {
    let attack_stat = player_attack_stat(player, CombatStyle::Magic, content);
    if accuracy_roll(attack_stat, npc.fortitude) {
        let dmg = roll_damage(max_hit);
        apply_damage_npc(npc, dmg);
        let electrics = grant_xp_grant(player, Skill::Electrics, xp);
        Some(CombatHit {
            damage: dmg,
            xp_grants: vec![electrics],
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{EntityId, Equipment, InventorySlot, ItemDef, ItemId, NpcId, SkillBook};

    fn test_content(items: Vec<ItemDef>) -> ContentPack {
        ContentPack {
            items,
            ..Default::default()
        }
    }

    fn test_player_with_gear(item: ItemDef, slot: openmmo_common::EquipSlot) -> PlayerState {
        let mut player = PlayerState {
            id: openmmo_common::PlayerId(uuid::Uuid::from_u128(1)),
            name: "test".into(),
            entity_id: EntityId(1),
            position: openmmo_common::TilePos::new(0, 0),
            hp: 10,
            max_hp: 10,
            skills: SkillBook::new_mvp(),
            inventory: openmmo_common::Inventory::new(28),
            bank: openmmo_common::Inventory::new(200),
            equipment: Equipment::default(),
            combat_target: None,
            action: Default::default(),
            quest_progress: Default::default(),
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: openmmo_common::TilePos::new(0, 0),
            ticks_stationary: 1,
        };
        player.skills.grant_xp(Skill::Combat, 400);
        player.skills.grant_xp(Skill::Electrics, 400);
        let slot_item = InventorySlot {
            item_id: item.id,
            quantity: 1,
        };
        match slot {
            openmmo_common::EquipSlot::Weapon => player.equipment.weapon = Some(slot_item),
            openmmo_common::EquipSlot::Head => player.equipment.head = Some(slot_item),
            openmmo_common::EquipSlot::Body => player.equipment.body = Some(slot_item),
            openmmo_common::EquipSlot::Legs => player.equipment.legs = Some(slot_item),
            openmmo_common::EquipSlot::Shield => player.equipment.shield = Some(slot_item),
        }
        player
    }

    #[test]
    fn player_attack_stat_includes_prowess_bonus() {
        let item = ItemDef {
            id: ItemId(1),
            name: "Blade".into(),
            stackable: false,
            equip_slot: Some(openmmo_common::EquipSlot::Weapon),
            prowess_bonus: 5,
            electrics_bonus: 0,
            fortitude_bonus: 0,
            tool_tag: None,
            alchemy_value: 0,
            attack_ticks: 4,
        };
        let content = test_content(vec![item.clone()]);
        let player = test_player_with_gear(item, openmmo_common::EquipSlot::Weapon);
        let combat_level = player.skills.level(Skill::Combat);
        assert_eq!(
            player_attack_stat(&player, CombatStyle::Melee, &content),
            combat_level + 5
        );
    }

    #[test]
    fn player_attack_stat_uses_electrics_for_magic() {
        let item = ItemDef {
            id: ItemId(2),
            name: "Coil".into(),
            stackable: false,
            equip_slot: Some(openmmo_common::EquipSlot::Weapon),
            prowess_bonus: 0,
            electrics_bonus: 4,
            fortitude_bonus: 0,
            tool_tag: None,
            alchemy_value: 0,
            attack_ticks: 4,
        };
        let content = test_content(vec![item.clone()]);
        let player = test_player_with_gear(item, openmmo_common::EquipSlot::Weapon);
        let electrics_level = player.skills.level(Skill::Electrics);
        assert_eq!(
            player_attack_stat(&player, CombatStyle::Magic, &content),
            electrics_level + 4
        );
    }

    #[test]
    fn use_gadget_can_miss() {
        let content = test_content(vec![]);
        let mut player = PlayerState {
            id: openmmo_common::PlayerId(uuid::Uuid::from_u128(1)),
            name: "test".into(),
            entity_id: EntityId(1),
            position: openmmo_common::TilePos::new(0, 0),
            hp: 10,
            max_hp: 10,
            skills: SkillBook::new_mvp(),
            inventory: openmmo_common::Inventory::new(28),
            bank: openmmo_common::Inventory::new(200),
            equipment: Equipment::default(),
            combat_target: None,
            action: Default::default(),
            quest_progress: Default::default(),
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: openmmo_common::TilePos::new(0, 0),
            ticks_stationary: 1,
        };
        let mut npc = NpcState {
            entity_id: EntityId(2),
            npc_id: NpcId(1),
            name: "target".into(),
            position: openmmo_common::TilePos::new(1, 0),
            home_position: openmmo_common::TilePos::new(1, 0),
            hp: 20,
            max_hp: 20,
            prowess: 1,
            fortitude: 1000,
            aggro_target: None,
            alive: true,
            respawn_ticks: 10,
            attack_cooldown: 0,
        };

        let mut misses = 0;
        for _ in 0..20 {
            npc.hp = 20;
            if use_gadget(&mut player, &mut npc, 10, 12, &content).is_none() {
                misses += 1;
            }
        }
        assert!(misses > 0);
    }
}
