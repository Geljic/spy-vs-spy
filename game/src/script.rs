//! Test scripts: scripted inputs drive the game to a known end point, then check it.
//!
//! Run: SVS_SCRIPT=tests/foo.script spy-vs-spy   (exit code 0 = pass, 1 = fail)
//! The game runs on a fixed 1/60 s step with a fixed seed, so a script always replays the same.
//!
//! Lines (`#` comments). Times are seconds of game time.
//!   seed <n>                         map layout (default 1)
//!   speed <n>                        game steps per rendered frame (default 4)
//!   ai <white|black> <on|off>
//!   at <t> place <spy> furniture <k> one of the spy's room furniture, k = 0.. (stands next to it)
//!   at <t> place <spy> door <north|east|south|west>
//!   at <t> place <spy> exit                standing at the exit door, facing it
//!   at <t> move <spy> forward <secs>        along the way the spy is facing
//!   at <t> expect over                      match has finished
//!   at <t> place <spy> room <r> <x> <z>
//!   at <t> hide <ITEM> furniture <k> of <spy>  moves the item into furniture k of that spy's room
//!   at <t> give <spy> <ITEM|all>
//!   at <t> move <spy> <north|east|south|west> <secs>    world-space direction
//!   at <t> press <spy> <search|trap|attack|cycle>
//!   at <t> expect <spy> room <r>
//!   at <t> expect <spy> has <ITEM>
//!   at <t> expect <spy> state <alive|dead|stunned|escaped>
//!   at <t> expect winner <white|black|none>
//!   at <t> screenshot <file.png>
//!   at <t> end

use crate::rules::*;
use bevy::math::Vec2;

pub const DT: f32 = 1.0 / 60.0;

