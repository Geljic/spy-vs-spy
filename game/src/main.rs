//! Spy vs. Spy: clean-room PC rewrite. Classic-mode prototype.
//!
//! Reads textures and sounds from your own disc image at runtime (see gamedata.rs).
//! Env: SVS_DISC=<path to .iso/.mdf>, SVS_DEMO=1 (AI vs AI), SVS_SCREENSHOT=<png> (+ SVS_SHOT_AFTER=secs)

mod ai;
mod arena;
mod level;
mod model;
mod gamedata;
mod rules;
mod script;

use bevy::asset::RenderAssetUsages;
use bevy::camera::Viewport;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use bevy::window::PrimaryWindow;
use rules::*;
use std::collections::HashMap;

const SPACING: f32 = 30.0;
const WALL_H: f32 = 3.4;
const WALL_T: f32 = 0.3;

const SPY_COLORS: [Color; 2] = [Color::srgb(0.95, 0.95, 0.93), Color::srgb(0.07, 0.07, 0.09)];

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Spy vs. Spy (rewrite prototype)".into(),
            resolution: (1600, 900).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)));
    if std::env::var("SVS_MAP").is_ok() {
        // free-roam a real level decoded from the disc
        app.add_plugins(arena::plugin);
    } else {
        app.add_systems(Startup, (load_disc_assets, setup).chain()).add_systems(
            Update,
            (
                (controls, step_game, sync_spies, sync_furniture, sync_cameras, hud, play_sounds).chain(),
                auto_screenshot,
                script_outputs,
            ),
        );
    }
    app.run()
}

// ---------- resources & components ----------

#[derive(Resource)]
struct Game {
    m: Match,
    brains: [ai::Brain; 2],
    ai: [bool; 2],
    inputs: [SpyInput; 2],
    seed: u64,
    cam_yaw: [f32; 2],
    cam_orbit: [f32; 2],
    cam_room: [usize; 2],
}

#[derive(Resource, Default)]
struct ScriptRun(Option<script::Script>);

#[derive(Resource, Default)]
struct DiscAssets {
    textures: HashMap<String, Handle<Image>>,
    sounds: HashMap<String, Handle<AudioSource>>,
    status: String,
}

#[derive(Component)]
struct SpyRoot(usize);
#[derive(Component)]
struct SpyClub(usize);
#[derive(Component)]
struct SpyCam(usize);
#[derive(Component)]
struct FurnVisual(usize, Handle<StandardMaterial>);
#[derive(Component)]
struct WorldEntity;
#[derive(Component)]
struct HudText(usize);
#[derive(Component)]
struct MsgText(usize);
#[derive(Component)]
struct TimerText;
#[derive(Component)]
struct CenterText;

fn room_origin(m: &Match, r: usize) -> Vec3 {
    let room = &m.rooms[r];
    Vec3::new(room.gx as f32 * SPACING, 0.0, room.gy as f32 * SPACING)
}

fn world_pos(m: &Match, r: usize, p: Vec2) -> Vec3 {
    room_origin(m, r) + Vec3::new(p.x, 0.0, p.y)
}

// ---------- disc assets ----------

const MAP_FOR_TEXTURES: &str = "Maps/50AEC05532C44EAD.map"; // Embassy_Room_05

fn load_disc_assets(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut audio: ResMut<Assets<AudioSource>>) {
    let mut out = DiscAssets::default();
    let Some(disc) = gamedata::find_disc() else {
        out.status = "No disc image found: using plain colours. Set SVS_DISC to your .iso/.mdf".into();
        warn!("{}", out.status);
        commands.insert_resource(out);
        return;
    };
    let loaded = gamedata::Pak::open(&disc).and_then(|mut pak| pak.read(MAP_FOR_TEXTURES));
    match loaded {
        Ok(data) => {
            let a = gamedata::load_container(&data);
            for (name, t) in a.textures {
                let mut img = Image::new(
                    Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    t.rgba,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::RENDER_WORLD,
                );
                img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
                out.textures.insert(name, images.add(img));
            }
            for (name, wav) in a.sounds {
                out.sounds.insert(name, audio.add(AudioSource { bytes: wav.into() }));
            }
            out.status = format!("Disc: {} ({} textures, {} sounds)", disc.display(), out.textures.len(), out.sounds.len());
            info!("{}", out.status);
        }
        Err(e) => {
            out.status = format!("Could not read {}: {e}", disc.display());
            warn!("{}", out.status);
        }
    }
    commands.insert_resource(out);
}

