//! Simple opponent: search rooms, plant traps, fight when close, run for the exit.

use crate::rules::*;
use bevy::math::Vec2;
use std::collections::HashSet;

#[derive(Default)]
pub struct Brain {
    searched: HashSet<usize>,
    seen_items: usize,
    think_t: f32,
    target_room: Option<usize>,
    wander_t: f32,
}

impl Brain {
    pub fn think(&mut self, m: &mut Match, s: usize, dt: f32) -> SpyInput {
        let mut inp = SpyInput::default();
        let me = &m.spies[s];
        if !me.can_act() {
            return inp;
        }
        // forget what was searched whenever items move around (a death scatters them)
        let total_held: usize = m.spies.iter().map(|sp| sp.items.len()).sum();
        if total_held < self.seen_items {
            self.searched.clear();
        }
        self.seen_items = total_held;
        self.think_t -= dt;

        let me = &m.spies[s];
        let other = &m.spies[1 - s];
        let (room, pos) = (me.room, me.pos);
        let have_all = me.items.len() == ALL_ITEMS.len();

        // 1. run for the exit
        if have_all {
            return self.go_to_room(m, s, m.exit_room, true);
        }

        // 2. fight if the other spy is close
        let other_here = other.room == room && other.can_act();
        if other_here && other.pos.distance(pos) < 4.0 {
            let to = other.pos - pos;
            inp.mv = to.normalize_or_zero();
            if to.length() < 1.3 {
                inp.mv = Vec2::ZERO;
                m.spies[s].facing = to.normalize_or_zero();
                inp.attack = self.think_t <= 0.0;
                if inp.attack {
                    self.think_t = 0.35;
                }
            }
            return inp;
        }

        // 3. search the nearest unsearched furniture in this room
        let next = m
            .furniture_in(room)
            .filter(|f| !self.searched.contains(f))
            .min_by(|&a, &b| m.furniture[a].pos.distance(pos).total_cmp(&m.furniture[b].pos.distance(pos)));
        if let Some(f) = next {
            let fp = m.furniture[f].pos;
            if fp.distance(pos) < FURN_RADIUS + SPY_RADIUS + 0.6 {
                if self.think_t <= 0.0 {
                    self.searched.insert(f);
                    self.think_t = 0.3;
                    // sometimes leave a surprise after searching it
                    if m.furniture[f].items.is_empty() && m.furniture[f].trap.is_none() && m.chance(0.3) {
                        m.spies[s].sel = m.rand_range(FURNITURE_TRAPS.len());
                        inp.trap = true;
                    } else {
                        inp.search = true;
                    }
                }
            } else {
                inp.mv = steer(m, room, pos, fp);
            }
            return inp;
        }

        // 4. move on: nearest room with unsearched furniture, otherwise wander
        let target = self.target_room.filter(|&r| r != room && m.furniture_in(r).any(|f| !self.searched.contains(&f)));
        let target = target.or_else(|| {
            (0..m.rooms.len())
                .filter(|&r| r != room && m.furniture_in(r).any(|f| !self.searched.contains(&f)))
                .min_by_key(|&r| room_dist(m, room, r))
        });
        let target = match target {
            Some(t) => t,
            None => {
                self.wander_t -= dt;
                if self.wander_t <= 0.0 || self.target_room == Some(room) || self.target_room.is_none() {
                    self.wander_t = 6.0;
                    self.searched.clear();
                    self.target_room = Some(m.rand_range(m.rooms.len()));
                }
                self.target_room.unwrap()
            }
        };
        self.target_room = Some(target);
        self.go_to_room(m, s, target, false)
    }

    fn go_to_room(&mut self, m: &mut Match, s: usize, target: usize, exiting: bool) -> SpyInput {
        let mut inp = SpyInput::default();
        let me = &m.spies[s];
        let (room, pos) = (me.room, me.pos);
        let side = if room == target {
            if exiting { m.rooms[room].exit_side } else { None }
        } else {
            m.next_room_towards(room, target).map(|(_, side)| side)
        };
        let Some(side) = side else { return inp };
        let door_pos = side_dir(side) * (HALF + 1.0);
        // occasionally rig the door on the way out
        if !exiting && pos.distance(side_dir(side) * HALF) < 1.6 && self.think_t <= 0.0 {
            self.think_t = 1.0;
            if m.spies[s].traps[Trap::Bucket.idx()] > 0 && m.chance(0.15) {
                inp.trap = true;
                return inp;
            }
        }
        inp.mv = steer(m, room, pos, door_pos);
        inp
    }
}

/// Head for `goal`, sliding around furniture in the way.
fn steer(m: &Match, room: usize, pos: Vec2, goal: Vec2) -> Vec2 {
    let mut dir = (goal - pos).normalize_or_zero();
    for f in m.furniture_in(room) {
        let fp = m.furniture[f].pos;
        let to = fp - pos;
        let d = to.length();
        if d < FURN_RADIUS + SPY_RADIUS + 0.9 && to.dot(dir) > 0.0 && fp.distance(goal) > 0.5 {
            let side = Vec2::new(-to.y, to.x).normalize_or_zero();
            let side = if side.dot(goal - pos) >= 0.0 { side } else { -side };
            dir = (dir + side * 1.2).normalize_or_zero();
        }
    }
    dir
}

fn room_dist(m: &Match, a: usize, b: usize) -> usize {
    let (ra, rb) = (&m.rooms[a], &m.rooms[b]);
    ra.gx.abs_diff(rb.gx) + ra.gy.abs_diff(rb.gy)
}
