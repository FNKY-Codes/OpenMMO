use egui::{Context, Pos2, RichText};
use openmmo_common::{
    ContentPack, EntityId, EntityKind, HarvestTag, PlayerId, RegionDef, TilePos, WorldEntity,
};

use super::theme::{self, TEXT_MUTED};
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
    /// Walk to and open a station (bank chest, furnace, ...).
    Use(EntityId),
    Pickup(EntityId),
    Trade(PlayerId),
    /// Client-side: print the examine text to chat.
    Examine(EntityId),
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
            label: "Walk here".into(),
            action: ContextMenuAction::WalkTo(tile),
        }],
    })
}

/// Verb for harvesting a node, RuneScape style.
pub fn harvest_verb(tag: Option<HarvestTag>) -> &'static str {
    match tag {
        Some(HarvestTag::Timber) => "Chop",
        Some(HarvestTag::Ore) => "Mine",
        Some(HarvestTag::Water) => "Fish",
        Some(HarvestTag::Flora) => "Pick",
        None => "Scavenge",
    }
}

/// The default (left-click) action for an entity, with its label.
pub fn primary_action(
    content: &ContentPack,
    entity: &WorldEntity,
) -> Option<(String, ContextMenuAction)> {
    match &entity.kind {
        EntityKind::GroundItem { .. } => {
            Some(("Take".into(), ContextMenuAction::Pickup(entity.entity_id)))
        }
        EntityKind::Object {
            object_id,
            depleted,
            ..
        } => {
            let def = content.object(*object_id)?;
            if let Some(station) = def.station {
                let verb = match station {
                    openmmo_common::StationTag::Bank => "Open",
                    openmmo_common::StationTag::Market => "Browse",
                    _ => "Use",
                };
                return Some((verb.into(), ContextMenuAction::Use(entity.entity_id)));
            }
            if def.harvest_tag.is_some() && !*depleted {
                return Some((
                    harvest_verb(def.harvest_tag).into(),
                    ContextMenuAction::Scavenge(entity.entity_id),
                ));
            }
            None
        }
        EntityKind::Npc {
            aggro_range,
            npc_id,
            ..
        } => {
            let hostile = *aggro_range > 0 || content.npc(*npc_id).is_some_and(|d| d.prowess > 0);
            if hostile {
                Some(("Attack".into(), ContextMenuAction::Attack(entity.entity_id)))
            } else {
                Some((
                    "Talk-to".into(),
                    ContextMenuAction::TalkTo(entity.entity_id),
                ))
            }
        }
        EntityKind::Boss { .. } => {
            Some(("Attack".into(), ContextMenuAction::Attack(entity.entity_id)))
        }
        EntityKind::Player { player_id, .. } => {
            Some(("Trade with".into(), ContextMenuAction::Trade(*player_id)))
        }
    }
}

fn build_entity_menu(content: &ContentPack, entity: &WorldEntity, screen_pos: Pos2) -> ContextMenu {
    let (title, subtitle) = entity_display_name(content, entity);
    let position = entity_position(entity);
    let mut actions = Vec::new();

    if let Some((label, action)) = primary_action(content, entity) {
        actions.push(ContextMenuEntry { label, action });
    }
    // Hostile NPCs can also be talked to if they have dialogue; friendlies
    // can be attacked only if they have any prowess (guards, etc.).
    if let EntityKind::Npc {
        aggro_range,
        npc_id,
        ..
    } = &entity.kind
    {
        let hostile = *aggro_range > 0 || content.npc(*npc_id).is_some_and(|d| d.prowess > 0);
        if !hostile && content.npc(*npc_id).is_some_and(|d| d.prowess > 0) {
            actions.push(ContextMenuEntry {
                label: "Attack".into(),
                action: ContextMenuAction::Attack(entity.entity_id),
            });
        }
    }
    if let Some(position) = position {
        actions.push(ContextMenuEntry {
            label: "Walk here".into(),
            action: ContextMenuAction::WalkTo(position),
        });
    }
    actions.push(ContextMenuEntry {
        label: "Examine".into(),
        action: ContextMenuAction::Examine(entity.entity_id),
    });

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
            theme::panel_frame().show(ui, |ui| {
                ui.set_min_width(150.0);
                ui.label(RichText::new(&menu.title).color(theme::ACCENT).strong());
                if let Some(subtitle) = &menu.subtitle {
                    ui.label(RichText::new(subtitle).color(TEXT_MUTED).small());
                }
                ui.separator();
                for entry in &menu.actions {
                    let label = format!("{}  {}", entry.label, menu.title);
                    if ui
                        .add(egui::Button::new(RichText::new(label)).frame(false))
                        .clicked()
                    {
                        chosen = Some(entry.action.clone());
                    }
                }
            });
        });
    chosen
}

/// Network message for a menu action; `None` for client-only actions.
pub fn to_client_message(action: ContextMenuAction) -> Option<openmmo_protocol::ClientMessage> {
    use openmmo_common::CombatStyle;
    use openmmo_protocol::ClientMessage;

    Some(match action {
        ContextMenuAction::WalkTo(target) => ClientMessage::WalkIntent { target },
        ContextMenuAction::Attack(target) => ClientMessage::Attack {
            target,
            style: CombatStyle::Melee,
        },
        ContextMenuAction::TalkTo(npc_entity) => ClientMessage::TalkToNpc { npc_entity },
        ContextMenuAction::Scavenge(object_entity) => ClientMessage::Scavenge { object_entity },
        ContextMenuAction::Use(object_entity) => ClientMessage::InteractObject { object_entity },
        ContextMenuAction::Pickup(ground_entity) => ClientMessage::PickupItem { ground_entity },
        ContextMenuAction::Trade(target_player) => ClientMessage::TradeRequest { target_player },
        ContextMenuAction::Examine(_) => return None,
    })
}

