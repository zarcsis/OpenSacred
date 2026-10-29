//! The talks (#1028; docs/re/netscript.md, "Dialogues"): a left click on a creature of a dialogue
//! sends the hero within 1.5 times their reach, where the creature stops, both turn to each other
//! and the creature plays its talk; its function `Dialog:<name>` runs (`scripts::speak`), and a
//! window at the screen's middle shows the creature's name, its texts and its buttons, until the
//! original's interface of M5 (#1087). A button closes it, ends the talk and runs its function
//! (`scripts::answer`); without buttons Enter or a click closes it; the talk ends too where the
//! hero stands farther than 1,500 from the creature. Its voice-over ends as it closes. The world
//! runs on meanwhile, the hero's clicks on it held back while the window shows
//! (docs/decisions/interface.md, "Dialogue window holds back world clicks").

use bevy::picking::Pickable;
use bevy::picking::events::PointerClick;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy_kira_audio::prelude::{Audio, AudioControl};
use opensacred_data::tables::PORTRAITS;
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::attack::{self, Aim};
use crate::fight::Health;
use crate::files;
use crate::frame::{Control, Playing, Step, Unpaused};
use crate::game::{AppState, FONT};
use crate::hero::{self, Body, Size};
use crate::lines::Texts;
use crate::npcs::{Dies, Npcs, ScriptName, Still, Talk, Talks, Walk};
use crate::scripts::{self, SHOWN};
use crate::sfx::Sfx;
use crate::voices;

/// How far, past 1.5 times the two creatures' reach, the talk opens (`0x529C60`).
const OPENS_WITHIN: f32 = 1.5;
/// How far apart the hero and the creature end their talk (`0x5370B0`).
const ENDS_BEYOND: f32 = 1_500.0;
/// How far the creature moves from where the hero walks before the hero heads for it again, and
/// how many times the hero stops short of it before it gives the talk up.
const MOVED: f32 = 50.0;
const STOPS: u8 = 3;
/// The sides of the creatures that a type's dialogue of `SetRgnDialog` speaks for (`0x549920`),
/// and the bits of their flags word, one of which they need, and the one they must lack.
const REGION_SIDES: [u8; 6] = [3, 7, 8, 12, 14, 15];
const REGION_FLAGS: u32 = 0x1006_DCF8;
const NOT_REGION: u32 = 0x4_0000;

/// The name's colour (`0xFFFFFF80`), the texts', the buttons' and the panel's.
const NAME: Color = Color::srgb(1.0, 1.0, 0.5);
const WORDS: Color = Color::srgb(0.92, 0.92, 0.88);
const BUTTON: Color = Color::srgb(0.25, 0.9, 0.25);
const PANEL: Color = Color::srgba(0.04, 0.04, 0.05, 0.78);
const FRAME: Color = Color::srgb(0.45, 0.42, 0.36);
const FONT_SIZE: f32 = 18.0;
/// An `InfoPlayer`'s text: its colour, as the chapters' titles show, how long it shows, in
/// seconds, and how far down the screen (**hypothesis**: the engine's event `0x10` of the kind 3
/// is not read).
const NOTICE: Color = Color::srgb(1.0, 0.55, 0.12);
const NOTICE_SECONDS: f32 = 5.0;
const NOTICE_TOP: f32 = 22.0;
/// The modes of an `InfoPlayer` that show its text (`0x4AE350`): 0 and 1, 2 with the music
/// `0x1B61` once, which has no file, and 3 with the sound 7000, which no tree's `InfoPlayer` asks
/// for (#1243).
const SHOWS: [i32; 4] = [0, 1, 2, 3];

/// An `InfoPlayer`'s text on the screen, and how long it still shows.
#[derive(Component)]
struct Notice(f32);

/// The hero speaking to `creature`, its dialogue `dialogue`: going to it until the talk opens.
#[derive(Component, Clone, Debug)]
pub struct Talking {
    creature: Entity,
    dialogue: String,
    open: bool,
    /// How many times the hero has stopped short of the creature.
    stops: u8,
}

