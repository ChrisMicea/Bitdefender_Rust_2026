use std::collections::HashMap;
use crate::protocol::{GameConfig, GameState, Hero, HeroTypeConfig, MoveArgs, Player, ShootArgs, Projectile};
use pathfinding::prelude::bfs;

// Scoring weights
const W_CAN_SHOOT: f64 = 100.0; // tile from which we can hit the primary target
const W_APPROACH: f64 = 40.0; // base value for closing distance when not yet in range
const W_CROSSFIRE: f64 = 50.0; // peak bonus for a ~90° angle between our two heroes
const W_OPTIMAL_RANGE: f64 = 2.0; // penalty per tile away from optimal firing distance
const W_EXPOSURE: f64 = 40.0; // penalty for being shootable by a NON-target enemy
const W_INCOMING: f64 = 120.0; // penalty for sitting in a projectile's path

// this was only a basic mechanism for prioritizing "cover" but with no extra logic it only got my robots stuck in walls
const W_COVER: f64 = 0.0; // bonus for hugging a wall while reloading
const W_SAME_TILE_PENALTY: f64 = 80.0; // penalty for robots sitting on top of each other
const W_ADJACENT_TILE_PENALTY: f64 = 15.0; // smaller penalty for robots sitting on adjacent tiles - nudge them apart
// must stay BELOW W_SAME_TILE_PENALTY
// Smpirical tests show that this bias / bonus worked against the optimal distance bonus and made the robots stay still as the enemy came close
// so they had no more space to dodge.
const W_STAY_BIAS: f64 = 0.0; // in order to discourage robot oscillation, a robot will only move if a measurably better tile is available
const OPTIMAL_RANGE_FRAC: f64 = 0.65; // sweet spot as a fraction of max range

#[derive(Default)]
pub struct GameData {
    pub game_map: Vec<Vec<i32>>,
    pub my_player: Player,
    pub player_heroes: Vec<Hero>,
    game_config: GameConfig,
    current_destination: (i32, i32),
    middle_point: (i32, i32),
    game_state: GameState,
    primary_target_id: Option<i32>, // which enemy hero both of our heroes are currently focusing.
}

impl GameData {
    pub fn initialize_game(&mut self, config: GameConfig, state: GameState, my_player_id: i32) {
        self.game_config = config;

        // initialize game map's dimensions
        self.game_map = vec![vec![0; self.game_config.width as usize]; self.game_config.height as usize];

        // save the game state
        self.game_state = state;

        // determine which player (with which ID) am I
        for player in &self.game_config.players {
            if player.id == my_player_id {
                self.my_player = player.clone();
                break;
            }
        }

        println!(
            "map size: height={}, width={}",
            self.game_map.len(),
            self.game_map.first().map_or(0, |row| row.len())
        );

        // determine the heroes associated with my player's ID
        self.determine_player_heroes();

        // build the game map
        self.build_game_map();

        // set middle point
        self.middle_point = (
            (self.game_map[0].len() / 2) as i32,
            (self.game_map.len() / 2) as i32,
        );

        self.primary_target_id = None;
    }

    pub fn update_game_state(&mut self, state: GameState) {
        self.game_state = state;
    }

    // Returns (moves, shoots) for this turn — each hero appears in exactly one list.
    pub fn decide_actions(&mut self) -> (Vec<MoveArgs>, Vec<ShootArgs>) {
        let mut move_commands: Vec<MoveArgs> = Vec::new();
        let mut shoot_commands: Vec<ShootArgs> = Vec::new();

        self.update_heroes_from_state();
        let enemies = self.find_enemy_heroes();

        // Pick (and persist) the enemy both heroes will gang up on.
        let primary = self.pick_primary_target(&enemies);

        // Clone so we can reference the "other" hero's position while iterating.
        let heroes = self.player_heroes.clone();

        // Where each hero will end up THIS turn. A hero deciding later sees its
        // ally's committed move rather than its stale position — this is what
        // breaks the symmetry that makes two stacked heroes move identically.
        let mut committed: HashMap<i32, (i32, i32)> = HashMap::new();

        for hero in &heroes {
            // Offense first: if we can shoot, we don't move.
            if let Some(shoot_args) = self.try_shoot(hero, &enemies, primary) {
                println!("Hero {} shoots at ({}, {})", hero.id, shoot_args.x, shoot_args.y);
                committed.insert(hero.id, (hero.x, hero.y)); // a shooting hero holds position
                shoot_commands.push(shoot_args);
                continue;
            }

            // Otherwise pick the best tile to move to.
            // Ally's committed position if it already decided, else its current one.
            let other = heroes
                .iter()
                .find(|h| h.id != hero.id)
                .map(|h| committed.get(&h.id).copied().unwrap_or((h.x, h.y)));

            let mv = self.best_move_for_hero(hero, other, &enemies, primary);
            println!("Hero {} moves to ({}, {})", hero.id, mv.x, mv.y);
            committed.insert(hero.id, (mv.x, mv.y));
            move_commands.push(mv);
        }

        return (move_commands, shoot_commands)
    }

