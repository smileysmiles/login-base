/// Input accepted by the login use case.
#[derive(Debug)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}