/// The creatures' portraits of the dialogues' window (#1032): the table of `tables/portraits.ron`
/// and its textures' files by their names; none without the table (assets converted before it).
#[derive(Resource, Default)]
pub struct Portraits {
    table: Option<opensacred_data::tables::Portraits>,
    files: std::collections::HashMap<String, String>,
}

impl Portraits {
    /// Reads `tables/portraits.ron` of `assets` and finds its textures.
    pub fn read(assets: &std::path::Path) -> anyhow::Result<Self> {
        let path = assets.join("tables").join(PORTRAITS);
        if !files::exists(&path) {
            return Ok(Self::default());
        }
        let text = files::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let table: opensacred_data::tables::Portraits =
            ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        let mut names: Vec<&str> = table
            .creatures
            .iter()
            .map(|c| c.portrait.texture.as_str())
            .chain([table.default.texture.as_str()])
            .collect();
        names.sort_unstable();
        names.dedup();
        let found = crate::hud::find_textures(assets, &names);
        let files = names
            .into_iter()
            .zip(found)
            .filter_map(|(name, file)| Some((name.to_owned(), file?)))
            .collect();
        Ok(Self {
            table: Some(table),
            files,
        })
    }

    /// The texture's file of the creature type `creature`'s portrait, and its part: its left,
    /// top, width and height.
    fn of(&self, creature: u32) -> Option<(&str, [u16; 4])> {
        let portrait = self.table.as_ref()?.of(creature);
        Some((self.files.get(&portrait.texture)?.as_str(), portrait.rect))
    }
}

/// A portrait in the window: its texture, and its part, its left, top, width and height.
type Portrait = (Handle<Image>, [u16; 4]);

/// The dialogue's window while it shows: its root, the creature, and its buttons' functions.
#[derive(Resource, Clone, Debug)]
pub struct Shown {
    root: Entity,
    creature: Entity,
    functions: Vec<String>,
    /// Its name and texts, and its buttons' labels, which the command `answer` tells.
    text: String,
    labels: Vec<String>,
}

/// The dialogue a creature speaks, its own (`+0x245`, [`Talk`]) or, without one, its type's of
/// `SetRgnDialog` where it is of the townsfolk's sides and flags and hostile neither way
/// (`0x549920`); found anew at each query.
pub(crate) fn dialogue_of(talk: Option<&Talk>, mind: &Mind, npcs: Option<&Npcs>) -> Option<String> {
    let region = |creature| npcs?.region_talk(creature).map(str::to_owned);
    dialogue_by(talk, mind, region)
}

/// [`dialogue_of`], its type's dialogue of `SetRgnDialog` by `region`.
fn dialogue_by(
    talk: Option<&Talk>,
    mind: &Mind,
    region: impl FnOnce(u32) -> Option<String>,
) -> Option<String> {
    if let Some(talk) = talk {
        return Some(talk.0.clone());
    }
    let townsfolk = REGION_SIDES.contains(&mind.side)
        && mind.flags & REGION_FLAGS != 0
        && mind.flags & NOT_REGION == 0
        && !mind.hostile();
    townsfolk.then(|| region(mind.creature)).flatten()
}

