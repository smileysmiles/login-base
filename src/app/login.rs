use crate::domain::player::Player;

#[derive(Debug)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
}

pub trait PlayerRepository {
    fn get_by_username(&self, username: &str) -> Option<Player>;
}

pub trait ComplianceService {
    fn is_excluded(&self, player_id: u64) -> bool;
}

pub struct LoginService<R: PlayerRepository, C: ComplianceService> {
    players: R,
    compliance: C,
}

impl<R: PlayerRepository, C: ComplianceService> LoginService<R, C> {
    pub fn new(players: R, compliance: C) -> Self {
        Self { players, compliance }
    }

    pub fn login(&self, req: LoginRequest) -> LoginResponse {
        let Some(player) = self.players.get_by_username(&req.username) else {
            return LoginResponse {
                success: false,
                message: "Invalid username or password".to_string(),
            };
        };

        if player.is_locked {
            return LoginResponse {
                success: false,
                message: "Account locked".to_string(),
            };
        }

        // For now, keep it simple: plain-text check.
        // We’ll swap this for proper hashing later.
        if req.password != player.password_hash {
            return LoginResponse {
                success: false,
                message: "Invalid username or password".to_string(),
            };
        }

        if self.compliance.is_excluded(player.id) {
            return LoginResponse {
                success: false,
                message: "Self-excluded".to_string(),
            };
        }

        LoginResponse {
            success: true,
            message: "OK".to_string(),
        }
    }
}
