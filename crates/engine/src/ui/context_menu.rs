use egui::{Context, Pos2, RichText};
use openmmo_common::{
    ContentPack, EntityId, EntityKind, PlayerId, RegionDef, TilePos, WorldEntity,
};

use super::theme::TEXT_MUTED;
use super::widgets::entity_display_name;

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub screen_pos: Pos2,
    pub title: String,
    pub subtitle: Option<String>,
    pub actions: Vec<ContextMenuEntry>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    pub action: ContextMenuAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuAction {
    WalkTo(TilePos),
    Attack(EntityId),
    TalkTo(EntityId),
    Scavenge(EntityId),
    Pickup(EntityId),
    Trade(PlayerId),
}

pub fn build_context_menu(
    content: &ContentPack,
    region: Option<&RegionDef>,
    entities: &[WorldEntity],
    local_player: Option<PlayerId>,
    entity_id: Option<EntityId>,
    tile: TilePos,
    screen_pos: Pos2,
) -> Option<ContextMenu> {
    if let Some(entity_id) = entity_id {
        let entity = entities.iter().find(|e| e.entity_id == entity_id)?;
        if is_local_player(entity, local_player) {
            return None;
        }
        return Some(build_entity_menu(content, entity, screen_pos));
    }

    if !is_tile_walkable(region, tile) {
        return None;
    }

    Some(ContextMenu {
        screen_pos,
        title: "Ground".into(),
        subtitle: Some(format!("({}, {})", tile.x, tile.y)),
        actions: vec![ContextMenuEntry {
            label: "Move here".into(),
            action: ContextMenuAction::WalkTo(tile),
        }],
    })
}

fn build_entity_menu(content: &ContentPack, entity: &WorldEntity, screen_pos: Pos2) -> ContextMenu {
    let (title, subtitle) = entity_display_name(content, entity);
    let position = entity_position(entity);
    let mut actions = Vec::new();

    match &entity.kind {
        EntityKind::GroundItem { .. } => {
            actions.push(ContextMenuEntry {
                label: "Pick up".into(),
                action: ContextMenuAction::Pickup(entity.entity_id),
            });
        }
        EntityKind::Object { .. } => {
            actions.push(ContextMenuEntry {
                label: "Scavenge".into(),
                action: ContextMenuAction::Scavenge(entity.entity_id),
            });
        }
        EntityKind::Npc { aggro_range, .. } => {
            if *aggro_range == 0 {
                actions.push(ContextMenuEntry {
                    label: "Talk to".into(),
                    action: ContextMenuAction::TalkTo(entity.entity_id),
                });
            } else {
                actions.push(ContextMenuEntry {
                    label: "Attack".into(),
                    action: ContextMenuAction::Attack(entity.entity_id),
                });
            }
        }
        EntityKind::Boss { .. } => {
            actions.push(ContextMenuEntry {
                label: "Attack".into(),
                action: ContextMenuAction::Attack(entity.entity_id),
            });
        }
        EntityKind::Player { player_id, .. } => {
            actions.push(ContextMenuEntry {
                label: "Trade".into(),
                action: ContextMenuAction::Trade(*player_id),
            });
        }
    }

    if let Some(position) = position {
        actions.push(ContextMenuEntry {
            label: "Move here".into(),
            action: ContextMenuAction::WalkTo(position),
        });
    }

    ContextMenu {
        screen_pos,
        title,
        subtitle,
        actions,
    }
}

pub fn draw_context_menu(ctx: &Context, menu: &ContextMenu) -> Option<ContextMenuAction> {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return None;
    }

    let mut chosen = None;
    egui::Area::new(egui::Id::new("world_context_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(menu.screen_pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(140.0);
                ui.label(RichText::new(&menu.title).strong());
                if let Some(subtitle) = &menu.subtitle {
                    ui.label(RichText::new(subtitle).color(TEXT_MUTED).small());
                }
                ui.separator();
                for entry in &menu.actions {
                    if ui.button(&entry.label).clicked() {
                        chosen = Some(entry.action.clone());
                    }
                }
            });
        });
    chosen
}

pub fn to_client_message(action: ContextMenuAction) -> openmmo_protocol::ClientMessage {
    use openmmo_common::CombatStyle;
    use openmmo_protocol::ClientMessage;

    match action {
        ContextMenuAction::WalkTo(target) => ClientMessage::WalkIntent { target },
        ContextMenuAction::Attack(target) => ClientMessage::Attack {
            target,
            style: CombatStyle::Melee,
        },
        ContextMenuAction::TalkTo(npc_entity) => ClientMessage::TalkToNpc { npc_entity },
        ContextMenuAction::Scavenge(object_entity) => ClientMessage::Scavenge { object_entity },
        ContextMenuAction::Pickup(ground_entity) => ClientMessage::PickupItem { ground_entity },
        ContextMenuAction::Trade(target_player) => ClientMessage::TradeRequest { target_player },
    }
}

fn is_local_player(entity: &WorldEntity, local_player: Option<PlayerId>) -> bool {
    match (&entity.kind, local_player) {
        (EntityKind::Player { player_id, .. }, Some(local)) => *player_id == local,
        _ => false,
    }
}

fn entity_position(entity: &WorldEntity) -> Option<TilePos> {
    match &entity.kind {
        EntityKind::Player { position, .. }
        | EntityKind::Npc { position, .. }
        | EntityKind::Boss { position, .. }
        | EntityKind::Object { position, .. }
        | EntityKind::GroundItem { position, .. } => Some(*position),
    }
}

