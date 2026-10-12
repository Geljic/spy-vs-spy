//! Classic-mode rules as plain data + a step function, independent of rendering.
//! Keeping all state here means netcode can later sync `Match` + per-spy inputs.

use bevy::math::Vec2;
use std::collections::VecDeque;

pub const GRID: usize = 3;
pub const HALF: f32 = 5.0; // room interior is [-HALF, HALF] on x and z
pub const WALL_PAD: f32 = 0.45;
pub const DOOR_HALF: f32 = 0.9;
pub const SPY_RADIUS: f32 = 0.45;
pub const FURN_RADIUS: f32 = 0.8;
pub const SPEED: f32 = 4.5;
pub const SEARCH_TIME: f32 = 0.7;
pub const DEAD_TIME: f32 = 4.0;
pub const MATCH_TIME: f32 = 300.0;
pub const HITS_TO_KILL: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Passport,
    Key,
    Money,
    Plans,
}
pub const ALL_ITEMS: [Item; 4] = [Item::Passport, Item::Key, Item::Money, Item::Plans];

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Passport => "PASSPORT",
            Item::Key => "KEY",
            Item::Money => "MONEY",
            Item::Plans => "SECRET PLANS",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trap {
    Bomb,
    Spring,
    Bucket,
}
pub const FURNITURE_TRAPS: [Trap; 2] = [Trap::Bomb, Trap::Spring];

impl Trap {
    pub fn name(self) -> &'static str {
        match self {
            Trap::Bomb => "BOMB",
            Trap::Spring => "SPRING",
            Trap::Bucket => "BUCKET",
        }
    }
    pub fn idx(self) -> usize {
        self as usize
    }
}

/// Wall sides: 0 = north (-z), 1 = east (+x), 2 = south (+z), 3 = west (-x)
pub fn side_dir(side: usize) -> Vec2 {
    [Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0)][side]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FurnKind {
    Desk,
    Bookshelf,
    FileCabinet,
    Couch,
    Plant,
    CoatRack,
    Tv,
    Vending,
    Copier,
    WineCabinet,
}
pub const FURN_KINDS: [FurnKind; 10] = [
    FurnKind::Desk,
    FurnKind::Bookshelf,
    FurnKind::FileCabinet,
    FurnKind::Couch,
    FurnKind::Plant,
    FurnKind::CoatRack,
    FurnKind::Tv,
    FurnKind::Vending,
    FurnKind::Copier,
    FurnKind::WineCabinet,
];

impl FurnKind {
    pub fn name(self) -> &'static str {
        match self {
            FurnKind::Desk => "desk",
            FurnKind::Bookshelf => "bookshelf",
            FurnKind::FileCabinet => "file cabinet",
            FurnKind::Couch => "couch",
            FurnKind::Plant => "plant",
            FurnKind::CoatRack => "coat rack",
            FurnKind::Tv => "TV",
            FurnKind::Vending => "vending machine",
            FurnKind::Copier => "copier",
            FurnKind::WineCabinet => "wine cabinet",
        }
    }
}

pub struct Furniture {
    pub room: usize,
    pub pos: Vec2,
    pub kind: FurnKind,
    pub items: Vec<Item>,
    pub trap: Option<(Trap, usize)>,
}

pub struct Door {
    pub rooms: [(usize, usize); 2], // (room, side) for both ends
    pub trap: Option<(Trap, usize)>,
}

pub struct Room {
    pub gx: usize,
    pub gy: usize,
    pub doors: [Option<usize>; 4],
    pub exit_side: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpyState {
    Alive,
    Searching { t: f32, furn: usize },
    Stunned { t: f32 },
    Dead { t: f32 },
    Escaped,
}

pub struct Spy {
    pub name: &'static str,
    pub room: usize,
    pub pos: Vec2,
    pub facing: Vec2,
    pub items: Vec<Item>,
    pub hits: u8,
    pub state: SpyState,
    pub traps: [u8; 3],
    pub sel: usize, // index into FURNITURE_TRAPS
    pub attack_cd: f32,
    pub swing_t: f32,  // >0 while the attack animation plays
    pub flash_t: f32,  // >0 after being hit
    pub message: String,
    pub message_t: f32,
    pub deaths: u32,
}

impl Spy {
    fn say(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_t = 2.5;
    }
    pub fn can_act(&self) -> bool {
        matches!(self.state, SpyState::Alive)
    }
}

#[derive(Default, Clone, Copy)]
pub struct SpyInput {
    pub mv: Vec2,
    pub search: bool,
    pub trap: bool,
    pub attack: bool,
    pub cycle: bool,
}

pub struct Match {
    pub rooms: Vec<Room>,
    pub doors: Vec<Door>,
    pub furniture: Vec<Furniture>,
    pub spies: [Spy; 2],
    pub time_left: f32,
    pub winner: Option<usize>,
    pub over: bool,
    pub sounds: Vec<&'static str>,
    pub exit_room: usize,
    rng: u64,
}

impl Match {
    pub fn rand(&mut self) -> u32 {
        // xorshift64*: deterministic from the seed, which netcode will want
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32
    }
    pub fn rand_range(&mut self, n: usize) -> usize {
        (self.rand() as usize) % n.max(1)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        (self.rand() as f32 / u32::MAX as f32) < p
    }