/// On a press of the left button, the creature of a dialogue alive nearest the cursor's line of
/// sight becomes the hero's to speak to, the hero walking to it, ahead of an item to take
/// (`0x53A170`); any other press ends a talk not open yet.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::hud::Pointer>,
    mut aim: ResMut<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    npcs: Option<Res<Npcs>>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Mind,
            &Health,
            Option<&Talk>,
            &crate::crowd::InTheWay,
        ),
        Without<Dies>,
    >,
    mut heroes: Query<(Entity, &mut Body, Option<&Talking>)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (hero, _, talking) in &heroes {
        if talking.is_some_and(|talking| !talking.open) {
            commands.entity(hero).remove::<Talking>();
        }
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let Some((creature, (at, level), dialogue)) = creatures
        .iter()
        .filter(|(_, _, _, health, ..)| health.current > 0)
        .filter_map(|(entity, at, mind, _, talk, way)| {
            let dialogue = dialogue_of(talk, mind, npcs.as_deref())?;
            let at = at.translation();
            Some((
                entity,
                (at, way.level),
                dialogue,
                attack::from_line(ray, at),
            ))
        })
        .filter(|&(.., distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(entity, at, dialogue, _)| (entity, at, dialogue))
    else {
        return;
    };
    aim.0 = true;
    for (hero, mut body, _) in &mut heroes {
        debug!("talk: the hero goes to speak to {creature} ({dialogue})");
        body.send_onto(at, level);
        // The strike goes with its attack: `attack::engage`, which alone ends it, takes only a
        // hero that attacks (#1215).
        commands
            .entity(hero)
            .remove::<(attack::Attack, attack::Striking)>()
            .insert(Talking {
                creature,
                dialogue: dialogue.clone(),
                open: false,
                stops: 0,
            });
    }
}

/// The hero going to speak to a creature: within 1.5 times their reach it stops, the creature
/// stops, both turn to each other and the creature plays its talk, and the dialogue runs
/// ([`open`]); farther it walks to the creature, heading for it again as it moves, and gives the
/// talk up where it stops short of it again and again, or where the creature is gone or dead
/// (`0x529C60`, `0x52AB70`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn approach(
    mut commands: Commands,
    mut heroes: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        &mut Talking,
        Option<&Size>,
    )>,
    mut creatures: Query<
        (&mut Transform, &Mind, &Health, &crate::crowd::InTheWay),
        (Without<Body>, Without<Dies>),
    >,
) {
    for (hero, mut transform, mut body, mut talking, size) in &mut heroes {
        if talking.open {
            continue;
        }
        let Ok((mut theirs, mind, health, way)) = creatures.get_mut(talking.creature) else {
            commands.entity(hero).remove::<Talking>();
            continue;
        };
        if health.current == 0 {
            commands.entity(hero).remove::<Talking>();
            continue;
        }
        let here = transform.translation;
        let there = theirs.translation;
        let size = size.map_or(minds::SIZE, |size| size.0);
        let within = OPENS_WITHIN * minds::reach_f32(size, mind.size());
        if (there - here).xz().length() > within {
            // An order waits for the world's next tick (`hero::take_orders`).
            if body.ordered() {
                continue;
            }
            let heading = body.target();
            if heading.is_none() {
                talking.stops += 1;
                if talking.stops > STOPS {
                    debug!("talk: the hero stands too far from {}", talking.creature);
                    commands.entity(hero).remove::<Talking>();
                    continue;
                }
            }
            if heading.is_none_or(|to| (to - there).xz().length() > MOVED) {
                body.send_onto(there, way.level);
            }
            continue;
        }
        body.stop();
        transform.rotation = hero::facing(there - here);
        theirs.rotation = hero::facing(here - there);
        talking.open = true;
        let creature = talking.creature;
        commands
            .entity(creature)
            .remove::<Walk>()
            .insert((Still, Talks));
        let dialogue = talking.dialogue.clone();
        commands.queue(move |world: &mut World| open(world, hero, creature, &dialogue));
    }
}