// ---------- world building ----------

fn setup(
    mut commands: Commands,
    disc: Res<DiscAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let demo = std::env::var("SVS_DEMO").is_ok();
    let sc = std::env::var("SVS_SCRIPT").ok().map(|p| match script::Script::load(&p) {
        Ok(sc) => sc,
        Err(e) => {
            eprintln!("script error: {e}");
            std::process::exit(2);
        }
    });
    let seed = sc.as_ref().map(|s| s.seed).unwrap_or(seed);
    let mut ai = [demo, true];
    if let Some(sc) = &sc {
        println!("running test script {}", sc.name);
        ai = [sc.ai[0].unwrap_or(false), sc.ai[1].unwrap_or(false)];
    }
    commands.insert_resource(ScriptRun(sc));
    let game = Game {
        m: Match::new(seed),
        brains: Default::default(),
        ai,
        inputs: Default::default(),
        seed,
        cam_yaw: [0.0; 2],
        cam_orbit: [0.0; 2],
        cam_room: [usize::MAX; 2],
    };
    spawn_world(&mut commands, &game.m, &disc, &mut meshes, &mut mats);
    commands.insert_resource(game);

    commands.insert_resource(GlobalAmbientLight { brightness: 700.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 0.0, ..default() },
        Transform::from_xyz(20.0, 40.0, 30.0).looking_at(Vec3::new(30.0, 0.0, 30.0), Vec3::Y),
    ));

    // one camera per spy (split screen) + a full-window UI camera on top
    for s in 0..2 {
        commands.spawn((Camera3d::default(), Camera { order: s as isize, ..default() }, SpyCam(s)));
    }
    commands.spawn((
        Camera2d,
        Camera { order: 2, clear_color: ClearColorConfig::None, ..default() },
        IsDefaultUiCamera,
    ));
    spawn_hud(&mut commands, &disc.status);
}

fn tex_mat(
    mats: &mut Assets<StandardMaterial>,
    disc: &DiscAssets,
    tex: &str,
    fallback: Color,
    tiling: Vec2,
) -> Handle<StandardMaterial> {
    let t = disc.textures.get(tex).cloned();
    mats.add(StandardMaterial {
        base_color: if t.is_some() { Color::WHITE } else { fallback },
        base_color_texture: t,
        uv_transform: Affine2::from_scale(tiling),
        perceptual_roughness: 0.85,
        ..default()
    })
}