    // Picks the enemy to focus fire. Keeps the current target if it's still
    // visible (avoid flip-flopping); otherwise picks the lowest-HP enemy,
    // tie-broken by proximity to our heroes' centre.
    fn pick_primary_target(&mut self, enemies: &[Hero]) -> Option<(i32, i32)> {
        if enemies.is_empty() {
            self.primary_target_id = None;
            return None;
        }

        // Keep existing target if still present.
        if let Some(id) = self.primary_target_id {
            if let Some(e) = enemies.iter().find(|e| e.id == id) {
                return Some((e.x, e.y));
            }
        }

        // Centre of our heroes (for the proximity tie-break).
        let (mut sumx, mut sumy, mut nheroes) = (0i64, 0i64, 0i64);
        for h in &self.player_heroes {
            sumx += h.x as i64;
            sumy += h.y as i64;
            nheroes += 1;
        }
        let centre = if nheroes > 0 {
            ((sumx / nheroes) as i32, (sumy / nheroes) as i32)
        } else {
            self.middle_point
        };

        let best = enemies.iter().min_by(|a, b| {
            a.hp.cmp(&b.hp).then_with(|| {
                let da = coarse_dist((a.x, a.y), centre);
                let db = coarse_dist((b.x, b.y), centre);
                da.cmp(&db)
            })
        })?;

        self.primary_target_id = Some(best.id);
        Some((best.x, best.y))
    }

    // ── Movement: score every reachable tile, pick the best ─────────────────────

    fn best_move_for_hero(
        &self,
        hero: &Hero,
        other: Option<(i32, i32)>,
        enemies: &[Hero],
        primary: Option<(i32, i32)>,
    ) -> MoveArgs {
        // No enemies in sight → advance toward the middle via BFS (one step).
        if primary.is_none() {
            if let Some(next) = self.step_to_goal((hero.x, hero.y), self.middle_point) {
                return MoveArgs { hero_id: hero.id, x: next.0, y: next.1, comment: None };
            }
            return MoveArgs { hero_id: hero.id, x: hero.x, y: hero.y, comment: None };
        }

        let max_range = self.max_range_for(hero);

        // Candidate centers: stay put + 8 coarse-grid steps.
        let deltas: [(i32, i32); 9] = [
            (0, 0), (3, 0), (-3, 0), (0, 3), (0, -3),
            (3, 3), (3, -3), (-3, 3), (-3, -3),
        ];

        let mut best_tile = (hero.x, hero.y);
        let mut best_score = f64::NEG_INFINITY;

        for (dx, dy) in deltas {
            let c = (hero.x + dx, hero.y + dy);
            // Staying is always legal; moves must land on a valid, wall-free center.
            if (dx, dy) != (0, 0) && !self.is_center_legal(c.0, c.1) {
                continue;
            }

            let score = self.score_tile(c, hero, other, enemies, primary, max_range);
            if score > best_score {
                best_score = score;
                best_tile = c;
            }
        }

        MoveArgs { hero_id: hero.id, x: best_tile.0, y: best_tile.1, comment: None }
    }

