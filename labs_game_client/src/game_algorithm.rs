use std::collections::HashMap;
use crate::protocol::{GameConfig, GameState, Hero, HeroTypeConfig, MoveArgs, Player, ShootArgs};
use pathfinding::prelude::bfs;

#[derive(Default)]
pub struct GameData {
    pub game_map: Vec<Vec<i32>>,
    pub my_player: Player,
    pub player_heroes: Vec<Hero>,
    game_config: GameConfig,
    current_destination: (i32, i32),
    middle_point: (i32, i32),
    game_state: GameState,
    hero_paths: HashMap<i32, Vec<(i32, i32)>>,
    curr_path_index_for_hero: HashMap<i32, usize>,
}

impl GameData {
    pub fn initialize_game(&mut self, config: GameConfig, state: GameState, my_player_id: i32) {
        self.game_config = config;

        // initialize gmae map's dimensions
        self.game_map = vec![vec![0; self.game_config.width as usize]; self.game_config.height as usize];
        // for i in 0..config.height {
        //     for j  in 0..config.width {
        //         self.game_map[i as usize][j as usize] = 0;
        //     }
        // }

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

        self.current_destination = self.middle_point;

        self.hero_paths = HashMap::new();
        self.curr_path_index_for_hero = HashMap::new();
        for hero in &self.player_heroes {
            self.hero_paths.insert(hero.id, my_bfs((hero.x, hero.y), self.middle_point, &self.game_map));
            self.curr_path_index_for_hero.insert(hero.id, 0);
        }
    }

    pub fn update_game_state(&mut self, state: GameState) {
        self.game_state = state;
    }

    // Returns (moves, shoots) for this turn — each hero appears in exactly one list.
    pub fn decide_actions(&mut self) -> (Vec<MoveArgs>, Vec<ShootArgs>) {
        // TODO: implement own a* algorithm instead of library bfs
        let mut enemies: Vec<Hero> = Vec::new();
        let mut move_commands: Vec<MoveArgs> = Vec::new();
        let mut shoot_commands: Vec<ShootArgs> = Vec::new();

        self.update_heroes_from_state();
        self.update_destination_and_paths(&mut enemies);

        println!("\n\ndestination is {} {}\n", self.current_destination.0, self.current_destination.1);

        // let heroes: Vec<Hero> = self.player_heroes.clone();
        for hero in self.player_heroes.iter() {
            if let Some(shoot_args) = self.try_shoot(hero, &enemies) {
                println!("Hero {} shoots at ({}, {})", hero.id, shoot_args.x, shoot_args.y);
                shoot_commands.push(shoot_args);
            } else {
                let mv = self.move_hero(hero);
                println!("Move command: {:?}", mv);
                move_commands.push(mv);
                *self.curr_path_index_for_hero.entry(hero.id).or_insert(0) += 1;
            }
        }

        (move_commands, shoot_commands)
    }

    // Returns true if no wall lies on the Bresenham line between from and to
    // (the shooter's own tile is excluded from the check).
    fn has_clear_shot(&self, from: &Hero, to: &Hero) -> bool {
        let line = bresenham_line(from.x, from.y, to.x, to.y);
        line.iter().skip(1).all(|&(x, y)| {
            let xi = x as usize;
            let yi = y as usize;
            yi < self.game_map.len()
                && xi < self.game_map[yi].len()
                && self.game_map[yi][xi] == 0
        })
    }