fn spawn_world(
    commands: &mut Commands,
    m: &Match,
    disc: &DiscAssets,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    let floor_tex = ["e2_emb_floor_1", "e2_emb_floor_2", "e2_emb_floor_3"];
    let wall_tex = ["e2_emb_wall_1", "e2_emb_wallblue", "e2_emb_wallgreen", "e2_emb_woodpanel_1"];
    let floor_mesh = meshes.add(Cuboid::new(HALF * 2.0, 0.1, HALF * 2.0));
    let door_mat = tex_mat(mats, disc, "o2_embassydoor", Color::srgb(0.45, 0.3, 0.15), Vec2::ONE);
    let frame_mat = mats.add(Color::srgb(0.25, 0.17, 0.1));
    let ceil_mat = tex_mat(mats, disc, "e2_emb_ceiling_1", Color::srgb(0.8, 0.8, 0.75), Vec2::splat(3.0));
    let exit_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.3),
        emissive: LinearRgba::rgb(0.4, 2.0, 0.6),
        ..default()
    });

    for (r, room) in m.rooms.iter().enumerate() {
        let o = room_origin(m, r);
        let fm = tex_mat(mats, disc, floor_tex[r % floor_tex.len()], Color::srgb(0.55, 0.5, 0.42), Vec2::splat(3.0));
        let wm = tex_mat(mats, disc, wall_tex[r % wall_tex.len()], Color::srgb(0.6, 0.62, 0.7), Vec2::new(3.0, 1.0));
        commands.spawn((Mesh3d(floor_mesh.clone()), MeshMaterial3d(fm), Transform::from_translation(o - Vec3::Y * 0.05), WorldEntity));
        commands.spawn((Mesh3d(floor_mesh.clone()), MeshMaterial3d(ceil_mat.clone()), Transform::from_translation(o + Vec3::Y * (WALL_H + 0.05)), WorldEntity));

        for side in 0..4 {
            // the wall nearest the camera is a low cut-away so we can see in
            let h = WALL_H;
            let has_door = room.doors[side].is_some();
            let is_exit = room.exit_side == Some(side);
            let dir = side_dir(side);
            let along_x = side % 2 == 0; // north/south walls run along x
            let centre = o + Vec3::new(dir.x, 0.0, dir.y) * (HALF + WALL_T / 2.0);
            let seg = |len: f32, offset: f32, y0: f32, y1: f32| -> (Cuboid, Vec3) {
                let size = if along_x { Vec3::new(len, y1 - y0, WALL_T) } else { Vec3::new(WALL_T, y1 - y0, len) };
                let off = if along_x { Vec3::new(offset, 0.0, 0.0) } else { Vec3::new(0.0, 0.0, offset) };
                (Cuboid::from_size(size), centre + off + Vec3::Y * (y0 + y1) / 2.0)
            };
            let full = HALF * 2.0 + WALL_T * 2.0;
            let mut parts = Vec::new();
            if has_door || is_exit {
                let side_len = (full - DOOR_HALF * 2.0) / 2.0;
                let off = DOOR_HALF + side_len / 2.0;
                parts.push(seg(side_len, -off, 0.0, h));
                parts.push(seg(side_len, off, 0.0, h));
                if h > 2.2 {
                    parts.push(seg(DOOR_HALF * 2.0, 0.0, 2.2, h));
                }
            } else {
                parts.push(seg(full, 0.0, 0.0, h));
            }
            for (cuboid, pos) in parts {
                commands.spawn((Mesh3d(meshes.add(cuboid)), MeshMaterial3d(wm.clone()), Transform::from_translation(pos), WorldEntity));
            }
            if has_door || is_exit {
                // door frame posts + an open door leaf (or the glowing exit)
                for k in [-1.0, 1.0] {
                    let (c, p) = seg(0.12, k * (DOOR_HALF + 0.06), 0.0, 2.25);
                    let c = Cuboid::from_size(c.size() + Vec3::new(0.06, 0.0, 0.06));
                    commands.spawn((Mesh3d(meshes.add(c)), MeshMaterial3d(frame_mat.clone()), Transform::from_translation(p), WorldEntity));
                }
                {
                    let leaf = if is_exit { exit_mat.clone() } else { door_mat.clone() };
                    let (c, p) = seg(DOOR_HALF * 2.0, 0.0, 0.0, 2.2);
                    let push = Vec3::new(dir.x, 0.0, dir.y) * (WALL_T * 0.5 + 0.06);
                    let size = if along_x { Vec3::new(c.size().x, c.size().y, 0.06) } else { Vec3::new(0.06, c.size().y, c.size().z) };
                    commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(leaf), Transform::from_translation(p + push), WorldEntity));
                }
                if is_exit {
                    commands.spawn((
                        PointLight { color: Color::srgb(0.3, 1.0, 0.4), intensity: 60_000.0, range: 8.0, ..default() },
                        Transform::from_translation(o + Vec3::new(dir.x, 0.0, dir.y) * (HALF - 0.8) + Vec3::Y * 1.6),
                        WorldEntity,
                    ));
                }
            }
        }
        commands.spawn((
            PointLight { intensity: 320_000.0, range: 18.0, shadow_maps_enabled: true, ..default() },
            Transform::from_translation(o + Vec3::Y * (WALL_H - 0.4)),
            WorldEntity,
        ));
    }

    for (f, furn) in m.furniture.iter().enumerate() {
        spawn_furniture(commands, m, f, furn, disc, meshes, mats);
    }
    for s in 0..2 {
        spawn_spy(commands, s, meshes, mats);
    }
}

