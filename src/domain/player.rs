#[derive(Debug, Clone)]
pub struct Player {
    pub id: u64,
    pub username: String,
    pub password_hash: String,
    pub is_locked: bool,
}

impl Player {
    pub fn new(
        id: u64,
        username: impl Into<String>,
        password_hash: impl Into<String>,
        is_locked: bool,
    ) -> Self {
        Self {
            id,
            username: username.into(),
            password_hash: password_hash.into(),
            is_locked,
        }
    }
}
