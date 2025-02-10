#[derive(PartialEq, Debug, Clone)]
pub enum Action {
    EnvBuild(String),
    EnvRun(String),
    Skip,
}

pub type Actions = Vec<Action>;