    fn score_tile(
        &self,
        c: (i32, i32),
        hero: &Hero,
        other: Option<(i32, i32)>,
        enemies: &[Hero],
        primary: Option<(i32, i32)>,
        max_range: f64,
    ) -> f64 {
        let mut score = 0.0;

        if let Some(t) = primary {
            let dist = euclid(c, t);
            let clear = self.has_clear_shot_from(c, t);

            if clear && dist <= max_range {
                // We can shoot the target from here — strongly preferred.
                score += W_CAN_SHOOT;
                // Prefer a comfortable distance rather than point-blank or fringe.
                let optimal = OPTIMAL_RANGE_FRAC * max_range;
                score -= (dist - optimal).abs() * W_OPTIMAL_RANGE;
            } else {
                // Not in a firing position yet — reward closing the gap.
                score += W_APPROACH - coarse_dist(c, t) as f64;
            }

            // Crossfire: reward a wide angle between our two heroes vs the target.
            if let Some(o) = other {
                score += crossfire_bonus(c, o, t);
            }
        }

        // Exposure: penalize tiles a NON-target enemy could shoot (we accept exposure to the target, since that's who we're trading with).
        for e in enemies {
            let is_primary = primary.map_or(false, |t| t == (e.x, e.y));
            if is_primary {
                continue;
            }
            if self.has_clear_shot_from((e.x, e.y), c) {
                score -= W_EXPOSURE;
            }
        }

        // Incoming projectiles: heavily penalize stepping into a live trajectory.
        for p in &self.game_state.projectiles {
            if p.owner_id == hero.owner_id {
                continue; // our own shots don't hurt us (no friendly fire)
            }
            if self.projectile_threatens(p, c) {
                score -= W_INCOMING;
            }
        }

        // Cover: while reloading, hugging a wall is safer.
        if hero.cooldown > 0 && self.adjacent_to_wall(c) {
            score += W_COVER;
        }

        // Anti-stacking: discourage sitting on top of our own hero.
        if let Some(o) = other {
            let d = coarse_dist(c, o);
            if d == 0 { score -= W_SAME_TILE_PENALTY; }  // same tile: strongly avoid
            else if d == 1 { score -= W_ADJACENT_TILE_PENALTY; }  // adjacent: mild nudge apart
        }

        // stay bonus: a small bonus for holding position, so a hero relocates only
        // when another tile is clearly better. This is what kills the back-and-forth oscillation.
        if c == (hero.x, hero.y) {
            score += W_STAY_BIAS;
        }

        return score
    }