fn spawn_furniture(
    commands: &mut Commands,
    m: &Match,
    f: usize,
    furn: &Furniture,
    disc: &DiscAssets,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    let (size, color, tex) = match furn.kind {
        FurnKind::Desk => (Vec3::new(1.6, 0.8, 0.9), Color::srgb(0.45, 0.28, 0.14), "128cc_desk"),
        FurnKind::Bookshelf => (Vec3::new(1.4, 2.0, 0.5), Color::srgb(0.35, 0.2, 0.1), "128cc_bookshelf"),
        FurnKind::FileCabinet => (Vec3::new(0.7, 1.3, 0.7), Color::srgb(0.5, 0.52, 0.55), "128cc_filecabinet"),
        FurnKind::Couch => (Vec3::new(1.8, 0.8, 0.9), Color::srgb(0.6, 0.15, 0.15), "e2_emb_couch"),
        FurnKind::Plant => (Vec3::new(0.6, 1.4, 0.6), Color::srgb(0.2, 0.55, 0.2), "128cc_plant_pot"),
        FurnKind::CoatRack => (Vec3::new(0.4, 1.9, 0.4), Color::srgb(0.3, 0.2, 0.12), "e2_coat_rack"),
        FurnKind::Tv => (Vec3::new(1.1, 1.2, 0.6), Color::srgb(0.15, 0.15, 0.17), "e2_emb_tv_128"),
        FurnKind::Vending => (Vec3::new(1.0, 2.0, 0.8), Color::srgb(0.2, 0.3, 0.7), "e2_emb_venmachine"),
        FurnKind::Copier => (Vec3::new(1.0, 1.1, 0.8), Color::srgb(0.8, 0.78, 0.7), "o2_copymachine"),
        FurnKind::WineCabinet => (Vec3::new(1.0, 1.8, 0.6), Color::srgb(0.4, 0.1, 0.12), "e2_winecab"),
    };
    let mat = tex_mat(mats, disc, tex, color, Vec2::ONE);
    // long side against the nearest wall
    let rot = if furn.pos.x.abs() > furn.pos.y.abs() { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
    let pos = world_pos(m, furn.room, furn.pos) + Vec3::Y * size.y / 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(pos).with_rotation(rot),
        FurnVisual(f, mat),
        WorldEntity,
    ));
}

fn spawn_spy(commands: &mut Commands, s: usize, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) {
    let body = mats.add(StandardMaterial { base_color: SPY_COLORS[s], perceptual_roughness: 0.6, ..default() });
    let club = mats.add(Color::srgb(0.5, 0.33, 0.15));
    commands
        .spawn((Transform::default(), Visibility::default(), SpyRoot(s), WorldEntity))
        .with_children(|p| {
            p.spawn((Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 0.65, 0.0)));
            p.spawn((Mesh3d(meshes.add(Sphere::new(0.27))), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 1.35, 0.0)));
            // the famous pointy nose
            p.spawn((
                Mesh3d(meshes.add(Cone { radius: 0.09, height: 0.6 })),
                MeshMaterial3d(body.clone()),
                Transform::from_xyz(0.0, 1.3, 0.45).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
            p.spawn((Mesh3d(meshes.add(Cylinder::new(0.45, 0.04))), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 1.55, 0.0)));
            p.spawn((Mesh3d(meshes.add(Cylinder::new(0.24, 0.32))), MeshMaterial3d(body.clone()), Transform::from_xyz(0.0, 1.72, 0.0)));
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.06, 0.8))),
                MeshMaterial3d(club),
                Transform::from_xyz(0.35, 0.9, 0.3),
                Visibility::Hidden,
                SpyClub(s),
            ));
        });
}

