use crate::domain::player::Player;
use crate::app::login::PlayerRepository;

pub struct InMemoryPlayerRepository {
    players: Vec<Player>,
}

impl InMemoryPlayerRepository {
    pub fn new_with_demo_user() -> Self {
        let demo = Player::new(1, "demo", "password", false);

        Self {
            players: vec![demo],
        }
    }
}

impl PlayerRepository for InMemoryPlayerRepository {
    fn get_by_username(&self, username: &str) -> Option<Player> {
        self.players
            .iter()
            .find(|p| p.username == username)
            .cloned()
    }
}