#[derive(Clone, Debug)]
enum Cmd {
    PlaceFurniture(usize, usize),
    PlaceDoor(usize, usize),
    PlaceRoom(usize, usize, Vec2),
    PlaceExit(usize),
    Forward(usize, f32),
    ExpectOver,
    Hide(Item, usize, usize), // item, room-of-spy, furniture k
    Give(usize, Option<Item>),
    Move(usize, Vec2, f32),
    Press(usize, &'static str),
    ExpectRoom(usize, usize),
    ExpectHas(usize, Item),
    ExpectState(usize, String),
    ExpectWinner(Option<usize>),
    Screenshot(String),
    End,
}

pub struct Script {
    pub name: String,
    pub seed: u64,
    pub speed: u32,
    pub ai: [Option<bool>; 2],
    cmds: Vec<(f32, Cmd, String)>,
    next: usize,
    pub t: f32,
    holds: Vec<(usize, Vec2, f32)>, // spy, dir, until
    pub failures: Vec<String>,
    pub passes: usize,
    pub screenshots: Vec<String>,
    pub ended: bool,
}

fn spy_idx(s: &str) -> Result<usize, String> {
    match s {
        "white" => Ok(0),
        "black" => Ok(1),
        _ => Err(format!("unknown spy '{s}'")),
    }
}
fn item(s: &str) -> Result<Item, String> {
    ALL_ITEMS
        .into_iter()
        .find(|i| i.name().split(' ').last().unwrap().eq_ignore_ascii_case(s) || i.name().eq_ignore_ascii_case(s))
        .ok_or(format!("unknown item '{s}'"))
}
fn side(s: &str) -> Result<usize, String> {
    ["north", "east", "south", "west"].iter().position(|&x| x == s).ok_or(format!("unknown side '{s}'"))
}

impl Script {
    pub fn load(path: &str) -> Result<Script, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let mut sc = Script {
            name: path.to_string(),
            seed: 1,
            speed: 4,
            ai: [None, None],
            cmds: Vec::new(),
            next: 0,
            t: 0.0,
            holds: Vec::new(),
            failures: Vec::new(),
            passes: 0,
            screenshots: Vec::new(),
            ended: false,
        };
        for (ln, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let w: Vec<&str> = line.split_whitespace().collect();
            let err = |e: String| format!("{path}:{}: {e}", ln + 1);
            let num = |s: &str| s.parse::<f32>().map_err(|_| err(format!("bad number '{s}'")));
            match w[0] {
                "seed" => sc.seed = w[1].parse().map_err(|_| err("bad seed".into()))?,
                "speed" => sc.speed = w[1].parse().map_err(|_| err("bad speed".into()))?,
                "ai" => sc.ai[spy_idx(w[1]).map_err(err)?] = Some(w[2] == "on"),
                "at" => {
                    let t = num(w[1])?;
                    let a = &w[2..];
                    let cmd = match a {
                        ["place", s, "furniture", k] => Cmd::PlaceFurniture(spy_idx(s).map_err(err)?, k.parse().map_err(|_| err("bad k".into()))?),
                        ["place", s, "exit"] => Cmd::PlaceExit(spy_idx(s).map_err(err)?),
                        ["move", s, "forward", secs] => Cmd::Forward(spy_idx(s).map_err(err)?, num(secs)?),
                        ["expect", "over"] => Cmd::ExpectOver,
                        ["place", s, "door", d] => Cmd::PlaceDoor(spy_idx(s).map_err(err)?, side(d).map_err(err)?),
                        ["place", s, "room", r, x, z] => Cmd::PlaceRoom(spy_idx(s).map_err(err)?, r.parse().map_err(|_| err("bad room".into()))?, Vec2::new(num(x)?, num(z)?)),
                        ["hide", i, "furniture", k, "of", s] => Cmd::Hide(item(i).map_err(err)?, spy_idx(s).map_err(err)?, k.parse().map_err(|_| err("bad k".into()))?),
                        ["give", s, "all"] => Cmd::Give(spy_idx(s).map_err(err)?, None),
                        ["give", s, i] => Cmd::Give(spy_idx(s).map_err(err)?, Some(item(i).map_err(err)?)),
                        ["move", s, d, secs] => Cmd::Move(spy_idx(s).map_err(err)?, side_dir(side(d).map_err(err)?), num(secs)?),
                        ["press", s, b] => {
                            let b: &'static str = match *b {
                                "search" => "search",
                                "trap" => "trap",
                                "attack" => "attack",
                                "cycle" => "cycle",
                                _ => return Err(err(format!("unknown button '{b}'"))),
                            };
                            Cmd::Press(spy_idx(s).map_err(err)?, b)
                        }
                        ["expect", "winner", who] => Cmd::ExpectWinner(if *who == "none" { None } else { Some(spy_idx(who).map_err(err)?) }),
                        ["expect", s, "room", r] => Cmd::ExpectRoom(spy_idx(s).map_err(err)?, r.parse().map_err(|_| err("bad room".into()))?),
                        ["expect", s, "has", i] => Cmd::ExpectHas(spy_idx(s).map_err(err)?, item(i).map_err(err)?),
                        ["expect", s, "state", st] => Cmd::ExpectState(spy_idx(s).map_err(err)?, st.to_string()),
                        ["screenshot", f] => Cmd::Screenshot(f.to_string()),
                        ["end"] => Cmd::End,
                        _ => return Err(err(format!("can't parse: {line}"))),
                    };
                    sc.cmds.push((t, cmd, line.to_string()));
                }
                _ => return Err(err(format!("can't parse: {line}"))),
            }
        }
        sc.cmds.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(sc)
    }

    /// Run due commands; returns per-spy input overrides for this step.
    pub fn step(&mut self, m: &mut Match) -> [Option<SpyInput>; 2] {
        let mut ov: [Option<SpyInput>; 2] = [None, None];
        while self.next < self.cmds.len() && self.cmds[self.next].0 <= self.t + 1e-4 {
            let (_, cmd, line) = self.cmds[self.next].clone();
            self.next += 1;
            self.run(m, cmd, &line, &mut ov);
        }
        for &(s, dir, until) in &self.holds {
            if self.t < until {
                ov[s].get_or_insert_default().mv = dir;
            }
        }
        self.t += DT;
        ov
    }

    fn check(&mut self, ok: bool, line: &str, detail: String) {
        if ok {
            self.passes += 1;
            println!("  ok    {line}");
        } else {
            println!("  FAIL  {line}  ({detail})");
            self.failures.push(format!("{line} ({detail})"));
        }
    }

    fn run(&mut self, m: &mut Match, cmd: Cmd, line: &str, ov: &mut [Option<SpyInput>; 2]) {
        match cmd {
            Cmd::PlaceFurniture(s, k) => {
                let room = m.spies[s].room;
                let fs: Vec<usize> = m.furniture_in(room).collect();
                let f = fs[k % fs.len()];
                let fp = m.furniture[f].pos;
                // stand on the room-centre side of it, facing it
                let to_centre = (-fp).normalize_or_zero();
                m.spies[s].pos = fp + to_centre * (FURN_RADIUS + SPY_RADIUS + 0.2);
                m.spies[s].facing = -to_centre;
            }
            Cmd::PlaceDoor(s, sd) => {
                let d = side_dir(sd);
                m.spies[s].pos = d * (HALF - 1.0);
                m.spies[s].facing = d;
            }
            Cmd::PlaceExit(s) => {
                let side = m.rooms[m.exit_room].exit_side.unwrap_or(0);
                m.spies[s].room = m.exit_room;
                m.spies[s].pos = side_dir(side) * (HALF - 1.0);
                m.spies[s].facing = side_dir(side);
            }
            Cmd::Forward(s, secs) => {
                let f = m.spies[s].facing;
                self.holds.push((s, f, self.t + secs));
            }
            Cmd::ExpectOver => {
                let w = m.winner.map(|w| m.spies[w].name).unwrap_or("nobody");
                self.check(m.over, line, format!("over={} winner={w} time_left={:.0}", m.over, m.time_left));
            }
            Cmd::PlaceRoom(s, r, p) => {
                m.spies[s].room = r;
                m.spies[s].pos = p;
            }
            Cmd::Hide(i, s, k) => {
                for f in &mut m.furniture {
                    f.items.retain(|x| *x != i);
                }
                for sp in &mut m.spies {
                    sp.items.retain(|x| *x != i);
                }
                let fs: Vec<usize> = m.furniture_in(m.spies[s].room).collect();
                m.furniture[fs[k % fs.len()]].items.push(i);
            }
            Cmd::Give(s, i) => {
                let items: Vec<Item> = i.map(|x| vec![x]).unwrap_or(ALL_ITEMS.to_vec());
                for it in items {
                    for f in &mut m.furniture {
                        f.items.retain(|x| *x != it);
                    }
                    if !m.spies[s].items.contains(&it) {
                        m.spies[s].items.push(it);
                    }
                }
            }
            Cmd::Move(s, dir, secs) => self.holds.push((s, dir, self.t + secs)),
            Cmd::Press(s, b) => {
                let i = ov[s].get_or_insert_default();
                match b {
                    "search" => i.search = true,
                    "trap" => i.trap = true,
                    "attack" => i.attack = true,
                    _ => i.cycle = true,
                }
            }
            Cmd::ExpectRoom(s, r) => {
                let got = m.spies[s].room;
                self.check(got == r, line, format!("room is {got}"));
            }
            Cmd::ExpectHas(s, i) => {
                let ok = m.spies[s].items.contains(&i);
                self.check(ok, line, format!("has {:?}", m.spies[s].items));
            }
            Cmd::ExpectState(s, st) => {
                let got = match m.spies[s].state {
                    SpyState::Alive | SpyState::Searching { .. } => "alive",
                    SpyState::Dead { .. } => "dead",
                    SpyState::Stunned { .. } => "stunned",
                    SpyState::Escaped => "escaped",
                };
                self.check(got == st, line, format!("state is {got}"));
            }
            Cmd::ExpectWinner(w) => {
                let ok = m.winner == w;
                self.check(ok, line, format!("winner is {:?}", m.winner));
            }
            Cmd::Screenshot(f) => self.screenshots.push(f),
            Cmd::End => self.ended = true,
        }
    }
}