/// Runs the creature's dialogue (`scripts::speak`) and shows its window: its name, its texts and
/// its first four buttons, its sounds played; a dialogue that takes the creature's away
/// (`DelDlg`) takes it; one without a text ends the talk at once.
fn open(world: &mut World, hero: Entity, creature: Entity, dialogue: &str) {
    let window = scripts::speak(world, dialogue).unwrap_or_default();
    if window.untalks
        && let Ok(mut entity) = world.get_entity_mut(creature)
    {
        entity.remove::<Talk>();
        if let Some(mut mind) = entity.get_mut::<Mind>() {
            mind.dialogue = false;
        }
    }
    let texts = world.get_resource::<Texts>();
    let lines: Vec<String> = window
        .texts
        .iter()
        .filter_map(|key| texts.and_then(|texts| texts.named(key)).map(shown_text))
        .collect();
    if lines.is_empty() {
        info!("talk: {dialogue} shows no text");
        end(world, hero, creature);
        return;
    }
    let name = world
        .get::<ScriptName>(creature)
        .and_then(|name| texts.and_then(|texts| texts.named(resource(&name.0))))
        .map(str::to_owned);
    let buttons: Vec<(String, String)> = window
        .buttons
        .iter()
        .take(SHOWN)
        .filter_map(|(label, function)| {
            let label = texts.and_then(|texts| texts.named(resource(label)))?;
            Some((label.to_owned(), function.clone()))
        })
        .collect();
    info!(
        "talk: {dialogue} shows {} texts and {} buttons",
        lines.len(),
        buttons.len()
    );
    voice(world, &window.sounds);
    let font: Handle<Font> = world.resource::<AssetServer>().load(FONT);
    // Its portrait by its type (`0x4345C0`, #1032); the hero's own talk shows none.
    let portrait = world
        .get::<Mind>(creature)
        .map(|mind| mind.creature)
        .and_then(|kind| {
            let (file, rect) = world.get_resource::<Portraits>()?.of(kind)?;
            Some((file.to_owned(), rect))
        })
        .map(|(file, rect)| {
            let image = crate::hud::image(world.resource::<AssetServer>(), file);
            (image, rect)
        });
    let functions = buttons.iter().map(|(_, f)| f.clone()).collect();
    let text = lines.join("\n");
    let root = spawn_window(
        &mut world.commands(),
        &font,
        (name.as_deref(), portrait),
        &text,
        &buttons,
    );
    world.flush();
    world.insert_resource(Shown {
        root,
        creature,
        functions,
        text: name.map_or(text.clone(), |name| format!("{name}\n\n{text}")),
        labels: buttons.into_iter().map(|(label, _)| label).collect(),
    });
}

/// A text of `global.res` as the window shows it: its line breaks (`<n>`, 3,760 of them in the
/// English texts) broken, its colours' tags (`<color=…>`, `<colorw=…>`) left out, and any other
/// tag, a speaker's sigh or groan, kept as it stands.
fn shown_text(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tag = &rest[start..];
        let Some(end) = tag.find('>') else {
            out.push_str(tag);
            return out;
        };
        match &tag[1..end] {
            "n" => out.push('\n'),
            inner if inner.starts_with("color") => {}
            _ => out.push_str(&tag[..=end]),
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The key of a name of the scripts' resources, `res:<key>` (case aside), as the window looks
/// its string up: `res:17095` the string of `17095`.
pub(crate) fn resource(name: &str) -> &str {
    match name.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("res:") => &name[4..],
        _ => name,
    }
}

/// Plays the dialogue's sounds, its voice-overs under their texts' keys (`sound.pak`'s
/// `SOUND_FX_<key>`, `0x4A9730`), each the line by name that stops the one before, the hero's
/// own too (`0x693FE0`, `voices::NamedLine`).
fn voice(world: &mut World, sounds: &[String]) {
    for sound in sounds {
        let name = format!("SOUND_FX_{}", sound.to_ascii_uppercase());
        let Some((id, file)) = world.get_resource::<Sfx>().and_then(|sfx| sfx.named(&name)) else {
            debug!("talk: no sound {name}");
            continue;
        };
        let assets = world.resource::<AssetServer>().clone();
        let Some(mut sfx) = world.get_resource_mut::<Sfx>() else {
            return;
        };
        let sample = sfx.sample(id, file, &assets);
        let Some(audio) = world.get_resource::<Audio>() else {
            return;
        };
        let instance = audio.play(sample).handle();
        voices::name_line(world, Some(instance));
    }
}

/// Stops the line by name that plays, the window's voice-over or the hero's own line
/// (`0x693FB0`).
fn hush(world: &mut World) {
    voices::name_line(world, None);
}

/// The window: a panel at the screen's middle, the name over the texts, the buttons under them,
/// each choosing its function ([`choose`]).
fn spawn_window(
    commands: &mut Commands,
    font: &Handle<Font>,
    (name, portrait): (Option<&str>, Option<Portrait>),
    text: &str,
    buttons: &[(String, String)],
) -> Entity {
    let words = |size: f32| TextFont {
        font: font.clone().into(),
        font_size: FontSize::Px(size),
        ..default()
    };
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        width: Val::Percent(76.0),
                        max_width: Val::Px(900.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(16.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(FRAME),
                ))
                .with_children(|panel| {
                    // The portrait, 8 pixels in from the corner (`0x6E76C0`).
                    if let Some((image, [x, y, w, h])) = portrait {
                        let (x, y, w, h) = (f32::from(x), f32::from(y), f32::from(w), f32::from(h));
                        panel.spawn((
                            ImageNode {
                                image,
                                rect: Some(Rect::new(x, y, x + w, y + h)),
                                ..default()
                            },
                            Node {
                                width: Val::Px(w),
                                height: Val::Px(h),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                    }
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            row_gap: Val::Px(12.0),
                            padding: UiRect::all(Val::Px(10.0)),
                            ..default()
                        })
                        .with_children(|panel| {
                            if let Some(name) = name {
                                panel.spawn((Text::new(name), words(FONT_SIZE), TextColor(NAME)));
                            }
                            panel.spawn((
                                Text::new(text),
                                words(FONT_SIZE),
                                TextColor(WORDS),
                                TextLayout::justify(Justify::Justified),
                            ));
                            panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceEvenly,
                            ..default()
                        })
                        .with_children(|row| {
                            for (index, (label, _)) in buttons.iter().enumerate() {
                                row.spawn((Text::new(label), words(FONT_SIZE), TextColor(BUTTON)))
                                    .observe(
                                        move |click: On<PointerClick>, mut commands: Commands| {
                                            if click.button == PointerButton::Primary {
                                                commands.queue(move |world: &mut World| {
                                                    choose(world, index);
                                                });
                                            }
                                        },
                                    );
                            }
                        });
                        });
                });
        })
        .id()
}