    // True if all nine tiles of the 3×3 footprint centered at (cx,cy) are in-bounds and wall-free.
    fn is_center_legal(&self, cx: i32, cy: i32) -> bool {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 {
                    return false;
                }
                let (xu, yu) = (x as usize, y as usize);
                if yu >= self.game_map.len() || xu >= self.game_map[yu].len() {
                    return false;
                }
                if self.game_map[yu][xu] != 0 {
                    return false;
                }
            }
        }
        return true
    }

    // Clear line of sight from one center to another (no wall on the Bresenham line, excluding the origin tile itself).
    fn has_clear_shot_from(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let line = bresenham_line(from.0, from.1, to.0, to.1);
        line.iter().skip(1).all(|&(x, y)| {
            if x < 0 || y < 0 {
                return false;
            }
            let (xi, yi) = (x as usize, y as usize);
            yi < self.game_map.len() && xi < self.game_map[yi].len() && self.game_map[yi][xi] == 0
        })
    }

    // Will this projectile pass through the 3×3 footprint centered at `c`
    // within the next couple of turns? (Conservative: ignores walls/edges
    // that might consume it first.)
    fn projectile_threatens(&self, p: &Projectile, c: (i32, i32)) -> bool {
        let dirx = p.x - p.origin_x;
        let diry = p.y - p.origin_y;

        // Freshly fired and still at origin -> we can't infer a direction; just treat its current tile as the threat.
        if dirx == 0 && diry == 0 {
            return point_in_footprint((p.x, p.y), c);
        }

        // Extend a long ray in the direction of travel and walk it forward.
        let far = (p.x + dirx * 30, p.y + diry * 30);
        let line = bresenham_line(p.x, p.y, far.0, far.1);

        let speed = self
            .game_config
            .hero_types
            .get("sniper")
            .map(|h| h.projectile_speed)
            .unwrap_or(1)
            .max(1);
        let lookahead = (speed * 2) as usize + 1; // two turns of travel

        for (i, &(lx, ly)) in line.iter().enumerate() {
            if i > lookahead {
                break;
            }
            if point_in_footprint((lx, ly), c) {
                return true;
            }
        }
        return false
    }

    // Is there a wall just beyond the footprint in any cardinal direction?
    fn adjacent_to_wall(&self, c: (i32, i32)) -> bool {
        // +- 2 and also works with +- 3, 4, 5 because we check against the CENTER of a hero and that hero's body extends to +-1
        // so from +- 2 to +- 5 we would have a wall
        let probes = [(2, 0), (-2, 0), (0, 2), (0, -2)];
        for (dx, dy) in probes {
            let (x, y) = (c.0 + dx, c.1 + dy);
            if x < 0 || y < 0 {
                continue;
            }
            let (xu, yu) = (x as usize, y as usize);
            if yu < self.game_map.len() && xu < self.game_map[yu].len() && self.game_map[yu][xu] != 0
            {
                return true;
            }
        }
        false
    }

    fn max_range_for(&self, hero: &Hero) -> f64 {
        self.game_config
            .hero_types
            .get(&hero.type_)
            .map(|h| (h.projectile_ttl * h.projectile_speed) as f64)
            .unwrap_or(0.0)
    }

    // One BFS step toward a goal (used when no enemies are visible).
    fn step_to_goal(&self, from: (i32, i32), goal: (i32, i32)) -> Option<(i32, i32)> {
        let path = my_bfs(from, goal, &self.game_map);
        path.first().copied()
    }

    fn has_clear_shot(&self, from: &Hero, to: &Hero) -> bool {
        self.has_clear_shot_from((from.x, from.y), (to.x, to.y))
    }

    // Returns a ShootArgs if the hero can fire at any enemy this turn, otherwise None.
    // The primary (focus-fire) target is checked first.
    fn try_shoot(&self, hero: &Hero, enemies: &[Hero], primary: Option<(i32, i32)>) -> Option<ShootArgs> {
        if hero.cooldown > 0 {
            return None;
        }

        let max_range = self.max_range_for(hero);

        // Order enemies so the focus-fire target is considered first.
        let mut ordered: Vec<&Hero> = enemies.iter().collect();
        if let Some(t) = primary {
            ordered.sort_by_key(|e| if (e.x, e.y) == t { 0 } else { 1 });
        }

        for enemy in ordered {
            let dist = euclid((hero.x, hero.y), (enemy.x, enemy.y));
            if dist > max_range {
                continue;
            }
            if self.has_clear_shot(hero, enemy) {
                // TODO (next step): aim past the enemy to extend the beam.
                return Some(ShootArgs {
                    hero_id: hero.id,
                    x: enemy.x,
                    y: enemy.y,
                    comment: None,
                });
            }
        }

        None
    }

    fn update_heroes_from_state(&mut self) {
        self.player_heroes.clear();
        self.determine_player_heroes();
    }

    fn build_game_map(&mut self) {
        for wall in &self.game_state.walls {
            // walls are 3x3 squares, centered on wall.x, wall.y
            // self.game_map[wall.y as usize][wall.x as usize] = 1;
            // self.game_map[wall.y as usize][wall.x as usize + 1] = 1;
            // self.game_map[wall.y as usize][wall.x as usize - 1] = 1;
            // self.game_map[wall.y as usize + 1][wall.x as usize] = 1;
            // self.game_map[wall.y as usize - 1][wall.x as usize] = 1;
            // self.game_map[wall.y as usize + 1][wall.x as usize + 1] = 1;
            // self.game_map[wall.y as usize + 1][wall.x as usize - 1] = 1;
            // self.game_map[wall.y as usize - 1][wall.x as usize + 1] = 1;
            // self.game_map[wall.y as usize - 1][wall.x as usize - 1] = 1;
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let y = (wall.y + dy) as usize;
                    let x = (wall.x + dx) as usize;
                    self.game_map[y][x] = 1;
                }
            }
        }
    }

    fn determine_player_heroes(&mut self) {
        let mut heroes = Vec::new();
        for hero in &self.game_state.heroes {
            if hero.owner_id == self.my_player.id {
                heroes.push(hero.clone());
            }
        }
        self.player_heroes = heroes;
    }

    fn find_enemy_heroes(&self) -> Vec<Hero> {
        let mut enemy_heroes = Vec::new();
        for hero in &self.game_state.heroes {
            if hero.owner_id != self.my_player.id {
                enemy_heroes.push(hero.clone());
            }
        }
        return enemy_heroes;
    }
}

// Euclidean distance between two tiles.
fn euclid(a: (i32, i32), b: (i32, i32)) -> f64 {
    let dx = (a.0 - b.0) as f64;
    let dy = (a.1 - b.1) as f64;
    (dx * dx + dy * dy).sqrt()
}

// check if the hero is in the projectile's trajectory - as in, check if the middle +- 1 is passed by the 1x1 beam
fn point_in_footprint(p: (i32, i32), center: (i32, i32)) -> bool {
    (p.0 - center.0).abs() <= 1 && (p.1 - center.1).abs() <= 1
}

