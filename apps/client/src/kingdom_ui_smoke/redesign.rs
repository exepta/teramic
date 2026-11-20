//! Six real keep tabs, authoritative catalog costs and responsive DE/EN layouts.
use super::*;
use terra_core::{GameLanguage, GameplayConfig};
use terra_ui::BuildingView;

const TABS: [&str; 6] = [
    "overview",
    "members",
    "diplomacy",
    "building",
    "work",
    "army",
];
const TITLES: [&str; 6] = [
    "general",
    "members",
    "diplomacy",
    "buildings",
    "workers",
    "army",
];

#[derive(Resource, Default)]
struct Proof {
    claims_checked: bool,
}

pub(super) fn requested() -> bool {
    std::env::var("TERRA_KINGDOM_REDESIGN_GALLERY").as_deref() == Ok("1")
}

fn check(world: &mut World) -> Result<(), String> {
    let root = element(world, "kingdom-dialog").ok_or("Missing redesigned keep frame")?;
    let frame = rect(world, root).ok_or("Missing redesigned frame geometry")?;
    let window = world
        .query::<&Window>()
        .iter(world)
        .next()
        .ok_or("Missing window")?;
    let viewport = Rect::from_corners(Vec2::ZERO, window.resolution.physical_size().as_vec2());
    if !within(frame, viewport) {
        return Err("Redesigned keep leaves viewport".into());
    }
    let scroll = element(world, "kingdom-scroll").ok_or("Missing keep content")?;
    let content = rect(world, scroll).ok_or("Missing content geometry")?;
    if !within(
        content,
        Rect::from_corners(frame.min + Vec2::splat(15.0), frame.max - Vec2::splat(15.0)),
    ) {
        return Err("Redesigned keep content intersects its decorative border".into());
    }
    let buttons: Vec<_> = world
        .query_filtered::<Entity, With<tilt_ui::TiltButton>>()
        .iter(world)
        .filter(|entity| belongs(world, *entity, root))
        .collect();
    let texts: Vec<_> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| !text.0.trim().is_empty())
        .map(|(entity, text)| (entity, text.0.clone()))
        .collect();
    for button in buttons {
        let Some(box_rect) = rect(world, button) else {
            continue;
        };
        if box_rect.size().min_element() <= 0.0 {
            continue;
        }
        for (entity, text) in &texts {
            if belongs(world, *entity, button)
                && let Some(caption) = rect(world, *entity)
                && !within(caption, box_rect)
            {
                return Err(format!(
                    "Redesigned control caption leaves button: {text:?}/{caption:?}/{box_rect:?}"
                ));
            }
        }
    }
    Ok(())
}

fn reveal(world: &mut World, id: &str) -> Result<bool, String> {
    let entity =
        element(world, id).ok_or_else(|| format!("Missing real redesigned control: {id}"))?;
    let target = rect(world, entity).ok_or("Missing control geometry")?;
    let scroll = element(world, "kingdom-scroll").ok_or("Missing keep viewport")?;
    let viewport = rect(world, scroll).ok_or("Missing viewport geometry")?;
    if within(target, viewport) {
        return Ok(true);
    }
    let scale = world
        .get::<UiGlobalTransform>(scroll)
        .map_or(1.0, |pose| pose.matrix2.x_axis.length().max(0.1));
    world
        .get_mut::<ScrollPosition>(scroll)
        .ok_or("Keep cannot scroll")?
        .0
        .y += (target.center().y - viewport.center().y) / scale;
    Ok(false)
}

fn select_cottage(world: &mut World) -> Result<bool, String> {
    if world
        .resource::<BuildingView>()
        .templates
        .iter()
        .any(|template| template.id == "spruce-cottage-v1" && template.selected)
    {
        return Ok(true);
    }
    let cards: Vec<_> = world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(_, id, node)| {
            id.0 == "building-select-template" && node.size().min_element() > 0.0
        })
        .map(|(entity, _, _)| entity)
        .collect();
    let labels: Vec<_> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| matches!(text.0.as_str(), "Fichtenhütte" | "Spruce cottage"))
        .map(|(entity, _)| entity)
        .collect();
    if let Some(card) = cards
        .into_iter()
        .find(|card| labels.iter().any(|label| belongs(world, *label, *card)))
    {
        crate::building_local_smoke::emit_button(world, card)?;
    }
    Ok(false)
}

fn check_costs(world: &World) -> Result<(), String> {
    let view = world.resource::<BuildingView>();
    let session = world.resource::<ClientSession>();
    let selected = session
        .buildings
        .templates
        .iter()
        .find(|template| template.id == "spruce-cottage-v1")
        .ok_or("Real cottage missing from server catalog")?;
    if !view.template_selected || view.template_materials.len() != selected.materials.len() {
        return Err("Selected material preview is incomplete".into());
    }
    for (row, stack) in view.template_materials.iter().zip(&selected.materials) {
        if row.identifier != stack.item.identifier() || row.count != stack.count.to_string() {
            return Err("Native displayed material cost differs from authoritative JSON".into());
        }
    }
    Ok(())
}

