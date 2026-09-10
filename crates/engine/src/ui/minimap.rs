//! Minimap: a texture of the region's tiles (built once per region) with a
//! window around the player, entity dots, a compass and the HP orb.
//! Clicking the map walks there.

use egui::{Color32, ColorImage, Context, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use openmmo_common::{
    ContentPack, EntityKind, PlayerId, PropModel, RegionDef, RegionId, TileKind, TilePos,
    WorldEntity,
};

use super::theme;
use crate::world_style::minimap_color;

pub struct MinimapCache {
    pub region: RegionId,
    pub texture: TextureHandle,
}

/// Map size on screen in points and how many tiles it spans.
const MAP_PX: f32 = 168.0;
const VIEW_TILES: f32 = 44.0;

fn build_texture(ctx: &Context, region: &RegionDef, content: &ContentPack) -> TextureHandle {
    let w = region.width as usize;
    let h = region.height as usize;
    let mut img = ColorImage::new([w.max(1), h.max(1)], Color32::BLACK);
    for y in 0..h {
        for x in 0..w {
            let byte = region.tiles.get(y * w + x).copied().unwrap_or(0);
            let kind = TileKind::from_byte(byte).unwrap_or(TileKind::Rock);
            let [r, g, b] = minimap_color(kind);
            img.pixels[y * w + x] = Color32::from_rgb(r, g, b);
        }
    }
    // Static objects: trees, nodes and stations get RS-style map marks.
    for obj in &region.objects {
        let Some(def) = content.object(obj.object_id) else {
            continue;
        };
        let (x, y) = (obj.position.x, obj.position.y);
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            continue;
        }
        let color = match def.model_or_default() {
            PropModel::Tree | PropModel::Oak | PropModel::Pine | PropModel::Bush => {
                Color32::from_rgb(30, 90, 30)
            }
            PropModel::DeadTree | PropModel::Stump => Color32::from_rgb(70, 55, 35),
            PropModel::OreVein => Color32::from_rgb(200, 120, 50),
            PropModel::Pool => Color32::from_rgb(90, 170, 230),
            PropModel::Mushroom => Color32::from_rgb(120, 220, 200),
            PropModel::Crystal => Color32::from_rgb(160, 230, 230),
            PropModel::BankBooth => Color32::from_rgb(240, 200, 60),
            PropModel::Furnace | PropModel::Anvil | PropModel::Workbench | PropModel::Campfire => {
                Color32::from_rgb(240, 140, 60)
            }
            PropModel::MarketStall => Color32::from_rgb(230, 230, 120),
            PropModel::Tent => Color32::from_rgb(200, 180, 120),
            _ => continue,
        };
        img.pixels[y as usize * w + x as usize] = color;
    }
    for t in &region.transitions {
        let (x, y) = (t.position.x, t.position.y);
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            img.pixels[y as usize * w + x as usize] = Color32::from_rgb(230, 90, 230);
        }
    }
    ctx.load_texture(
        format!("minimap_{}", region.id.0),
        img,
        egui::TextureOptions::NEAREST,
    )
}

pub struct MinimapInput<'a> {
    pub region: Option<&'a RegionDef>,
    pub content: &'a ContentPack,
    pub entities: &'a [WorldEntity],
    pub local_player: Option<PlayerId>,
    pub player_tile: Option<TilePos>,
    pub hp: u32,
    pub max_hp: u32,
}

