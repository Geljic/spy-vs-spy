//! Arena mode: walk around a real level decoded from the disc.
//! SVS_MAP=<CreepyCarnival|HauntedHouse|...>; SVS_ARENA_TEST=1 runs a self-check and exits.

use crate::level::Level;
use crate::{SpyCam, SpyRoot};
use bevy::asset::RenderAssetUsages;
use bevy::camera::Viewport;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use bevy::window::PrimaryWindow;

const SPEED: f32 = 5.0;
const GRAVITY: f32 = 20.0;
const STEP_UP: f32 = 0.45;
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.9;

pub const ARENAS: [&str; 8] =
    ["CreepyCarnival", "HauntedHouse", "MedievalMansion", "VolcanicIsland", "Area51", "OilRig", "SpaceStation", "RobotFactory"];

pub struct ArenaSpy {
    pub pos: Vec3,
    pub vy: f32,
    pub facing: Vec2,
    pub grounded: bool,
}

#[derive(Resource)]
pub struct Arena {
    pub level: Level,
    pub spies: [ArenaSpy; 2],
    pub mv: [Vec2; 2],
    pub cam_yaw: [f32; 2],
    pub ai_t: f32,
    test: Option<TestStats>,
}

#[derive(Default)]
struct TestStats {
    steps: u32,
    grounded: u32,
    fell: bool,
    start: [Vec3; 2],
    max_travel: [f32; 2],
    shot: bool,
}

#[derive(Component)]
struct ArenaHud;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup_arena)
        .add_systems(Update, (arena_input, arena_step, arena_sync, arena_cameras, arena_hud, arena_test).chain());
}

/// Find a map by its in-game name inside DATA.PAK; returns the decoded level and the raw map.
fn load_level(name: &str) -> Result<(Level, Vec<u8>), String> {
    let disc = crate::gamedata::find_disc().ok_or("no disc image found (set SVS_DISC)")?;
    let mut pak = crate::gamedata::Pak::open(&disc).map_err(|e| e.to_string())?;
    let maps: Vec<String> = pak.names().filter(|n| n.starts_with("Maps/")).map(String::from).collect();
    for m in maps {
        let d = pak.read(&m).map_err(|e| e.to_string())?;
        let n = &d[0x34..0x48];
        let n = String::from_utf8_lossy(&n[..n.iter().position(|&b| b == 0).unwrap_or(20)]).into_owned();
        if n.eq_ignore_ascii_case(name) {
            return Ok((Level::from_map(&n, &d), d));
        }
    }
    Err(format!("no map called {name}"))
}