    pub fn room_index(gx: usize, gy: usize) -> usize {
        gy * GRID + gx
    }

    pub fn new(seed: u64) -> Match {
        let mut m = Match {
            rooms: Vec::new(),
            doors: Vec::new(),
            furniture: Vec::new(),
            spies: [new_spy("WHITE SPY", 0), new_spy("BLACK SPY", GRID * GRID - 1)],
            time_left: MATCH_TIME,
            winner: None,
            over: false,
            sounds: Vec::new(),
            exit_room: 0,
            rng: seed.max(1),
        };
        for gy in 0..GRID {
            for gx in 0..GRID {
                m.rooms.push(Room { gx, gy, doors: [None; 4], exit_side: None });
            }
        }
        // every neighbouring pair is connected
        for gy in 0..GRID {
            for gx in 0..GRID {
                let a = Self::room_index(gx, gy);
                if gx + 1 < GRID {
                    m.link(a, 1, Self::room_index(gx + 1, gy), 3);
                }
                if gy + 1 < GRID {
                    m.link(a, 2, Self::room_index(gx, gy + 1), 0);
                }
            }
        }
        // exit on an outer wall of an edge room that isn't a start room
        // (grid x, grid y, outer wall side) for the middle room of each edge
        let candidates = [(1, 0, 0), (2, 1, 1), (1, 2, 2), (0, 1, 3)];
        let (gx, gy, exit_side) = candidates[m.rand_range(candidates.len())];
        let exit_room = Self::room_index(gx, gy);
        m.rooms[exit_room].exit_side = Some(exit_side);
        m.exit_room = exit_room;

        // furniture: 3-4 pieces per room in free slots along the walls
        let slots = [
            Vec2::new(-3.9, -3.9),
            Vec2::new(3.9, -3.9),
            Vec2::new(-3.9, 3.9),
            Vec2::new(3.9, 3.9),
            Vec2::new(-2.0, -4.1),
            Vec2::new(2.0, -4.1),
            Vec2::new(4.1, -2.0),
            Vec2::new(4.1, 2.0),
            Vec2::new(-4.1, -2.0),
            Vec2::new(-4.1, 2.0),
            Vec2::new(-2.0, 4.1),
            Vec2::new(2.0, 4.1),
        ];
        for r in 0..m.rooms.len() {
            let n = 3 + m.rand_range(2);
            let mut free: Vec<Vec2> = slots.to_vec();
            for _ in 0..n {
                let k = m.rand_range(free.len());
                let pos = free.swap_remove(k);
                let kind = FURN_KINDS[m.rand_range(FURN_KINDS.len())];
                m.furniture.push(Furniture { room: r, pos, kind, items: Vec::new(), trap: None });
            }
        }
        // hide the items away from the start rooms
        let start_rooms = [m.spies[0].room, m.spies[1].room];
        for item in ALL_ITEMS {
            loop {
                let f = m.rand_range(m.furniture.len());
                if !start_rooms.contains(&m.furniture[f].room) && m.furniture[f].items.is_empty() {
                    m.furniture[f].items.push(item);
                    break;
                }
            }
        }
        m
    }

    fn link(&mut self, a: usize, sa: usize, b: usize, sb: usize) {
        let d = self.doors.len();
        self.doors.push(Door { rooms: [(a, sa), (b, sb)], trap: None });
        self.rooms[a].doors[sa] = Some(d);
        self.rooms[b].doors[sb] = Some(d);
    }

    pub fn furniture_in(&self, room: usize) -> impl Iterator<Item = usize> + '_ {
        (0..self.furniture.len()).filter(move |&f| self.furniture[f].room == room)
    }

