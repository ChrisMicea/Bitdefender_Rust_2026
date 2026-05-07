use crate::protocol::{GameConfig, GameState, Hero, MoveArgs, Player};

#[derive(Default)]
pub struct GameData {
    pub game_map: Vec<Vec<i32>>,
    pub my_player: Player,
    pub player_heroes: Vec<Hero>,
    current_destination: (i32, i32),
    middle_point: (i32, i32),
    game_state: GameState,
}

impl GameData {
    pub fn initialize_game(&mut self, config: GameConfig, state: GameState, my_player_id: i32) {
        // initialize gmae map's dimensions
        self.game_map = vec![vec![0; config.width as usize]; config.height as usize];
        // for i in 0..config.height {
        //     for j  in 0..config.width {
        //         self.game_map[i as usize][j as usize] = 0;
        //     }
        // }

        // save the game state
        self.game_state = state.clone();

        // determine which player (with which ID) am I
        for player in config.players {
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
        self.build_game_map(&state);

        // set middle point
        self.middle_point = (
            (self.game_map[0].len() / 2) as i32,
            (self.game_map.len() / 2) as i32,
        );
    }

    pub fn update_game_state(&mut self, state: GameState) {
        self.game_state = state;
    }

    pub fn move_heroes(&mut self) -> Vec<MoveArgs> {
        // returns exactly the messages to be sent to the server
        let mut move_commands: Vec<MoveArgs> = Vec::new();

        self.update_heroes_from_state();
        self.update_destination();

        // for hero in &self.player_heroes {
        //     // TODO: implement algorithm for moving heroes:
        //     // for now: move towards the middle
        //     // if you see enemy heroes (inside vision range), move towards them

        println!("\n\ndestination is {} {}\n", self.current_destination.0, self.current_destination.1);

        for hero in &self.player_heroes {
            move_commands.push(self.move_hero(hero));
        }
        // }

        print!("\n\n\n\n");
        for move_cmd in &move_commands {
            println!("Move command: {:?}", move_cmd);
        }
        print!("\n\n\n\n");

        return move_commands;
    }

    fn update_heroes_from_state(&mut self) {
        self.player_heroes.clear();
        self.determine_player_heroes();
    }

    fn update_destination(&mut self) {
        // TODO: implement algorithm for detecting enemies or resetting destination to middle

        self.current_destination = self.middle_point;
    }

    fn move_hero(&self, hero: &Hero) -> MoveArgs {
        // movement directions: 8 in total, including diagonals
        // !!!! TODO: implement backtracking / moving back when all "logical" directions are blocked

        // if a wall is met, try another logical direction
        if (self.current_destination.0 > hero.x && self.current_destination.1 > hero.y)
            && (hero.y + 3 < self.game_map.len() as i32)
            && (hero.x + 3 < self.game_map[0].len() as i32)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x + 3 && h.y == hero.y + 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y + 3) as usize][(hero.x + 3) as usize] == 0)
        {
            // move right and down
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x + 3,
                y: hero.y + 3,
            };
        } else if (self.current_destination.0 > hero.x && self.current_destination.1 < hero.y)
            && (hero.y - 3 >= 0)
            && (hero.x + 3 < self.game_map[0].len() as i32)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x + 3 && h.y == hero.y - 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y - 3) as usize][(hero.x + 3) as usize] == 0)
        {
            // move right and up
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x + 3,
                y: hero.y - 3,
            };
        } else if (self.current_destination.0 < hero.x && self.current_destination.1 > hero.y)
            && (hero.y + 3 < self.game_map.len() as i32)
            && (hero.x - 3 >= 0)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x - 3 && h.y == hero.y + 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y + 3) as usize][(hero.x - 3) as usize] == 0)
        {
            // move left and down
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x - 3,
                y: hero.y + 3,
            };
        } else if (self.current_destination.0 < hero.x && self.current_destination.1 < hero.y)
            && (hero.y - 3 >= 0)
            && (hero.x - 3 >= 0)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x - 3 && h.y == hero.y - 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y - 3) as usize][(hero.x - 3) as usize] == 0)
        {
            // move left and up
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x - 3,
                y: hero.y - 3,
            };
        } else if (self.current_destination.0 > hero.x)
            && (hero.x + 3 < self.game_map[0].len() as i32)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x + 3 && h.y == hero.y)) // avoid collisions with own heroes
            && (self.game_map[(hero.y) as usize][(hero.x + 3) as usize] == 0)
        {
            // move right
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x + 3,
                y: hero.y,
            };
        } else if (self.current_destination.0 < hero.x)
            && (hero.x - 3 >= 0)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x - 3 && h.y == hero.y)) // avoid collisions with own heroes
            && (self.game_map[(hero.y) as usize][(hero.x - 3) as usize] == 0)
        {
            // move left
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x - 3,
                y: hero.y,
            };
        } else if (self.current_destination.1 > hero.y)
            && (hero.y + 3 < self.game_map.len() as i32)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x && h.y == hero.y + 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y + 3) as usize][(hero.x) as usize] == 0)
        {
            // move down
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x,
                y: hero.y + 3,
            };
        } else if (self.current_destination.1 < hero.y)
            && (hero.y - 3 >= 0)
            && (!self.player_heroes.iter().any(|h| h.x == hero.x && h.y == hero.y - 3)) // avoid collisions with own heroes
            && (self.game_map[(hero.y - 3) as usize][(hero.x) as usize] == 0)
        {
            // move up
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x,
                y: hero.y - 3,
            };
        } else {
            // missing backtrack logic, return current position
            return MoveArgs {
                hero_id: hero.id,
                x: hero.x,
                y: hero.y,
            };
        }
    }

    fn build_game_map(&mut self, state: &GameState) {
        for wall in &state.walls {
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
}