/// A button chosen: the window closes, the talk ends, and its function runs with no creature
/// (`0x6E74E0`, `0x499DA0`).
fn choose(world: &mut World, index: usize) {
    let Some(shown) = world.get_resource::<Shown>().cloned() else {
        return;
    };
    close(world);
    if let Some(function) = shown.functions.get(index) {
        info!("talk: the button {index} runs {function}");
        scripts::answer(world, function);
    }
}

/// Closes the window, if one shows, its voice-over stopped, and ends the talk.
fn close(world: &mut World) {
    let Some(shown) = world.remove_resource::<Shown>() else {
        return;
    };
    hush(world);
    if let Ok(root) = world.get_entity_mut(shown.root) {
        root.despawn();
    }
    let mut heroes = world.query_filtered::<Entity, With<Talking>>();
    let heroes: Vec<Entity> = heroes.iter(world).collect();
    for hero in heroes {
        end(world, hero, shown.creature);
    }
}

/// Ends the talk of `hero` with `creature`: the creature free to go again.
fn end(world: &mut World, hero: Entity, creature: Entity) {
    if let Ok(mut hero) = world.get_entity_mut(hero) {
        hero.remove::<Talking>();
    }
    if let Ok(mut creature) = world.get_entity_mut(creature) {
        creature.remove::<(Still, Talks)>();
    }
}

/// Closes a window without buttons on Enter or a press of a mouse button (`0x655920`), and any
/// window once the hero and the creature stand farther than 1,500 apart or the creature is gone
/// (`0x5370B0`).
fn dismiss(
    mut commands: Commands,
    shown: Option<Res<Shown>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    heroes: Query<&Transform, With<Body>>,
    creatures: Query<&Transform, Without<Dies>>,
) {
    let Some(shown) = shown else {
        return;
    };
    let pressed = shown.functions.is_empty()
        && (keys.just_pressed(KeyCode::Enter) || buttons.get_just_pressed().next().is_some());
    let apart = match (heroes.iter().next(), creatures.get(shown.creature)) {
        (Some(hero), Ok(creature)) => {
            (creature.translation - hero.translation).xz().length() > ENDS_BEYOND
        }
        _ => true,
    };
    if pressed || apart {
        commands.queue(close);
    }
}