fn spawn_hud(commands: &mut Commands, status: &str) {
    let font = |px: f32| TextFont::from_font_size(FontSize::Px(px));
    let shadow = TextShadow::default();
    for s in 0..2 {
        let left = if s == 0 { Val::Px(12.0) } else { Val::Percent(50.8) };
        commands.spawn((
            Text::new(""),
            font(18.0),
            TextColor(Color::WHITE),
            shadow,
            Node { position_type: PositionType::Absolute, left, top: Val::Px(12.0), ..default() },
            HudText(s),
        ));
        commands.spawn((
            Text::new(""),
            font(22.0),
            TextColor(Color::srgb(1.0, 0.9, 0.4)),
            shadow,
            Node { position_type: PositionType::Absolute, left, bottom: Val::Px(48.0), ..default() },
            MsgText(s),
        ));
    }
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(84.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font(28.0),
                TextColor(Color::WHITE),
                shadow,
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                TimerText,
            ));
        });
    commands.spawn((
        Text::new(""),
        font(44.0),
        TextColor(Color::srgb(1.0, 0.85, 0.2)),
        shadow,
        Node { position_type: PositionType::Absolute, left: Val::Percent(30.0), top: Val::Percent(42.0), ..default() },
        CenterText,
    ));
    commands.spawn((
        Text::new(format!(
            "WHITE: WASD move (camera-relative)  E search  Q set trap  Tab pick trap  Space attack | BLACK: arrows  Enter  RShift  /  RCtrl | gamepads work too | F2 toggle Black AI  R restart  Esc quit\n{status}"
        )),
        font(13.0),
        TextColor(Color::srgb(0.75, 0.75, 0.75)),
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), bottom: Val::Px(6.0), ..default() },
    ));
    // split-screen divider
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Px(0.0),
            width: Val::Px(3.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
    ));
}

// ---------- per-frame systems ----------

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut game: ResMut<Game>,
    mut commands: Commands,
    world: Query<Entity, With<WorldEntity>>,
    disc: Res<DiscAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::F2) {
        game.ai[1] = !game.ai[1];
    }
    if keys.just_pressed(KeyCode::KeyR) {
        for e in &world {
            commands.entity(e).despawn();
        }
        game.seed = game.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        game.m = Match::new(game.seed);
        game.brains = Default::default();
        spawn_world(&mut commands, &game.m, &disc, &mut meshes, &mut mats);
    }

    type Map = (KeyCode, KeyCode, KeyCode, KeyCode, KeyCode, KeyCode, KeyCode, KeyCode, KeyCode);
    let maps: [Map; 2] = [
        (KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyQ, KeyCode::Space, KeyCode::Tab, KeyCode::KeyF),
        (KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Enter, KeyCode::ShiftRight, KeyCode::ControlRight, KeyCode::Slash, KeyCode::Numpad0),
    ];
    let pads: Vec<&Gamepad> = pads.iter().collect();
    let mut inputs = [SpyInput::default(); 2];
    let mut orbit = [0.0f32; 2];
    for s in 0..2 {
        let (up, down, left, right, search, trap, attack, cycle, attack2) = maps[s];
        let mut i = SpyInput::default();
        let axis = |a: KeyCode, b: KeyCode| keys.pressed(b) as i32 as f32 - keys.pressed(a) as i32 as f32;
        let mut raw = Vec2::new(axis(left, right), axis(down, up));
        i.search = keys.just_pressed(search);
        i.trap = keys.just_pressed(trap);
        i.attack = keys.just_pressed(attack) || keys.just_pressed(attack2);
        i.cycle = keys.just_pressed(cycle);
        if let Some(pad) = pads.get(s) {
            let st = pad.left_stick();
            if st.length() > 0.2 {
                raw = st;
            }
            let dp = pad.dpad();
            if dp.length() > 0.5 {
                raw = dp;
            }
            let rs = pad.right_stick();
            orbit[s] = if rs.x.abs() > 0.2 { rs.x } else { 0.0 };
            i.search |= pad.just_pressed(GamepadButton::West);
            i.trap |= pad.just_pressed(GamepadButton::North);
            i.attack |= pad.just_pressed(GamepadButton::South);
            i.cycle |= pad.just_pressed(GamepadButton::LeftTrigger) || pad.just_pressed(GamepadButton::RightTrigger);
        }
        // stick/keys are relative to this spy's camera
        let yaw = game.cam_yaw[s];
        let fwd = Vec2::new(yaw.sin(), yaw.cos());
        let right = Vec2::new(-fwd.y, fwd.x);
        i.mv = fwd * raw.y + right * raw.x;
        inputs[s] = i;
    }
    game.inputs = inputs;
    game.cam_orbit = orbit;
}

