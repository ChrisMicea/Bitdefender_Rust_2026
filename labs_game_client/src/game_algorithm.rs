use std::path::absolute;
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
    hero_paths: Vec<Vec<(i32, i32)>>,
    curr_path_index_for_hero: Vec<usize>,
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

        self.hero_paths = Vec::new();
        for hero in &self.player_heroes {
            let path = my_bfs((hero.x, hero.y), self.middle_point, &self.game_map);

            self.hero_paths.push(my_bfs((hero.x, hero.y), self.middle_point, &self.game_map));
        }

        self.curr_path_index_for_hero = vec![0; self.player_heroes.len()];
    }

    pub fn update_game_state(&mut self, state: GameState) {
        self.game_state = state;
    }

    pub fn move_heroes(&mut self) -> Vec<MoveArgs> {
        // TODO: implement own a* algorithm instead of library bfs

        // returns exactly the messages to be sent to the server
        let mut move_commands: Vec<MoveArgs> = Vec::new();

        self.update_heroes_from_state();
        self.update_destination_and_paths();

        println!("\n\ndestination is {} {}\n", self.current_destination.0, self.current_destination.1);

        for hero_index in 0..self.player_heroes.len() {
            let hero = &self.player_heroes[hero_index];
            move_commands.push(self.move_hero(&hero, hero_index));
            self.curr_path_index_for_hero[hero_index] += 1;
        }

        // VVVVVV unsafe because hero_paths is always indexed from 0 so you can't access it by hero.id
        // for hero in &self.player_heroes {
        //     move_commands.push(self.move_hero(hero));
        //     self.curr_path_index_for_hero[hero.id as usize] += 1; // increment path index (how far we are along the path) for this hero
        // }
        // }

        print!("\n\n\n\n");
        for move_cmd in &move_commands {
            println!("Move command: {:?}", move_cmd);
        }
        print!("\n\n\n\n");

        return move_commands;
    }

    // pub fn shoot(&mut self, hero: &Hero, target: (i32, i32)) -> Option<ShootArgs> {
    //     // LOGIC: only enter this function / method if you see an enemy;
    //     // if the bresenham line does not pass through a wall
    //     // (optional): if the projectile cannot reach the enemy (ttl and projectile speed ... cover distance?), don't shoot
    //     // shoot at the enemy
    //
    //     let line = bresenham_line(hero.x, hero.y, target.0, target.1);
    //
    //     for i in 0..line.len() {
    //         if line[i].0 == 1 || line[i].1 == 0
    //             return
    //     }
    // }

    fn update_heroes_from_state(&mut self) {
        self.player_heroes.clear();
        self.determine_player_heroes();
    }

    fn update_destination_and_paths(&mut self) {
        // TODO: improve algorithm for detecting enemies by setting destination to closest enemy per hero
        let enemy_heroes = self.find_enemy_heroes();
        if enemy_heroes.len() > 0 {
            self.current_destination = (enemy_heroes[0].x, enemy_heroes[0].y);

            // only update paths when destinatios is also updated
            for hero in &self.player_heroes {
                self.hero_paths[hero.id as usize] = my_bfs((hero.x, hero.y), self.current_destination, &self.game_map);
                self.curr_path_index_for_hero[hero.id as usize] = 0;
            }
        }
        else {
            self.current_destination = self.middle_point;
        }
    }

    fn move_hero(& self, hero: &Hero, hero_index: usize) -> MoveArgs {
        if self.curr_path_index_for_hero[hero_index] >= self.hero_paths[hero_index].len() {
            // reached destination - move nothing
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x,
                y: hero.y,
                comment: Some("I can't move".to_string()),
            };
        }

        let returned_move = MoveArgs {
            hero_id: hero.id,
            x: self.hero_paths[hero_index][self.curr_path_index_for_hero[hero_index]].0,
            y: self.hero_paths[hero_index][self.curr_path_index_for_hero[hero_index]].1,
            comment: None,
        };

        return returned_move;
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

    fn find_enemy_heroes(&mut self) -> Vec<Hero> {
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