fn check_selection(world: &mut World, id: &str, label: &str) -> Result<(), String> {
    let labels: Vec<_> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(_, text)| text.0 == label)
        .map(|(entity, _)| entity)
        .collect();
    let buttons: Vec<_> = world
        .query::<(Entity, &ElementId, &tilt_ui::ElementClasses, &ComputedNode)>()
        .iter(world)
        .filter(|(_, element, _, node)| element.0 == id && node.size().min_element() > 0.0)
        .map(|(entity, _, classes, _)| {
            (
                entity,
                classes.classes.iter().any(|class| class == "selected"),
            )
        })
        .collect();
    let mut selected = 0;
    for (button, marked) in buttons {
        let expected = labels.iter().any(|entity| belongs(world, *entity, button));
        if marked != expected {
            return Err(format!(
                "Actual selection class disagrees with {id}/{label}"
            ));
        }
        selected += usize::from(marked);
    }
    if selected != 1 {
        return Err(format!("Expected exactly one selected {id}/{label}"));
    }
    Ok(())
}

pub(super) fn run(world: &mut World) -> Result<bool, String> {
    if !world.contains_resource::<Gallery>() {
        let member = create_peer(world)?;
        world.insert_resource(Gallery {
            step: 0,
            since: Instant::now(),
            captures: 0,
            capture_pending: false,
            failed: false,
            selected: None,
            member: Some(member),
        });
        world.init_resource::<Proof>();
    }
    if world.contains_non_send::<GalleryPeer>() {
        world.non_send_mut::<GalleryPeer>().app.update();
    }
    world.resource_scope(|world, mut gallery: Mut<Gallery>| advance(world, &mut gallery))
}

fn advance(world: &mut World, gallery: &mut Gallery) -> Result<bool, String> {
    if gallery.failed {
        return Err("Redesign capture has no rendered UI".into());
    }
    if gallery.capture_pending || gallery.since.elapsed().as_secs_f32() < 0.7 {
        return Ok(false);
    }
    if gallery.step == 0 {
        let peer = world.non_send::<GalleryPeer>();
        if !peer.app.world().resource::<ClientSession>().ready {
            return Ok(false);
        }
        if peer.request == 0 {
            let id = own_kingdom(world)?;
            peer_request(world, KingdomAction::Join { id })?;
            return Ok(false);
        }
        let member = gallery.member.ok_or("Missing real joined member")?;
        if !world
            .resource::<ClientSession>()
            .kingdoms
            .get(own_kingdom(world)?)
            .is_some_and(|realm| realm.members.contains(&member))
        {
            return Ok(false);
        }
        for mut window in world.query::<&mut Window>().iter_mut(world) {
            window.resolution.set_physical_resolution(1600, 1000);
        }
        world.resource_mut::<GameplayConfig>().language = GameLanguage::German;
    } else if gallery.step <= 24 {
        let index = usize::from((gallery.step - 1) / 2);
        let tab = index % 6;
        let english = index >= 6;
        if (gallery.step - 1).is_multiple_of(2) {
            if index == 6 {
                for mut window in world.query::<&mut Window>().iter_mut(world) {
                    window.resolution.set_physical_resolution(1024, 768);
                }
                world.resource_mut::<GameplayConfig>().language = GameLanguage::English;
            }
            scroll_to_top(world);
            crate::menu_smoke::emit(world, &format!("kingdom-tab-{}", TABS[tab]), None)?;
        } else {
            let view = world.resource::<KingdomView>();
            let active = [
                view.overview_tab,
                view.members_tab,
                view.diplomacy_tab,
                view.building_tab,
                view.work_tab,
                view.army_tab,
            ];
            if !active[tab] || view.pending {
                return Ok(false);
            }
            if tab == 0 && !world.resource::<Proof>().claims_checked {
                if !reveal(world, "kingdom-claim-protected")? {
                    return Ok(false);
                }
                world.resource_mut::<Proof>().claims_checked = true;
                scroll_to_top(world);
                gallery.since = Instant::now();
                return Ok(false);
            }
            if tab == 1 {
                let member = gallery.member.ok_or("Missing real member")?;
                if !view
                    .members
                    .iter()
                    .any(|row| row.id == member.to_string() && row.selected)
                {
                    world.write_message(KingdomMenuAction::SelectMember(member));
                    gallery.since = Instant::now();
                    return Ok(false);
                }
            }
            if tab == 2
                && !view
                    .diplomacy
                    .iter()
                    .any(|realm| realm.key == "bot:lutera" && realm.selected)
            {
                world.write_message(KingdomMenuAction::SelectRealm("bot:lutera".into()));
                gallery.since = Instant::now();
                return Ok(false);
            }
            if tab == 3 {
                if world.resource::<BuildingView>().templates.len() < 2 || !select_cottage(world)? {
                    gallery.since = Instant::now();
                    return Ok(false);
                }
                check_costs(world)?;
            }
            if tab == 4 && world.resource::<ClientSession>().work.workers.is_empty() {
                return Ok(false);
            }
            if tab == 5 && world.resource::<KingdomView>().sections.army.is_empty() {
                return Ok(false);
            }
            match tab {
                1 => check_selection(world, "kingdom-select-member", "F57 Visitor")?,
                2 => check_selection(world, "kingdom-select-realm", "Lutera")?,
                3 => {
                    let label = terra_ui::localized_text(
                        world,
                        &world.resource::<BuildingView>().template_name,
                    );
                    check_selection(world, "building-select-template", &label)?;
                }
                _ => {}
            }
            check(world)?;
            let language = if english { "en-compact" } else { "de-wide" };
            let name = format!("keep-{}-{language}", TITLES[tab]);
            shot(world, gallery, &name)?;
        }
    } else {
        if gallery.captures != 12 {
            return Ok(false);
        }
        info!(
            captures = gallery.captures,
            "Native keep redesign passed: six real tabs, DE/EN responsive frames, actual second member, loaded army, actual template card and authoritative material preview"
        );
        return Ok(true);
    }
    gallery.step += 1;
    gallery.since = Instant::now();
    Ok(false)
}