/// Examine text for an entity.
pub fn examine_text(content: &ContentPack, entity: &WorldEntity) -> String {
    match &entity.kind {
        EntityKind::Player { name, .. } => format!("{name}. Another survivor."),
        EntityKind::Npc { npc_id, name, .. } => content
            .npc(*npc_id)
            .filter(|d| !d.examine.is_empty())
            .map(|d| d.examine.clone())
            .unwrap_or_else(|| format!("It's a {name}.")),
        EntityKind::Boss { name, .. } => format!("{name}. Run."),
        EntityKind::Object {
            object_id,
            depleted,
            ..
        } => {
            let def = content.object(*object_id);
            match def {
                Some(d) if *depleted => format!("{}. Nothing left to take for now.", d.name),
                Some(d) if !d.examine.is_empty() => d.examine.clone(),
                Some(d) => format!("It's a {}.", d.name),
                None => "You're not sure what that is.".into(),
            }
        }
        EntityKind::GroundItem { item_id, .. } => content
            .item(*item_id)
            .map(|i| {
                if i.examine.is_empty() {
                    format!("It's a {}.", i.name)
                } else {
                    i.examine.clone()
                }
            })
            .unwrap_or_else(|| "Something on the ground.".into()),
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
    region.is_walkable(tile)
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
                ..Default::default()
            }],
            objects: vec![
                openmmo_common::ObjectDef {
                    id: ObjectId(1),
                    name: "Birch Timber".into(),
                    harvest_tag: Some(HarvestTag::Timber),
                    scavenging_level: 1,
                    harvest_item: Some(ItemId(2)),
                    scavenging_ticks: 1,
                    depletes: true,
                    ..Default::default()
                },
                openmmo_common::ObjectDef {
                    id: ObjectId(10),
                    name: "Bank Chest".into(),
                    station: Some(openmmo_common::StationTag::Bank),
                    ..Default::default()
                },
            ],
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
            // centre tile is rock (blocked)
            tiles: vec![0, 0, 0, 0, 1, 0, 0, 0, 0],
            ..Default::default()
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
        assert_eq!(menu.actions.len(), 1);
        assert_eq!(
            menu.actions[0].action,
            ContextMenuAction::WalkTo(TilePos::new(0, 0))
        );
    }

    #[test]
    fn ground_menu_absent_on_blocked_tile() {
        assert!(build_context_menu(
            &ContentPack::default(),
            Some(&region()),
            &[],
            None,
            None,
            TilePos::new(1, 1),
            Pos2::ZERO,
        )
        .is_none());
    }

    #[test]
    fn tree_menu_says_chop_and_depleted_tree_has_no_verb() {
        let mut entity = WorldEntity {
            entity_id: EntityId(3),
            region_id: openmmo_common::RegionId(1),
            kind: EntityKind::Object {
                object_id: ObjectId(1),
                position: TilePos::new(0, 1),
                depleted: false,
            },
        };
        let menu = build_context_menu(
            &sample_content(),
            None,
            &[entity.clone()],
            None,
            Some(EntityId(3)),
            TilePos::new(0, 1),
            Pos2::ZERO,
        )
        .unwrap();
        assert_eq!(menu.title, "Birch Timber");
        assert_eq!(menu.actions[0].label, "Chop");
        assert_eq!(
            menu.actions[0].action,
            ContextMenuAction::Scavenge(EntityId(3))
        );
        assert!(menu.actions.iter().any(|a| a.label == "Examine"));

        if let EntityKind::Object { depleted, .. } = &mut entity.kind {
            *depleted = true;
        }
        let menu = build_context_menu(
            &sample_content(),
            None,
            &[entity],
            None,
            Some(EntityId(3)),
            TilePos::new(0, 1),
            Pos2::ZERO,
        )
        .unwrap();
        assert!(menu.actions.iter().all(|a| a.label != "Chop"));
    }

    #[test]
    fn station_menu_uses_open_and_maps_to_interact() {
        let entity = WorldEntity {
            entity_id: EntityId(4),
            region_id: openmmo_common::RegionId(1),
            kind: EntityKind::Object {
                object_id: ObjectId(10),
                position: TilePos::new(0, 1),
                depleted: false,
            },
        };
        let (label, action) = primary_action(&sample_content(), &entity).unwrap();
        assert_eq!(label, "Open");
        assert!(matches!(
            to_client_message(action),
            Some(openmmo_protocol::ClientMessage::InteractObject { .. })
        ));
        assert!(to_client_message(ContextMenuAction::Examine(EntityId(4))).is_none());
    }

    #[test]
    fn npc_menu_talk_for_friendly_attack_for_hostile() {
        let mut content = sample_content();
        content.npcs.push(openmmo_common::NpcDef {
            id: NpcId(1),
            name: "Rat".into(),
            prowess: 1,
            ..Default::default()
        });
        content.npcs.push(openmmo_common::NpcDef {
            id: NpcId(2),
            name: "Guide".into(),
            ..Default::default()
        });
        let npc = |id: u32, npc_id: u32| WorldEntity {
            entity_id: EntityId(id),
            region_id: openmmo_common::RegionId(1),
            kind: EntityKind::Npc {
                npc_id: NpcId(npc_id),
                name: "x".into(),
                position: TilePos::new(0, 0),
                hp: 5,
                max_hp: 5,
                aggro_range: 0,
            },
        };
        assert_eq!(primary_action(&content, &npc(1, 1)).unwrap().0, "Attack");
        assert_eq!(primary_action(&content, &npc(2, 2)).unwrap().0, "Talk-to");
    }
}
