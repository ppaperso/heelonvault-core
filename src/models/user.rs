use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum UserRole {
    User,
    Admin,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub role: UserRole,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub preferred_language: String,
    pub show_passwords_in_edit: bool,
    pub updated_at: Option<String>,
}