fn setup_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let name = std::env::var("SVS_MAP").unwrap_or_else(|_| ARENAS[0].into());
    let (mut level, map_bytes) = match load_level(&name) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("arena: {e}");
            std::process::exit(2);
        }
    };
    info!("arena {}: {} triangles, bounds {:?}..{:?}", level.name, level.tris.len(), level.min, level.max);

    let mut models = crate::model::load(&map_bytes);
    let ents = crate::model::entities(&map_bytes);
    info!("{} entities", ents.len());
    place_templates(&mut level, &mut models, &ents, &map_bytes);
    level.add_visual(models.parts.iter().filter(|p| !p.is_sky()).flat_map(|p| {
        p.indices.chunks_exact(3).map(|t| t.iter().map(|&i| Vec3::from_array(p.positions[i as usize])).collect::<Vec<_>>().try_into().unwrap())
    }));
    spawn_level_meshes(&mut commands, &level, models, &map_bytes, &mut meshes, &mut mats, &mut images);
    commands.insert_resource(GlobalAmbientLight { brightness: 350.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3500.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(30.0, 60.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // start where the original does: the Player entity, and the AI spy's (falls back to a floor spot)
    let start = |s: usize| -> Option<(Vec3, Vec2)> {
        let e = ents.iter().find(|e| if s == 0 { e.class.starts_with("Spy_HumanPlayer") } else { e.class.starts_with("AISpy") })?;
        let p = Vec3::from_array(e.pos);
        // the spy faces along the entity's Y axis (file forward)
        let f = Vec2::new(e.axes[1][0], e.axes[1][2]).normalize_or(Vec2::new(0.0, -1.0));
        let start = match level.ground(p + Vec3::Y * 1.0, p.y + 1.0) {
            Some(g) if p.y - g < 3.0 => Vec3::new(p.x, g, p.z),
            // some starts sit just off the collision (intro cutscene spots): use the nearest floor
            _ => level.nearest_floor(p, 8.0)?,
        };
        Some((start, f))
    };
    let spies = [0, 1].map(|s| {
        let (pos, facing) = start(s).unwrap_or((level.spawn_point(s), Vec2::new(0.0, -1.0)));
        ArenaSpy { pos, vy: 0.0, facing, grounded: false }
    });
    for s in 0..2 {
        crate::spawn_spy(&mut commands, s, &mut meshes, &mut mats);
        commands.spawn((
            Camera3d::default(),
            Camera { order: s as isize, ..default() },
            // closer to the PS2's plain output than the default filmic curve
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            SpyCam(s),
        ));
    }
    commands.spawn((Camera2d, Camera { order: 2, clear_color: ClearColorConfig::None, ..default() }, IsDefaultUiCamera));
    commands.spawn((
        Text::new(""),
        TextFont::from_font_size(bevy::text::FontSize::Px(16.0)),
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(10.0), ..default() },
        ArenaHud,
    ));
    let test = std::env::var("SVS_ARENA_TEST").is_ok().then(|| TestStats { start: [spies[0].pos, spies[1].pos], ..default() });
    for (k, sp) in spies.iter().enumerate() {
        info!("spy {k} starts at ({:.1}, {:.1}, {:.1})", sp.pos.x, sp.pos.y, sp.pos.z);
    }
    let cam_yaw = [0, 1].map(|s: usize| spies[s].facing.x.atan2(spies[s].facing.y));
    commands.insert_resource(Arena { level, spies, mv: [Vec2::ZERO; 2], cam_yaw, ai_t: 0.0, test });
}

fn arena_input(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut arena: ResMut<Arena>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    let maps = [
        (KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD),
        (KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight),
    ];
    let pads: Vec<&Gamepad> = pads.iter().collect();
    for s in 0..2 {
        let (u, d, l, r) = maps[s];
        let ax = |a: KeyCode, b: KeyCode| keys.pressed(b) as i32 as f32 - keys.pressed(a) as i32 as f32;
        let mut raw = Vec2::new(ax(l, r), ax(d, u));
        if let Some(p) = pads.get(s) {
            if p.left_stick().length() > 0.2 {
                raw = p.left_stick();
            }
        }
        let yaw = arena.cam_yaw[s];
        let fwd = Vec2::new(yaw.sin(), yaw.cos());
        let right = Vec2::new(-fwd.y, fwd.x);
        arena.mv[s] = (fwd * raw.y + right * raw.x).clamp_length_max(1.0);
    }
}

fn arena_step(time: Res<Time>, mut arena: ResMut<Arena>) {
    let dt = if arena.test.is_some() { 1.0 / 60.0 } else { time.delta_secs().min(0.05) };
    let a = &mut *arena;
    // in test mode both spies walk in widening circles to exercise floors and walls
    if a.test.is_some() {
        a.ai_t += dt;
        for s in 0..2 {
            let ang = a.ai_t * (0.6 + 0.2 * s as f32) + s as f32 * 2.0;
            a.mv[s] = Vec2::new(ang.cos(), ang.sin());
        }
    }
    for s in 0..2 {
        let mv = a.mv[s];
        let spy = &mut a.spies[s];
        if mv.length_squared() > 0.01 {
            spy.facing = mv.normalize();
        }
        let mut p = spy.pos + Vec3::new(mv.x, 0.0, mv.y) * SPEED * dt;
        p = a.level.push_out(p, RADIUS, HEIGHT);
        // gravity + ground following (can step up small ledges)
        spy.vy -= GRAVITY * dt;
        p.y += spy.vy * dt;
        // stick to downhill slopes and stairs while walking instead of floating off them
        let snap = if spy.grounded { 0.35 } else { 0.02 };
        match a.level.ground(p, spy.pos.y.max(p.y) + STEP_UP) {
            Some(g) if p.y <= g + snap => {
                p.y = g;
                spy.vy = 0.0;
                spy.grounded = true;
            }
            _ => spy.grounded = false,
        }
        // fell out of the world: back to spawn
        if p.y < a.level.min.y - 10.0 {
            warn!("spy {s} fell out of the level near ({:.1}, {:.1}, {:.1}) from start", spy.pos.x, spy.pos.y, spy.pos.z);
            p = a.level.spawn_point(s);
            spy.vy = 0.0;
            if let Some(t) = a.test.as_mut() {
                t.fell = true;
            }
        }
        spy.pos = p;
    }
    if let Some(t) = a.test.as_mut() {
        t.steps += 1;
        t.grounded += a.spies.iter().filter(|s| s.grounded).count() as u32;
        for s in 0..2 {
            t.max_travel[s] = t.max_travel[s].max(a.spies[s].pos.distance(t.start[s]));
        }
    }
}

fn arena_sync(arena: Res<Arena>, mut q: Query<(&SpyRoot, &mut Transform)>) {
    for (SpyRoot(s), mut tf) in &mut q {
        let spy = &arena.spies[*s];
        *tf = Transform::from_translation(spy.pos).with_rotation(Quat::from_rotation_y(spy.facing.x.atan2(spy.facing.y)));
    }
}

fn arena_cameras(
    time: Res<Time>,
    mut arena: ResMut<Arena>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cams: Query<(&SpyCam, &mut Camera, &mut Transform)>,
) {
    let Ok(win) = window.single() else { return };
    let (w, h) = (win.physical_width(), win.physical_height());
    if w < 4 || h < 4 {
        return;
    }
    let dt = time.delta_secs();
    for (SpyCam(s), mut cam, mut tf) in &mut cams {
        let s = *s;
        cam.viewport = Some(Viewport {
            physical_position: UVec2::new(if s == 0 { 0 } else { w / 2 }, 0),
            physical_size: UVec2::new(w / 2, h),
            ..default()
        });
        let spy = &arena.spies[s];
        let face = spy.facing.x.atan2(spy.facing.y);
        let diff = (face - arena.cam_yaw[s] + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let follow = if arena.mv[s].length_squared() > 0.01 { 2.0 } else { 0.0 };
        arena.cam_yaw[s] += diff * (follow * dt).min(1.0);
        let yaw = arena.cam_yaw[s];
        let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let target = arena.spies[s].pos + Vec3::Y * 1.3;
        // pull the camera in front of any wall between it and the spy
        let want = -fwd * 4.5 + Vec3::Y * 1.4;
        let dist = want.length();
        let dir = want / dist;
        let d = arena.level.raycast(target, dir, dist).map_or(dist, |hit| (hit - 0.3).max(0.4));
        let eye = target + dir * d;
        *tf = Transform::from_translation(eye).looking_at(target + fwd * 1.5, Vec3::Y);
        // debug: SVS_CAM="x,y,z,tx,ty,tz" pins White's camera (for checking geometry against the original)
        if s == 0 {
            if let Some(v) = std::env::var("SVS_CAM").ok().map(|c| c.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<_>>()) {
                if v.len() == 6 {
                    *tf = Transform::from_xyz(v[0], v[1], v[2]).looking_at(Vec3::new(v[3], v[4], v[5]), Vec3::Y);
                }
            }
        }
    }
}

fn arena_hud(arena: Res<Arena>, mut q: Query<&mut Text, With<ArenaHud>>) {
    for mut t in &mut q {
        let p = arena.spies[0].pos;
        **t = format!(
            "ARENA: {} ({} collision triangles from your disc)  |  WHITE: WASD  BLACK: arrows  Esc quit\nwhite at ({:.1}, {:.1}, {:.1}) {}",
            arena.level.name,
            arena.level.tris.len(),
            p.x,
            p.y,
            p.z,
            if arena.spies[0].grounded { "on ground" } else { "in air" }
        );
    }
}

/// SVS_ARENA_TEST: walk for 12 s of game time, then check the spies stayed on the level.
fn arena_test(mut arena: ResMut<Arena>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    let name = arena.level.name.clone();
    let Some(t) = arena.test.as_mut() else { return };
    if t.steps == 600 && !t.shot {
        t.shot = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(crate::shot_path(&format!("arena_{name}.png"))));
    }
    if t.steps < 12 * 60 {
        return;
    }
    let ground_frac = t.grounded as f32 / (t.steps * 2) as f32;
    let checks = [
        ("never fell out of the level", !t.fell),
        ("on the ground most of the time (>80%)", ground_frac > 0.8),
        ("white actually moved (>3 m)", t.max_travel[0] > 3.0),
        ("black actually moved (>3 m)", t.max_travel[1] > 3.0),
    ];
    let mut ok = true;
    for (what, pass) in checks {
        println!("  {}  {what}", if pass { "ok  " } else { "FAIL" });
        ok &= pass;
    }
    println!(
        "{} arena {name}: grounded {:.0}%, travel {:.1} m / {:.1} m",
        if ok { "PASS" } else { "FAIL" },
        ground_frac * 100.0,
        t.max_travel[0],
        t.max_travel[1]
    );
    exit.write(if ok { AppExit::Success } else { AppExit::error() });
    t.steps = 0; // don't report twice
}

/// Classes whose placed models are drawn: not managers, effects, camera rigs or the spies themselves.
fn drawn_class(class: &str) -> bool {
    !["Mgr_", "FX_", "fx_", "MM_MouseCameraPath", "Chall_BrdgKillPlane", "Spy_", "AISpy_"].iter().any(|p| class.starts_with(p))
}

/// Classes whose placed models block movement, until their behaviour (opening, falling, swinging) exists.
/// Provisional: the engine's own solid/trigger rule isn't decoded yet. Kill planes and lasers stay out.
fn solid_class(class: &str) -> bool {
    ["Door_", "Chall_Brdg_", "Chall_ChandSecDoor", "Safe_", "Keypad", "MM_KeyPad", "MM_Computer", "Cellar_Comp", "Switch", "SpySign", "MM_Bridge_Target", "Workbench"]
        .iter()
        .any(|p| class.starts_with(p))
}

/// Put each entity's model template into the world: render parts and (for solid classes) collision.
fn place_templates(level: &mut Level, models: &mut crate::model::LevelModels, ents: &[crate::model::Entity], map: &[u8]) {
    let tpls = crate::model::templates(map);
    let (mut drawn, mut solid) = (0, 0);
    for e in ents {
        let Some(t) = e.template.as_ref().and_then(|n| tpls.get(n)) else { continue };
        if drawn_class(&e.class) {
            drawn += 1;
            for p in &t.parts {
                // template space is file z-up; place() converts to Bevy y-up metres
                models.parts.push(crate::model::Part {
                    object: p.object.clone(),
                    material: p.material.clone(),
                    positions: p.positions.iter().map(|&v| e.place(v)).collect(),
                    normals: p.normals.iter().map(|&n| e.turn(n)).collect(),
                    uvs: p.uvs.clone(),
                    indices: p.indices.clone(),
                });
            }
        }
        if solid_class(&e.class) && !t.collision.is_empty() {
            solid += 1;
            level.add_tris(t.collision.iter().map(|tri| tri.map(|v| Vec3::from_array(e.place(v)))));
        }
    }
    info!("{} templates; placed {drawn} models, {solid} solid", tpls.len());
}

/// The real level: textured render meshes from the disc. Falls back to the collision shape.
fn spawn_level_meshes(
    commands: &mut Commands,
    level: &Level,
    models: crate::model::LevelModels,
    map: &[u8],
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use std::collections::HashMap;

    let tex = crate::gamedata::load_container(map).textures;
    info!("level render: {} objects, {} parts, {} textures", models.objects, models.parts.len(), tex.len());
    if models.parts.is_empty() {
        // no render data decoded: show the collision shape instead
        let mut pos = Vec::new();
        let mut nrm = Vec::new();
        for t in &level.tris {
            for v in t.v {
                pos.push(v.to_array());
                nrm.push(t.n.to_array());
            }
        }
        let n = pos.len() as u32;
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_indices(Indices::U32((0..n).collect()));
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(Color::srgb(0.6, 0.55, 0.45))), Transform::IDENTITY));
        return;
    }
    // material name -> texture name, with the naming fallbacks the data needs
    let find_tex = |m: &str| -> Option<String> {
        if let Some(t) = models.mat_tex.get(m) {
            if tex.contains_key(t) {
                return Some(t.clone());
            }
        }
        let lower = m.to_ascii_lowercase();
        let mut cands = vec![m.to_string(), lower.clone()];
        for suf in ["_ds", "_noalpha", "_shado", "_recproj", "_shadow"] {
            if let Some(stem) = lower.strip_suffix(suf) {
                cands.push(stem.to_string());
            }
        }
        cands.into_iter().find_map(|c| tex.keys().find(|k| k.eq_ignore_ascii_case(&c)).cloned())
    };
    // merge parts by texture
    let mut merged: HashMap<(Option<String>, bool), crate::model::Part> = HashMap::new();
    for p in models.parts {
        let key = (find_tex(&p.material), p.is_sky());
        let m = merged.entry(key).or_default();
        let base = m.positions.len() as u32;
        m.positions.extend(p.positions);
        m.normals.extend(p.normals);
        m.uvs.extend(p.uvs);
        m.indices.extend(p.indices.into_iter().map(|i| i + base));
    }
    let mut handles: HashMap<String, (Handle<Image>, u8)> = HashMap::new();
    for ((key, sky), part) in merged {
        let (image, cutout) = match &key {
            Some(name) => handles
                .entry(name.clone())
                .or_insert_with(|| {
                    let t = &tex[name];
                    // binary alpha -> cut-out; soft alpha (light shafts, glass) -> blended
                    let n = (t.rgba.len() / 4).max(1) as f32;
                    let low = t.rgba.chunks_exact(4).filter(|p| p[3] < 32).count() as f32 / n;
                    let mid = t.rgba.chunks_exact(4).filter(|p| p[3] >= 32 && p[3] < 224).count() as f32 / n;
                    let cutout = if mid > 0.15 { 2u8 } else if low > 0.0 { 1 } else { 0 };
                    let mut img = Image::new(
                        Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
                        TextureDimension::D2,
                        t.rgba.clone(),
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::RENDER_WORLD,
                    );
                    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        ..ImageSamplerDescriptor::linear()
                    });
                    (images.add(img), cutout)
                })
                .clone(),
            None => (Handle::default(), 0),
        };
        let material = StandardMaterial {
            base_color: if key.is_some() { Color::WHITE } else { Color::srgb(0.6, 0.6, 0.6) },
            base_color_texture: key.as_ref().map(|_| image),
            alpha_mode: match (cutout, key.as_deref().is_some_and(|k| k.contains("light") || k.contains("beam"))) {
                (2, true) => AlphaMode::Add,
                (2, false) => AlphaMode::Blend,
                (1, _) => AlphaMode::Mask(0.5),
                _ => AlphaMode::Opaque,
            },
            perceptual_roughness: 0.95,
            double_sided: true,
            cull_mode: None,
            unlit: sky,
            ..default()
        };
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, part.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs)
            .with_inserted_indices(Indices::U32(part.indices));
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(material)), Transform::IDENTITY));
    }
}