// Crossfire bonus: peaks when our two heroes sit at ~90° relative to the target,
// approaches zero when they share a line (0° or 180°). Uses |sin(angle)|.
fn crossfire_bonus(hero_pos: (i32, i32), ally_pos: (i32, i32), target: (i32, i32)) -> f64 {
    // Vectors pointing from the target out to each of our heroes.
    let target_to_hero = (
        (hero_pos.0 - target.0) as f64,
        (hero_pos.1 - target.1) as f64,
    );
    let target_to_ally = (
        (ally_pos.0 - target.0) as f64,
        (ally_pos.1 - target.1) as f64,
    );

    // Length of each vector = straight-line distance from the target to that hero.
    let len_hero = (target_to_hero.0.powi(2) + target_to_hero.1.powi(2)).sqrt();
    let len_ally = (target_to_ally.0.powi(2) + target_to_ally.1.powi(2)).sqrt();

    // If a hero is sitting right on the target, the angle is undefined — bail out.
    if len_hero < 1e-6 || len_ally < 1e-6 {
        return 0.0;
    }

    // The 2D cross product magnitude satisfies |cross| = |u| * |v| * sin(angle),
    // so dividing by both lengths isolates |sin(angle)| on its own.
    let cross = (target_to_hero.0 * target_to_ally.1 - target_to_hero.1 * target_to_ally.0).abs();
    let sin_angle = cross / (len_hero * len_ally); // 0 at 0°/180°, 1 at 90°

    sin_angle * W_CROSSFIRE
}


// Chebyshev distance on the coarse 3-tile grid (number of hero steps).
fn coarse_dist(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs() / 3;
    let dy = (a.1 - b.1).abs() / 3;
    dx.max(dy)
}

fn my_bfs (start: (i32, i32), mut goal: (i32, i32), game_map: &Vec<Vec<i32>>) -> Vec<(i32, i32)> {
    // for now, just a wrapper for the bfs function inside the pathfinding crate

    #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    struct Pos(i32, i32);

    impl Pos {
        fn successors(&self, game_map: &Vec<Vec<i32>>) -> Vec<Pos> {
            let &Pos(x, y) = self;

            let mut potential_successors = vec![Pos(x+3,y+3), Pos(x+3,y), Pos(x+3,y-3), Pos(x,y+3),
                 Pos(x, y-3), Pos(x-3,y+3), Pos(x-3,y), Pos(x-3,y-3)];

            potential_successors.retain(|pos| {
                let Pos(px, py) = *pos;

                if px < 0 || py < 0 {
                    return false;
                }

                let px = px as usize;
                let py = py as usize;

                py < game_map.len() && px < game_map[py].len() && game_map[py][px] == 0
            });
            // potential_successors.retain(|&pos| (*game_map)[pos.1 as usize][pos.0 as usize] == 0);

            return  potential_successors;
        }
    }

    println!("goal.0: {}, goal.1: {}", goal.0, goal.1);

    let mut found = false;
    if game_map[goal.1 as usize][goal.0 as usize] != 0 {
        // middle point is a wall, so we search for a near non-wall point
        for i in goal.1 as usize..game_map.len() {
            for j in goal.0 as usize..game_map[0].len() {
                if game_map[i][j] == 0 {
                    goal.0 = j as i32;
                    goal.1 = i as i32;
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }

    let mut result = bfs(
        &Pos(start.0, start.1),
        |pos| pos.successors(game_map),
        |pos| {
            let Pos(x, y) = *pos;
            (x - goal.0).abs() <= 2 && (y - goal.1).abs() <= 2
        },
    );

    let mut path: Vec<(i32, i32)> = result
        .unwrap_or_default() // asa a zis AI-ul, in loc de unwrap()
        .into_iter()
        .map(|Pos(x, y)| (x, y))
        .collect();

    if path.len() >= 2 {
        // remove the first element from path, which is the start position
        path.remove(0);
    }
    else {
        println!("{}", path.len());
        // panic!("Path is empty!");
    }

    return path;
}

fn a_star () -> Vec<(i32, i32)> {
    // for now, not implemented
    return Vec::new();
}

fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    // All grid cells on the line from (x0, y0) to (x1, y1), inclusive, in visit order.

    // points: list[tuple[int, int]] = []
    // dx = abs(x1 - x0)
    // dy = -abs(y1 - y0)
    // sx = 1 if x0 < x1 else -1
    // sy = 1 if y0 < y1 else -1
    // err = dx + dy
    // x, y = x0, y0
    // while True:
    //     points.append((x, y))
    // if x == x1 and y == y1:
    // break
    //     e2 = 2 * err
    // if e2 >= dy:
    //     err += dy
    // x += sx
    // if e2 <= dx:
    //     err += dx
    // y += sy
    // return points
    let mut points = Vec::new();

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();

    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };

    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        points.push((x, y));

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;

        if e2 >= dy {
            err += dy;
            x += sx;
        }

        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }

    return points
}