use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamMemberRole {
    Member,
    Leader,
}

impl TeamMemberRole {
    pub fn to_db_str(&self) -> &'static str {
        match self {
            TeamMemberRole::Member => "member",
            TeamMemberRole::Leader => "leader",
        }
    }

    pub fn from_db_str(s: &str) -> Result<Self, String> {
        match s {
            "member" => Ok(TeamMemberRole::Member),
            "leader" => Ok(TeamMemberRole::Leader),
            other => Err(format!("unknown team member role: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub created_by: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct TeamMember {
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: TeamMemberRole,
    pub joined_at: String,
}