    /// Nearest furniture within reach of a spy.
    pub fn reachable_furniture(&self, s: usize) -> Option<usize> {
        let spy = &self.spies[s];
        self.furniture_in(spy.room)
            .map(|f| (f, self.furniture[f].pos.distance(spy.pos)))
            .filter(|&(_, d)| d < FURN_RADIUS + SPY_RADIUS + 0.7)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(f, _)| f)
    }

    /// Door (index, side) a spy is standing in front of.
    pub fn reachable_door(&self, s: usize) -> Option<(usize, usize)> {
        let spy = &self.spies[s];
        let room = &self.rooms[spy.room];
        (0..4).find_map(|side| {
            let d = room.doors[side]?;
            let at = side_dir(side) * HALF;
            (spy.pos.distance(at) < 1.8).then_some((d, side))
        })
    }

    pub fn step(&mut self, dt: f32, inputs: [SpyInput; 2]) {
        if self.over {
            return;
        }
        self.time_left -= dt;
        if self.time_left <= 0.0 {
            self.time_left = 0.0;
            self.over = true;
            self.sounds.push("sm_menu_invalid");
            return;
        }
        for s in 0..2 {
            self.step_spy(s, dt, inputs[s]);
        }
        self.resolve_spy_overlap();
    }

    fn step_spy(&mut self, s: usize, dt: f32, input: SpyInput) {
        {
            let spy = &mut self.spies[s];
            spy.attack_cd = (spy.attack_cd - dt).max(0.0);
            spy.swing_t = (spy.swing_t - dt).max(0.0);
            spy.flash_t = (spy.flash_t - dt).max(0.0);
            spy.message_t = (spy.message_t - dt).max(0.0);
        }
        match self.spies[s].state {
            SpyState::Escaped => return,
            SpyState::Dead { t } => {
                let t = t - dt;
                if t <= 0.0 {
                    let spy = &mut self.spies[s];
                    spy.state = SpyState::Alive;
                    spy.hits = 0;
                    spy.say("Back in action!");
                    self.sounds.push("sm_spy_healed");
                } else {
                    self.spies[s].state = SpyState::Dead { t };
                }
                return;
            }
            SpyState::Stunned { t } => {
                let t = t - dt;
                self.spies[s].state = if t <= 0.0 { SpyState::Alive } else { SpyState::Stunned { t } };
                return;
            }
            SpyState::Searching { t, furn } => {
                let t = t - dt;
                if t <= 0.0 {
                    self.spies[s].state = SpyState::Alive;
                    self.finish_search(s, furn);
                } else {
                    self.spies[s].state = SpyState::Searching { t, furn };
                }
                return;
            }
            SpyState::Alive => {}
        }

        if input.cycle {
            let spy = &mut self.spies[s];
            spy.sel = (spy.sel + 1) % FURNITURE_TRAPS.len();
            let t = FURNITURE_TRAPS[spy.sel];
            spy.say(format!("Selected {}", t.name()));
            self.sounds.push("sm_gen_select");
        }

        // movement + facing
        let mv = input.mv.clamp_length_max(1.0);
        if mv.length_squared() > 0.01 {
            self.spies[s].facing = mv.normalize();
        }
        let room = self.spies[s].room;
        let mut pos = self.spies[s].pos + mv * SPEED * dt;
        // walls, except where a door lets you through
        let lim = HALF - WALL_PAD;
        for side in 0..4 {
            let dir = side_dir(side);
            let along = pos.dot(dir);
            let across = (pos - dir * along).length();
            let open = self.rooms[room].doors[side].is_some() || self.rooms[room].exit_side == Some(side);
            if along > lim && !(open && across < DOOR_HALF - SPY_RADIUS + 0.35) {
                pos -= dir * (along - lim);
            }
        }
        // furniture
        for f in self.furniture_in(room).collect::<Vec<_>>() {
            let fp = self.furniture[f].pos;
            let d = pos - fp;
            let min = FURN_RADIUS + SPY_RADIUS;
            if d.length() < min && d.length() > 1e-4 {
                pos = fp + d.normalize() * min;
            }
        }
        self.spies[s].pos = pos;

        // walked through a doorway?
        for side in 0..4 {
            let dir = side_dir(side);
            if pos.dot(dir) <= HALF + 0.05 {
                continue;
            }
            if self.rooms[room].exit_side == Some(side) {
                self.try_exit(s, side);
                return;
            }
            if let Some(d) = self.rooms[room].doors[side] {
                self.go_through(s, d, room);
                return;
            }
        }

        if input.attack && self.spies[s].attack_cd <= 0.0 {
            self.attack(s);
        }
        if input.search {
            if let Some(f) = self.reachable_furniture(s) {
                self.spies[s].state = SpyState::Searching { t: SEARCH_TIME, furn: f };
                self.spies[s].facing = (self.furniture[f].pos - self.spies[s].pos).normalize_or_zero();
                self.sounds.push("sm_menu_press");
            }
        }
        if input.trap {
            self.set_trap(s);
        }
    }

    fn try_exit(&mut self, s: usize, side: usize) {
        let spy = &mut self.spies[s];
        if spy.items.len() == ALL_ITEMS.len() {
            spy.state = SpyState::Escaped;
            self.winner = Some(s);
            self.over = true;
            self.sounds.push("sm_get_bux");
        } else {
            let missing = ALL_ITEMS.len() - spy.items.len();
            spy.say(format!("The exit! Still need {missing} item(s)."));
            spy.pos -= side_dir(side) * 0.6;
            self.sounds.push("sm_menu_invalid");
        }
    }

    fn go_through(&mut self, s: usize, d: usize, from: usize) {
        let door = &self.doors[d];
        let (to, to_side) = if door.rooms[0].0 == from { door.rooms[1] } else { door.rooms[0] };
        let trap = door.trap;
        let spy = &mut self.spies[s];
        spy.room = to;
        spy.pos = side_dir(to_side) * (HALF - 1.0);
        spy.facing = -side_dir(to_side);
        self.sounds.push("sm_footstep");
        if let Some((t, owner)) = trap {
            if owner != s {
                self.doors[d].trap = None;
                self.spring_trap(s, t, None);
            }
        }
    }

    fn set_trap(&mut self, s: usize) {
        // a doorway takes a bucket, furniture takes the selected trap
        if let Some((d, _)) = self.reachable_door(s) {
            let spy = &mut self.spies[s];
            if spy.traps[Trap::Bucket.idx()] == 0 {
                spy.say("No buckets left!");
                return;
            }
            if self.doors[d].trap.is_some() {
                spy.say("This door is already rigged.");
                return;
            }
            spy.traps[Trap::Bucket.idx()] -= 1;
            spy.say("Bucket of water set on the door.");
            self.doors[d].trap = Some((Trap::Bucket, s));
            self.sounds.push("sm_spy_giggle");
            return;
        }
        if let Some(f) = self.reachable_furniture(s) {
            let t = FURNITURE_TRAPS[self.spies[s].sel];
            let spy = &mut self.spies[s];
            if spy.traps[t.idx()] == 0 {
                spy.say(format!("No {}s left!", t.name()));
                return;
            }
            if self.furniture[f].trap.is_some() {
                spy.say("Something is already hidden here...");
                return;
            }
            spy.traps[t.idx()] -= 1;
            spy.say(format!("{} hidden in the {}.", t.name(), self.furniture[f].kind.name()));
            self.furniture[f].trap = Some((t, s));
            self.sounds.push("sm_spy_giggle");
            return;
        }
        self.spies[s].say("Get next to furniture or a door to set a trap.");
    }

    fn finish_search(&mut self, s: usize, f: usize) {
        if let Some((t, owner)) = self.furniture[f].trap {
            if owner != s {
                self.furniture[f].trap = None;
                self.spring_trap(s, t, Some(f));
                return;
            }
        }
        let found: Vec<Item> = self.furniture[f].items.drain(..).collect();
        let kind = self.furniture[f].kind.name();
        let spy = &mut self.spies[s];
        if found.is_empty() {
            spy.say(format!("Nothing in the {kind}."));
        } else {
            let names: Vec<&str> = found.iter().map(|i| i.name()).collect();
            spy.say(format!("Found the {}!", names.join(" + ")));
            spy.items.extend(found);
            self.sounds.push("sm_get_bux");
        }
    }

    fn spring_trap(&mut self, s: usize, t: Trap, furn: Option<usize>) {
        match t {
            Trap::Bomb => {
                self.sounds.push("sm_explosion1");
                self.kill(s, "BOOM! A bomb!");
            }
            Trap::Spring => {
                self.sounds.push("sm_spy_fallbig");
                // lose one item back into the furniture
                let spy = &mut self.spies[s];
                if let (Some(f), Some(item)) = (furn, spy.items.pop()) {
                    self.furniture[f].items.push(item);
                    spy.say(format!("SPRING! Dropped the {}.", item.name()));
                } else {
                    spy.say("SPRING! Boing!");
                }
                spy.state = SpyState::Stunned { t: 2.0 };
            }
            Trap::Bucket => {
                self.sounds.push("spy_shocked");
                let spy = &mut self.spies[s];
                spy.say("SPLASH! A bucket of water!");
                spy.state = SpyState::Stunned { t: 3.0 };
            }
        }
    }

    fn attack(&mut self, s: usize) {
        let o = 1 - s;
        self.spies[s].attack_cd = 0.5;
        self.spies[s].swing_t = 0.25;
        self.sounds.push("sm_club_swing");
        let (a, b) = (&self.spies[s], &self.spies[o]);
        if a.room != b.room || matches!(b.state, SpyState::Dead { .. } | SpyState::Escaped) {
            return;
        }
        let to = b.pos - a.pos;
        if to.length() > 1.6 || to.normalize_or_zero().dot(a.facing) < 0.3 {
            return;
        }
        self.sounds.push("sm_club_hit");
        let knock = to.normalize_or_zero() * 0.8;
        let victim = &mut self.spies[o];
        victim.hits += 1;
        victim.flash_t = 0.3;
        victim.pos += knock;
        if matches!(victim.state, SpyState::Searching { .. }) {
            victim.state = SpyState::Alive;
        }
        if victim.hits >= HITS_TO_KILL {
            self.kill(o, "Knocked out!");
        }
    }

    /// Death: drop everything into this room's furniture, come back after a while.
    fn kill(&mut self, s: usize, why: &str) {
        self.sounds.push("sm_spy_death");
        let room = self.spies[s].room;
        let items: Vec<Item> = self.spies[s].items.drain(..).collect();
        let furn: Vec<usize> = self.furniture_in(room).collect();
        for item in items {
            let f = furn[self.rand_range(furn.len())];
            self.furniture[f].items.push(item);
        }
        let spy = &mut self.spies[s];
        spy.state = SpyState::Dead { t: DEAD_TIME };
        spy.deaths += 1;
        spy.say(format!("{why} Items scattered in this room."));
    }

    fn resolve_spy_overlap(&mut self) {
        let (a, b) = (&self.spies[0], &self.spies[1]);
        if a.room != b.room
            || matches!(a.state, SpyState::Dead { .. })
            || matches!(b.state, SpyState::Dead { .. })
        {
            return;
        }
        let d = b.pos - a.pos;
        let min = SPY_RADIUS * 2.0;
        if d.length() < min && d.length() > 1e-4 {
            let push = d.normalize() * (min - d.length()) * 0.5;
            self.spies[0].pos -= push;
            self.spies[1].pos += push;
        }
    }

    /// Next room to walk into on the shortest path from `from` to `to`.
    pub fn next_room_towards(&self, from: usize, to: usize) -> Option<(usize, usize)> {
        if from == to {
            return None;
        }
        let mut prev = vec![usize::MAX; self.rooms.len()];
        let mut q = VecDeque::from([from]);
        prev[from] = from;
        while let Some(r) = q.pop_front() {
            for side in 0..4 {
                if let Some(d) = self.rooms[r].doors[side] {
                    let door = &self.doors[d];
                    let n = if door.rooms[0].0 == r { door.rooms[1].0 } else { door.rooms[0].0 };
                    if prev[n] == usize::MAX {
                        prev[n] = r;
                        q.push_back(n);
                    }
                }
            }
        }
        let mut cur = to;
        while prev[cur] != from {
            if prev[cur] == usize::MAX {
                return None;
            }
            cur = prev[cur];
        }
        let side = (0..4).find(|&side| {
            self.rooms[from].doors[side].is_some_and(|d| {
                let door = &self.doors[d];
                door.rooms[0].0 == cur || door.rooms[1].0 == cur
            })
        })?;
        Some((cur, side))
    }
}

fn new_spy(name: &'static str, room: usize) -> Spy {
    Spy {
        name,
        room,
        pos: Vec2::ZERO,
        facing: Vec2::new(0.0, 1.0),
        items: Vec::new(),
        hits: 0,
        state: SpyState::Alive,
        traps: [3, 3, 3],
        sel: 0,
        attack_cd: 0.0,
        swing_t: 0.0,
        flash_t: 0.0,
        message: String::new(),
        message_t: 0.0,
        deaths: 0,
    }
}