fn step_game(time: Res<Time>, mut game: ResMut<Game>, mut script: ResMut<ScriptRun>) {
    let g = &mut *game;
    let was_over = g.m.over;
    if let Some(sc) = script.0.as_mut() {
        // scripted: fixed step, replays identically every run
        for _ in 0..sc.speed.max(1) {
            if sc.ended {
                break;
            }
            let ov = sc.step(&mut g.m);
            for s in 0..2 {
                g.inputs[s] = match ov[s] {
                    Some(i) => i,
                    None if g.ai[s] => g.brains[s].think(&mut g.m, s, script::DT),
                    None => SpyInput::default(),
                };
            }
            g.m.step(script::DT, g.inputs);
        }
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let speed: u32 = std::env::var("SVS_SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for _ in 0..speed.max(1) {
        for s in 0..2 {
            if g.ai[s] {
                g.inputs[s] = g.brains[s].think(&mut g.m, s, dt);
            }
        }
        g.m.step(dt, g.inputs);
    }
    if g.m.over && !was_over {
        let who = g.m.winner.map(|w| g.m.spies[w].name).unwrap_or("nobody");
        info!("match over: winner={who} time_left={:.0}s deaths={}/{}", g.m.time_left, g.m.spies[0].deaths, g.m.spies[1].deaths);
    }
}

fn sync_spies(
    time: Res<Time>,
    game: Res<Game>,
    mut roots: Query<(&SpyRoot, &mut Transform, &mut Visibility)>,
    mut clubs: Query<(&SpyClub, &mut Visibility), Without<SpyRoot>>,
) {
    let m = &game.m;
    let t = time.elapsed_secs();
    for (SpyRoot(s), mut tf, mut vis) in &mut roots {
        let spy = &m.spies[*s];
        let mut p = world_pos(m, spy.room, spy.pos);
        let yaw = spy.facing.x.atan2(spy.facing.y);
        let mut rot = Quat::from_rotation_y(yaw);
        let mut scale = Vec3::ONE;
        match spy.state {
            SpyState::Dead { t: left } => {
                // float up like the angel in the original
                p.y += (DEAD_TIME - left) * 0.8;
                rot *= Quat::from_rotation_y((DEAD_TIME - left) * 4.0);
                scale = Vec3::splat((left / DEAD_TIME).max(0.2));
            }
            SpyState::Stunned { .. } => rot *= Quat::from_rotation_z((t * 14.0).sin() * 0.25),
            SpyState::Searching { .. } => p.y += (t * 25.0).sin().abs() * 0.08,
            _ => {}
        }
        if spy.flash_t > 0.0 {
            scale *= 1.0 + spy.flash_t;
        }
        *vis = if spy.state == SpyState::Escaped { Visibility::Hidden } else { Visibility::Inherited };
        *tf = Transform { translation: p, rotation: rot, scale };
    }
    for (SpyClub(s), mut vis) in &mut clubs {
        *vis = if m.spies[*s].swing_t > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn sync_furniture(game: Res<Game>, q: Query<&FurnVisual>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let near: Vec<usize> = (0..2)
        .filter(|&s| !game.ai[s] && game.m.spies[s].can_act())
        .filter_map(|s| game.m.reachable_furniture(s))
        .collect();
    for FurnVisual(f, h) in &q {
        if let Some(mut mat) = mats.get_mut(h) {
            let glow = if near.contains(f) { LinearRgba::rgb(0.25, 0.22, 0.05) } else { LinearRgba::BLACK };
            if mat.emissive != glow {
                mat.emissive = glow;
            }
        }
    }
}

fn sync_cameras(
    time: Res<Time>,
    mut game: ResMut<Game>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cams: Query<(&SpyCam, &mut Camera, &mut Transform)>,
) {
    let Ok(win) = window.single() else { return };
    let (w, h) = (win.physical_width(), win.physical_height());
    if w < 4 || h < 4 {
        return;
    }
    for (SpyCam(s), mut cam, mut tf) in &mut cams {
        let half = w / 2;
        cam.viewport = Some(Viewport {
            physical_position: UVec2::new(if *s == 0 { 0 } else { half }, 0),
            physical_size: UVec2::new(half, h),
            ..default()
        });
        let s = *s;
        let dt = time.delta_secs();
        let spy = &game.m.spies[s];
        let face_yaw = spy.facing.x.atan2(spy.facing.y);
        let (room, pos) = (spy.room, spy.pos);
        if game.cam_room[s] != room {
            // new room: snap behind the spy
            game.cam_room[s] = room;
            game.cam_yaw[s] = face_yaw;
        } else {
            // swing round behind the spy, plus manual orbit from the right stick
            let diff = (face_yaw - game.cam_yaw[s] + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            let follow = if game.inputs[s].mv.length_squared() > 0.01 { 2.2 } else { 0.0 };
            game.cam_yaw[s] += diff * (follow * dt).min(1.0) + game.cam_orbit[s] * 2.5 * dt;
        }
        let lim = HALF - 0.35;
        let space_behind = |yaw: f32| {
            let fwd = Vec2::new(yaw.sin(), yaw.cos());
            let mut back = 4.2f32;
            for (axis, p) in [(fwd.x, pos.x), (fwd.y, pos.y)] {
                if axis.abs() > 1e-3 {
                    let room_left = if axis > 0.0 { p + lim } else { lim - p };
                    back = back.min(room_left / axis.abs());
                }
            }
            back
        };
        // backed against a wall: swing round so the camera looks from the middle of the room
        if space_behind(game.cam_yaw[s]) < 2.6 && pos.length() > 0.5 {
            let out_yaw = pos.x.atan2(pos.y);
            let diff = (out_yaw - game.cam_yaw[s] + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            game.cam_yaw[s] += diff * (4.0 * dt).min(1.0);
        }
        let yaw = game.cam_yaw[s];
        let fwd = Vec2::new(yaw.sin(), yaw.cos());
        // stay inside the room so walls never block the view
        // stay inside the room: if there's no space behind the spy, rise and look down instead
        let back = space_behind(yaw).max(0.6);
        let squeeze = (4.2 - back) / 3.6; // 0 = full distance, 1 = right on top
        let cam = pos - fwd * back;
        let eye = world_pos(&game.m, room, cam) + Vec3::Y * (2.2 + squeeze * 0.9);
        let target = world_pos(&game.m, room, pos + fwd * (1.6 + squeeze * 0.8)) + Vec3::Y * (1.1 - squeeze * 0.6);
        *tf = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

fn hud(
    game: Res<Game>,
    mut q: ParamSet<(
        Query<(&HudText, &mut Text)>,
        Query<(&MsgText, &mut Text)>,
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<CenterText>>,
    )>,
) {
    let m = &game.m;
    for (HudText(s), mut text) in &mut q.p0() {
        let spy = &m.spies[*s];
        let items: Vec<String> = ALL_ITEMS
            .iter()
            .map(|i| format!("[{}] {}", if spy.items.contains(i) { "X" } else { " " }, i.name()))
            .collect();
        let traps: Vec<String> = FURNITURE_TRAPS
            .iter()
            .enumerate()
            .map(|(k, t)| format!("{}{} x{}", if spy.sel == k { ">" } else { " " }, t.name(), spy.traps[t.idx()]))
            .collect();
        let state = match spy.state {
            SpyState::Dead { t } => format!("DEAD - back in {:.0}s", t.ceil()),
            SpyState::Stunned { .. } => "STUNNED".into(),
            SpyState::Searching { .. } => "searching...".into(),
            SpyState::Escaped => "ESCAPED!".into(),
            SpyState::Alive => format!("health {}", "#".repeat((HITS_TO_KILL - spy.hits) as usize)),
        };
        let who = if game.ai[*s] { " (AI)" } else { "" };
        **text = format!(
            "{}{}  {}\n{}\nTraps: {}  BUCKET x{}",
            spy.name,
            who,
            state,
            items.join("  "),
            traps.join(" "),
            spy.traps[Trap::Bucket.idx()]
        );
    }
    for (MsgText(s), mut text) in &mut q.p1() {
        let spy = &m.spies[*s];
        **text = if spy.message_t > 0.0 { spy.message.clone() } else { String::new() };
    }
    for mut text in &mut q.p2() {
        let t = m.time_left.ceil() as u32;
        **text = format!("{}:{:02}", t / 60, t % 60);
    }
    for mut text in &mut q.p3() {
        **text = match (m.over, m.winner) {
            (true, Some(w)) => format!("{} ESCAPES!\n   press R for a rematch", m.spies[w].name),
            (true, None) => "TIME UP - nobody escaped!\n   press R for a rematch".into(),
            _ => String::new(),
        };
    }
}

fn play_sounds(mut commands: Commands, mut game: ResMut<Game>, disc: Res<DiscAssets>) {
    for name in game.m.sounds.drain(..) {
        if let Some(h) = disc.sounds.get(name) {
            commands.spawn((AudioPlayer::new(h.clone()), PlaybackSettings::DESPAWN));
        }
    }
}

/// SVS_SCREENSHOT=<file.png>: save one frame after SVS_SHOT_AFTER seconds, then quit.
fn auto_screenshot(time: Res<Time>, mut commands: Commands, mut done: Local<u8>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("SVS_SCREENSHOT") else { return };
    let after: f32 = std::env::var("SVS_SHOT_AFTER").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let t = time.elapsed_secs();
    if *done == 0 && t > after {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *done = 1;
    } else if *done == 1 && t > after + 1.5 {
        exit.write(AppExit::Success);
    }
}

/// Test scripts: take requested screenshots, and on `end` report and exit with pass/fail.
fn script_outputs(
    time: Res<Time>,
    mut script: ResMut<ScriptRun>,
    mut commands: Commands,
    mut ended_at: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(sc) = script.0.as_mut() else { return };
    for f in sc.screenshots.drain(..) {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot_path(&f)));
    }
    if sc.ended && ended_at.is_none() {
        *ended_at = Some(time.elapsed_secs());
    }
    // give screenshots a moment to land on disk
    if let Some(t) = *ended_at {
        if time.elapsed_secs() > t + 0.8 {
            let ok = sc.failures.is_empty();
            println!("{} {}: {} checks passed, {} failed", if ok { "PASS" } else { "FAIL" }, sc.name, sc.passes, sc.failures.len());
            exit.write(if ok { AppExit::Success } else { AppExit::error() });
            *ended_at = Some(f32::INFINITY);
        }
    }
}

/// Screenshot file names resolve against $SVS_SHOT_DIR (default: current directory).
pub fn shot_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::var("SVS_SHOT_DIR").unwrap_or_else(|_| ".".into());
    let p = std::path::Path::new(name);
    let file = p.file_name().map(|f| f.to_owned()).unwrap_or_else(|| name.into());
    std::path::Path::new(&dir).join(file)
}