    // Returns a ShootArgs if the hero can fire at any enemy this turn, otherwise None.
    // Conditions: cooldown == 0, enemy within projectile range, and no wall on the line.
    fn try_shoot(&self, hero: &Hero, enemies: &[Hero]) -> Option<ShootArgs> {
        if hero.cooldown > 0 {
            return None;
        }

        let hero_config = self.game_config.hero_types.get(&hero.type_)?;
        let max_range = (hero_config.projectile_ttl * hero_config.projectile_speed) as f64;

        for enemy in enemies {
            let dx = (enemy.x - hero.x) as f64;
            let dy = (enemy.y - hero.y) as f64;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > max_range {
                continue;
            }

            if self.has_clear_shot(hero, enemy) {
                // TODO: if possible, don't just shoot at the enemy's position, but to the maximum range
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

    fn update_destination_and_paths(&mut self, enemies: &mut Vec<Hero>) {
        // TODO: improve algorithm for detecting enemies by setting destination to closest enemy per hero
        *enemies = self.find_enemy_heroes();
        if enemies.len() > 0 {
            // only update paths when destination is also updated (also updates in move_hero(), don't forget)
            for hero in &self.player_heroes {
                let path = my_bfs((hero.x, hero.y), self.current_destination, &self.game_map);
                let is_path_empty = path.is_empty();
                self.hero_paths.insert(hero.id, path);
                if is_path_empty {
                    // move around a bit if you're on top of the enemy
                    if (self.game_map[(self.current_destination.1 + 3) as usize][self.current_destination.0 as usize] == 0) {
                        self.current_destination.1 += 3;
                    }
                    else if self.game_map[(self.current_destination.1 - 3) as usize][self.current_destination.0 as usize] == 0 {
                        self.current_destination.1 -= 3;
                    }
                    else if self.game_map[(self.current_destination.1) as usize][(self.current_destination.0 + 3) as usize] == 0 {
                        self.current_destination.0 += 3;
                    }
                    else if self.game_map[(self.current_destination.1) as usize][(self.current_destination.0 - 3) as usize] == 0 {
                        self.current_destination.0 -= 3;
                    }
                    else if self.game_map[(self.current_destination.1 + 3) as usize][(self.current_destination.0 - 3) as usize] == 0 {
                        self.current_destination.0 -= 3;
                        self.current_destination.1 += 3;
                    }
                    else if self.game_map[(self.current_destination.1 - 3) as usize][(self.current_destination.0 + 3) as usize] == 0 {
                        self.current_destination.0 += 3;
                        self.current_destination.1 -= 3;
                    }
                    else if self.game_map[(self.current_destination.1 + 3) as usize][(self.current_destination.0 + 3) as usize] == 0 {
                        self.current_destination.0 += 3;
                        self.current_destination.1 += 3;
                    }
                    else if self.game_map[(self.current_destination.1 - 3) as usize][(self.current_destination.0 - 3) as usize] == 0 {
                        self.current_destination.0 -= 3;
                        self.current_destination.1 -= 3;
                    }
                }
                else {
                    // only home straight in on the enemy if you are not already on top of him (bfs distance is 0)
                    self.current_destination = (enemies[0].x, enemies[0].y);
                }
                self.curr_path_index_for_hero.insert(hero.id, 0);
            }
        }
        else {
            self.current_destination = self.middle_point;
        }
    }

    fn move_hero(&self, hero: &Hero) -> MoveArgs {
        if self.curr_path_index_for_hero[&hero.id] >= self.hero_paths[&hero.id].len() {
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x,
                y: hero.y,
                comment: Some("I can't move".to_string()),
            };
        }

       return MoveArgs {
            hero_id: hero.id,
            x: self.hero_paths[&hero.id][self.curr_path_index_for_hero[&hero.id]].0,
            y: self.hero_paths[&hero.id][self.curr_path_index_for_hero[&hero.id]].1,
            comment: None,
        }
    }

    fn build_game_map(&mut self) {
        for wall in &self.game_state.walls {
            // walls are 3x3 squares, centered on wall.x, wall.y
            self.game_map[wall.y as usize][wall.x as usize] = 1;
            self.game_map[wall.y as usize][wall.x as usize + 1] = 1;
            self.game_map[wall.y as usize][wall.x as usize - 1] = 1;
            self.game_map[wall.y as usize + 1][wall.x as usize] = 1;
            self.game_map[wall.y as usize - 1][wall.x as usize] = 1;
            self.game_map[wall.y as usize + 1][wall.x as usize + 1] = 1;
            self.game_map[wall.y as usize + 1][wall.x as usize - 1] = 1;
            self.game_map[wall.y as usize - 1][wall.x as usize + 1] = 1;
            self.game_map[wall.y as usize - 1][wall.x as usize - 1] = 1;
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