/// Shows the scripts' `InfoPlayer`s' texts (#1033): of a mode that shows one, the last of the
/// frame's, in place of the one before, across the screen's upper part, for 5 s; the other modes
/// (an image, an event, the interface's states) are not followed.
fn notices(
    mut commands: Commands,
    mut infos: ResMut<scripts::Infos>,
    texts: Option<Res<Texts>>,
    assets: Res<AssetServer>,
    shown: Query<Entity, With<Notice>>,
) {
    let mut last = None;
    for (mode, key) in infos.0.drain(..) {
        if !SHOWS.contains(&mode) {
            debug!("talk: an InfoPlayer of the mode {mode} ({key}) is not followed");
            continue;
        }
        match texts
            .as_deref()
            .and_then(|texts| texts.named(resource(&key)))
        {
            Some(text) => last = Some(shown_text(text)),
            None => debug!("talk: no text {key}"),
        }
    }
    let Some(text) = last else {
        return;
    };
    info!(
        "talk: the player is shown {}",
        text.lines().next().unwrap_or_default()
    );
    for old in &shown {
        commands.entity(old).despawn();
    }
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(NOTICE_TOP),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            Notice(NOTICE_SECONDS),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(text),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(FONT_SIZE * 1.4),
                    ..default()
                },
                TextColor(NOTICE),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// Takes an `InfoPlayer`'s text away once its time is over.