/// Draws the minimap in the top-right corner. Returns a tile if the player
/// clicked the map.
pub fn draw_minimap(
    ctx: &Context,
    cache: &mut Option<MinimapCache>,
    input: &MinimapInput<'_>,
) -> Option<TilePos> {
    let mut walk_to = None;
    egui::Area::new(egui::Id::new("minimap_area"))
        .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-8.0, 8.0))
        .order(egui::Order::Middle)
        .interactable(true)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    // HP orb
                    let (orb_rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    draw_hp_orb(ui, orb_rect, input.hp, input.max_hp);

                    // Map
                    let (map_rect, resp) =
                        ui.allocate_exact_size(Vec2::splat(MAP_PX), Sense::click());
                    let painter = ui.painter();
                    painter.rect_filled(map_rect, 3.0, Color32::BLACK);
                    let Some(region) = input.region else {
                        return;
                    };
                    if cache.as_ref().map(|c| c.region) != Some(region.id) {
                        *cache = Some(MinimapCache {
                            region: region.id,
                            texture: build_texture(ctx, region, input.content),
                        });
                    }
                    let Some(c) = cache.as_ref() else { return };
                    let center = input
                        .player_tile
                        .map(|t| (t.x as f32 + 0.5, t.y as f32 + 0.5))
                        .unwrap_or((region.width as f32 / 2.0, region.height as f32 / 2.0));
                    let half = VIEW_TILES / 2.0;
                    let w = region.width as f32;
                    let h = region.height as f32;
                    // Window in tile space, shifted to stay inside the region.
                    let mut x0 = center.0 - half;
                    let mut y0 = center.1 - half;
                    if w > VIEW_TILES {
                        x0 = x0.clamp(0.0, w - VIEW_TILES);
                    } else {
                        x0 = (w - VIEW_TILES) / 2.0;
                    }
                    if h > VIEW_TILES {
                        y0 = y0.clamp(0.0, h - VIEW_TILES);
                    } else {
                        y0 = (h - VIEW_TILES) / 2.0;
                    }
                    let uv = Rect::from_min_max(
                        Pos2::new(x0 / w, y0 / h),
                        Pos2::new((x0 + VIEW_TILES) / w, (y0 + VIEW_TILES) / h),
                    );
                    // Only the part of the uv inside [0,1] has texture; draw the image
                    // over the whole map rect but clip to the region's extent.
                    let px_per_tile = MAP_PX / VIEW_TILES;
                    let tile_to_screen = |tx: f32, ty: f32| {
                        Pos2::new(
                            map_rect.min.x + (tx - x0) * px_per_tile,
                            map_rect.min.y + (ty - y0) * px_per_tile,
                        )
                    };
                    let region_rect =
                        Rect::from_min_max(tile_to_screen(0.0, 0.0), tile_to_screen(w, h))
                            .intersect(map_rect);
                    let clip_uv = Rect::from_min_max(
                        Pos2::new(uv.min.x.max(0.0), uv.min.y.max(0.0)),
                        Pos2::new(uv.max.x.min(1.0), uv.max.y.min(1.0)),
                    );
                    painter.image(c.texture.id(), region_rect, clip_uv, Color32::WHITE);

                    // Entity dots
                    for e in input.entities {
                        let (pos, color, r) = match &e.kind {
                            EntityKind::Player {
                                player_id,
                                position,
                                ..
                            } => (
                                *position,
                                if Some(*player_id) == input.local_player {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(235, 235, 235)
                                },
                                2.5,
                            ),
                            EntityKind::Npc {
                                position,
                                aggro_range,
                                ..
                            } => (
                                *position,
                                if *aggro_range > 0 {
                                    Color32::from_rgb(240, 200, 60)
                                } else {
                                    Color32::from_rgb(120, 220, 255)
                                },
                                2.0,
                            ),
                            EntityKind::Boss { position, .. } => {
                                (*position, Color32::from_rgb(230, 60, 220), 3.0)
                            }
                            EntityKind::GroundItem { position, .. } => {
                                (*position, Color32::from_rgb(230, 60, 60), 1.8)
                            }
                            EntityKind::Object { .. } => continue,
                        };
                        let p = tile_to_screen(pos.x as f32 + 0.5, pos.y as f32 + 0.5);
                        if map_rect.contains(p) {
                            painter.circle_filled(p, r, color);
                        }
                    }
                    // Local player marker on top.
                    if let Some(t) = input.player_tile {
                        let p = tile_to_screen(t.x as f32 + 0.5, t.y as f32 + 0.5);
                        painter.circle_filled(p, 3.0, Color32::WHITE);
                        painter.circle_stroke(p, 3.0, Stroke::new(1.0, Color32::BLACK));
                    }
                    painter.rect_stroke(map_rect, 3.0, Stroke::new(1.5, theme::PANEL_BORDER));
                    // Compass
                    painter.circle_filled(
                        map_rect.left_top() + Vec2::splat(11.0),
                        9.0,
                        theme::PANEL_BG_DARK,
                    );
                    painter.text(
                        map_rect.left_top() + Vec2::splat(11.0),
                        egui::Align2::CENTER_CENTER,
                        "N",
                        egui::FontId::proportional(12.0),
                        theme::ACCENT,
                    );
                    if resp.clicked() {
                        if let Some(p) = resp.interact_pointer_pos() {
                            let tx = x0 + (p.x - map_rect.min.x) / px_per_tile;
                            let ty = y0 + (p.y - map_rect.min.y) / px_per_tile;
                            if tx >= 0.0 && ty >= 0.0 && tx < w && ty < h {
                                walk_to = Some(TilePos::new(tx as i32, ty as i32));
                            }
                        }
                    }
                });
                ui.label(
                    egui::RichText::new(input.region.map(|r| r.name.as_str()).unwrap_or(""))
                        .color(theme::TEXT_MUTED)
                        .small(),
                );
            });
        });
    walk_to
}

fn draw_hp_orb(ui: &mut egui::Ui, rect: Rect, hp: u32, max_hp: u32) {
    let painter = ui.painter();
    let ratio = if max_hp > 0 {
        hp as f32 / max_hp as f32
    } else {
        0.0
    };
    let c = rect.center();
    let r = rect.width() * 0.5 - 2.0;
    painter.circle_filled(c, r, theme::PANEL_BG_DARK);
    // Fill from the bottom up.
    let fill_h = (2.0 * r) * ratio;
    let fill_rect = Rect::from_min_max(
        Pos2::new(c.x - r, c.y + r - fill_h),
        Pos2::new(c.x + r, c.y + r),
    );
    let mut clipped = painter.clone();
    clipped.set_clip_rect(fill_rect.intersect(rect));
    clipped.circle_filled(c, r, theme::hp_color(ratio));
    painter.circle_stroke(c, r, Stroke::new(1.5, theme::PANEL_BORDER));
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        hp.to_string(),
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(c.x, c.y + r + 8.0),
        egui::Align2::CENTER_CENTER,
        "HP",
        egui::FontId::proportional(10.0),
        theme::TEXT_MUTED,
    );
}