pub fn is_tile_walkable(region: Option<&RegionDef>, tile: TilePos) -> bool {
    let Some(region) = region else {
        return true;
    };
    if tile.x < 0 || tile.y < 0 {
        return false;
    }
    let x = tile.x as u32;
    let y = tile.y as u32;
    if x >= region.width || y >= region.height {
        return false;
    }
    let idx = (y * region.width + x) as usize;
    region.tiles.get(idx).copied().unwrap_or(1) != 255
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{ItemId, NpcId, ObjectId};

    fn sample_content() -> ContentPack {
        ContentPack {
            items: vec![openmmo_common::ItemDef {
                id: ItemId(2),
                name: "Raw Timber".into(),
                stackable: true,
                equip_slot: None,
                prowess_bonus: 0,
                electrics_bonus: 0,
                fortitude_bonus: 0,
                tool_tag: None,
                alchemy_value: 0,
                attack_ticks: 4,
            }],
            objects: vec![openmmo_common::ObjectDef {
                id: ObjectId(1),
                name: "Birch Timber".into(),
                harvest_tag: None,
                scavenging_level: 1,
                scavenging_xp: 0,
                harvest_item: Some(ItemId(2)),
                scavenging_ticks: 1,
                depletes: true,
            }],
            ..Default::default()
        }
    }

    fn region() -> RegionDef {
        RegionDef {
            id: openmmo_common::RegionId(1),
            name: "Test".into(),
            width: 3,
            height: 3,
            spawn: TilePos::new(0, 0),
            tiles: vec![0, 0, 0, 0, 255, 0, 0, 0, 0],
            objects: vec![],
            npcs: vec![],
        }
    }

    #[test]
    fn ground_menu_offers_move_on_walkable_tile() {
        let menu = build_context_menu(
            &ContentPack::default(),
            Some(&region()),
            &[],
            None,
            None,
            TilePos::new(0, 0),
            Pos2::ZERO,
        )
        .expect("menu");
        assert_eq!(menu.title, "Ground");
        assert_eq!(menu.actions.len(), 1);
        assert_eq!(menu.actions[0].label, "Move here");
    }

    #[test]
    fn ground_menu_hidden_on_blocked_tile() {
        let menu = build_context_menu(
            &ContentPack::default(),
            Some(&region()),
            &[],
            None,
            None,
            TilePos::new(1, 1),
            Pos2::ZERO,
        );
        assert!(menu.is_none());
    }

    #[test]
    fn npc_menu_has_talk_and_move() {
        let entity = WorldEntity {
            entity_id: EntityId(1),
            kind: EntityKind::Npc {
                npc_id: NpcId(100),
                name: "Guide".into(),
                position: TilePos::new(2, 2),
                hp: 10,
                max_hp: 10,
                aggro_range: 0,
            },
        };
        let menu = build_context_menu(
            &ContentPack::default(),
            None,
            &[entity],
            None,
            Some(EntityId(1)),
            TilePos::new(2, 2),
            Pos2::ZERO,
        )
        .expect("menu");
        assert_eq!(menu.title, "Guide");
        assert_eq!(menu.actions.len(), 2);
        assert_eq!(menu.actions[0].label, "Talk to");
        assert_eq!(menu.actions[1].label, "Move here");
    }

    #[test]
    fn hostile_npc_menu_has_attack() {
        let entity = WorldEntity {
            entity_id: EntityId(2),
            kind: EntityKind::Npc {
                npc_id: NpcId(1),
                name: "Crawler".into(),
                position: TilePos::new(1, 0),
                hp: 10,
                max_hp: 10,
                aggro_range: 3,
            },
        };
        let menu = build_context_menu(
            &ContentPack::default(),
            None,
            &[entity],
            None,
            Some(EntityId(2)),
            TilePos::new(1, 0),
            Pos2::ZERO,
        )
        .expect("menu");
        assert_eq!(menu.actions[0].label, "Attack");
    }

    #[test]
    fn object_menu_uses_content_name_and_scavenge() {
        let entity = WorldEntity {
            entity_id: EntityId(3),
            kind: EntityKind::Object {
                object_id: ObjectId(1),
                position: TilePos::new(0, 1),
            },
        };
        let menu = build_context_menu(
            &sample_content(),
            None,
            &[entity],
            None,
            Some(EntityId(3)),
            TilePos::new(0, 1),
            Pos2::ZERO,
        )
        .expect("menu");
        assert_eq!(menu.title, "Birch Timber");
        assert_eq!(menu.subtitle, Some("Harvestable".into()));
        assert_eq!(menu.actions[0].label, "Scavenge");
    }

    #[test]
    fn ground_item_menu_has_pickup() {
        let entity = WorldEntity {
            entity_id: EntityId(4),
            kind: EntityKind::GroundItem {
                item_id: ItemId(2),
                quantity: 3,
                position: TilePos::new(1, 2),
            },
        };
        let menu = build_context_menu(
            &sample_content(),
            None,
            &[entity],
            None,
            Some(EntityId(4)),
            TilePos::new(1, 2),
            Pos2::ZERO,
        )
        .expect("menu");
        assert_eq!(menu.title, "Raw Timber x3");
        assert_eq!(menu.actions[0].label, "Pick up");
    }

    #[test]
    fn local_player_suppresses_menu() {
        let local = PlayerId(uuid::Uuid::from_u128(1));
        let entity = WorldEntity {
            entity_id: EntityId(5),
            kind: EntityKind::Player {
                player_id: local,
                name: "You".into(),
                position: TilePos::new(0, 0),
                hp: 10,
                max_hp: 10,
            },
        };
        let menu = build_context_menu(
            &ContentPack::default(),
            None,
            &[entity],
            Some(local),
            Some(EntityId(5)),
            TilePos::new(0, 0),
            Pos2::ZERO,
        );
        assert!(menu.is_none());
    }
}