fn fade(mut commands: Commands, time: Res<Time>, mut notices: Query<(Entity, &mut Notice)>) {
    for (entity, mut notice) in &mut notices {
        notice.0 -= time.delta_secs();
        if notice.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Begins the hero's own talk of each dialogue the scripts' `Popup`s give it (`0x4A7760`,
/// `0x54CBB0`; #1033), one at a time, once no window shows: its window without a name.
fn popups(
    mut commands: Commands,
    mut popups: ResMut<scripts::Popups>,
    shown: Option<Res<Shown>>,
    heroes: Query<Entity, With<Body>>,
) {
    if shown.is_some() || popups.0.is_empty() {
        return;
    }
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    match popups.0.remove(0) {
        scripts::Begins::Own(dialogue) => {
            info!("talk: the hero's own talk of {dialogue}");
            commands.queue(move |world: &mut World| open(world, hero, hero, &dialogue));
        }
        // The creature takes the dialogue, and the hero speaks to it as a click on it does
        // (`0x49DCF0`: the hero's talk's action toward it).
        scripts::Begins::With { creature, dialogue } => {
            commands.queue(move |world: &mut World| {
                let mut named = world.query::<(Entity, &crate::npcs::ScriptName)>();
                let found = named
                    .iter(world)
                    .find(|(_, name)| name.0 == creature)
                    .map(|(entity, _)| entity);
                let Some(entity) = found else {
                    info!("talk: no {creature} in the world to speak {dialogue}");
                    return;
                };
                world.entity_mut(entity).insert(Talk(dialogue.clone()));
                match talk_to(world, Some(entity)) {
                    Ok(line) => info!("talk: {creature} takes {dialogue}: {line}"),
                    Err(e) => info!("talk: {creature} takes {dialogue}: {e}"),
                }
            });
        }
    }
}

/// Sends the hero to speak to `creature`, or to the creature of a dialogue nearest it (the
/// command `talk`), as a click on it does: a line of which.
pub fn talk_to(world: &mut World, creature: Option<Entity>) -> Result<String, String> {
    let mut heroes = world.query_filtered::<(Entity, &Transform), With<Body>>();
    let (hero, here) = heroes
        .iter(world)
        .next()
        .map(|(hero, transform)| (hero, transform.translation))
        .ok_or("the hero is not in the world yet")?;
    let found: Vec<(Entity, Vec3, crate::terrain::Level, String)> = {
        let mut creatures = world.query_filtered::<(
            Entity,
            &Transform,
            &Mind,
            &Health,
            Option<&Talk>,
            &crate::crowd::InTheWay,
        ), Without<Dies>>();
        let npcs = world.get_resource::<Npcs>();
        creatures
            .iter(world)
            .filter(|(entity, _, _, health, ..)| {
                health.current > 0 && creature.is_none_or(|c| c == *entity)
            })
            .filter_map(|(entity, at, mind, _, talk, way)| {
                Some((
                    entity,
                    at.translation,
                    way.level,
                    dialogue_of(talk, mind, npcs)?,
                ))
            })
            .collect()
    };
    let (creature, at, level, dialogue) = found
        .into_iter()
        .min_by(|a, b| {
            let d = |at: Vec3| (at - here).xz().length_squared();
            d(a.1).total_cmp(&d(b.1))
        })
        .ok_or("no creature of a dialogue is near")?;
    let mut body = world
        .get_mut::<Body>(hero)
        .ok_or("the hero is not in the world yet")?;
    body.send_onto(at, level);
    world
        .entity_mut(hero)
        .remove::<(crate::attack::Attack, crate::attack::Striking)>()
        .insert(Talking {
            creature,
            dialogue: dialogue.clone(),
            open: false,
            stops: 0,
        });
    Ok(format!("the hero goes to speak to {creature} ({dialogue})"))
}

/// Chooses the shown window's button `button` (from 1), as a click on it does, or tells its
/// lines without one (the command `answer`).
pub fn answer(world: &mut World, button: Option<usize>) -> Result<String, String> {
    let shown = world
        .get_resource::<Shown>()
        .cloned()
        .ok_or("no dialogue's window shows")?;
    let Some(button) = button else {
        let mut lines = vec![shown.text.clone()];
        lines.extend(
            shown
                .labels
                .iter()
                .enumerate()
                .map(|(i, label)| format!("{}: {label}", i + 1)),
        );
        return Ok(lines.join("\n"));
    };
    let index = button
        .checked_sub(1)
        .filter(|&i| i < shown.functions.len())
        .ok_or_else(|| format!("the window has {} buttons", shown.functions.len()))?;
    choose(world, index);
    Ok(format!(
        "the button {button} runs {}",
        shown.functions[index]
    ))
}

/// Whether no dialogue's window shows, which holds back the hero's clicks on the world.
fn no_window(shown: Option<Res<Shown>>) -> bool {
    shown.is_none()
}

/// Leaves no window and no voice-over behind a game.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Shown>();
    commands.queue(hush);
}

/// The talks.
pub struct TalkPlugin;

impl Plugin for TalkPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, Control.run_if(no_window))
            .add_systems(OnExit(AppState::InGame), forget)
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .after(attack::aim)
                        .after(hero::steer)
                        .after(crate::doors::pick)
                        .before(crate::pickup::pick),
                    approach.after(hero::run),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            )
            .add_systems(Update, (dismiss, notices, fade, popups).in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creature_speaks_its_own_dialogue_or_its_types_as_townsfolk() {
        let farmer = |side: u8, flags: u32| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                flags,
                (false, true),
                None,
                [70.0, 180.0],
                (677, 3),
            );
            mind.side = side;
            mind
        };
        let region = |creature: u32| (creature == 677).then(|| "NPC_Dialog_Farmer".to_owned());
        let own = Talk("auftrag10".into());
        // Its own first, whatever it is.
        assert_eq!(
            dialogue_by(Some(&own), &farmer(2, 0x1), region).as_deref(),
            Some("auftrag10")
        );
        // A townsperson of the side 3 and the flags 8 takes its type's.
        assert_eq!(
            dialogue_by(None, &farmer(3, 0x8), region).as_deref(),
            Some("NPC_Dialog_Farmer")
        );
        // Not of a hostile side, nor without the flags, nor of the flag 0x40000.
        assert_eq!(dialogue_by(None, &farmer(2, 0x8), region), None);
        assert_eq!(dialogue_by(None, &farmer(3, 0), region), None);
        assert_eq!(dialogue_by(None, &farmer(3, 0x4_0008), region), None);
    }

    #[test]
    fn a_text_breaks_its_lines_and_leaves_its_colours_out() {
        assert_eq!(shown_text("one.<n><n>two"), "one.\n\ntwo");
        assert_eq!(
            shown_text("<colorw=FFFFFF80>name <sigh> on"),
            "name <sigh> on"
        );
        assert_eq!(shown_text("open <n"), "open <n");
    }

    #[test]
    fn a_resource_name_is_looked_up_by_its_key() {
        assert_eq!(resource("res:17095"), "17095");
        assert_eq!(resource("Res:1024"), "1024");
        assert_eq!(resource("RES:HQ_1"), "HQ_1");
        assert_eq!(resource("Dialog"), "Dialog");
        assert_eq!(resource("ré"), "ré");
    }
}
