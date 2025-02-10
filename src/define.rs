use crate::{action, env, path_match};
use glob::Pattern;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::ffi::OsString;
use toml;

const DEFAULT_DEFINE: &str = r#"
[school]
# system = "predefined"
build = 'use flake {}'
run = 'direnv exec . {}'

[groups]
some-c = {build = 'use flake "github:the-nix-way/dev-templates?dir=c-cpp"'}
some-go = {build = 'use flake "github:the-nix-way/dev-templates?dir=go"'}
some-js = {build = 'use flake "github:the-nix-way/dev-templates?dir=node"'}
js-dev = {run = 'direnv exec . npm install .'}
some-pl = {build = 'use flake "github:the-nix-way/dev-templates?dir=perl"'}
some-py = [
    {build = 'use flake "github:the-nix-way/dev-templates?dir=python"'},
    {build = 'layout python'},
]
py-dev = [
    {build = 'export PYTHONSTARTUP=~/.config/python/config.py'},
    {run = 'direnv exec . pip install -U pip'},
    {run = 'direnv exec . pip install -r ~/.config/python/requirements.txt'},
    {run = 'direnv exec . pip install -e .'},
]
some-rs = {build = 'use flake "github:the-nix-way/dev-templates?dir=rust"'}

[[events]]
glob = '.*'
action = {skip = true}

[[events]]
extension = ['c', 'h']
group = 'some-c'

[[events]]
extension = ['go']
name = ['go.mod', 'go.sum', 'go.work']
group = 'some-go'

[[events]]
extension = ['js', 'ts']
group = 'some-js'

[[events]]
name = ['package.json']
group = ['some-js', 'js-dev']

[[events]]
extension = ['pl', 'pm', 'pod']
name = ['Makefile.pl', 'Build.pl', 'cpanfile']
group = 'some-pl'

[[events]]
extension = ['py', 'pyx', 'pyw', 'pyi', 'ipy', 'ipynb']
group = 'some-py'

[[events]]
name = ['setup.py', 'pyproject.toml', 'setup.cfg', 'Pipfile']
group = ['some-py', 'py-dev']

[[events]]
extension = ['rs']
group = 'some-rs'

[[events]]
name = ['Cargo.toml']
group = 'some-rs'
"#;

#[derive(Debug, Deserialize)]
struct Config {
    school: School,
    groups: Option<HashMap<String, TomlActions>>,
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct School {
    build: String,
    run: String,
}

#[derive(Debug)]
struct Strings(Vec<String>);

impl<'de> Deserialize<'de> for Strings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum _Strings {
            Unit(String),
            Array(Vec<String>),
        }

        let strings = _Strings::deserialize(deserializer)?;
        match strings {
            _Strings::Unit(s) => Ok(Strings(vec![s])),
            _Strings::Array(s) => Ok(Strings(s)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TomlAction {
    build: Option<Strings>,
    run: Option<Strings>,
    skip: Option<toml::Value>,
}

#[derive(Debug)]
struct TomlActions(Vec<TomlAction>);

impl<'de> Deserialize<'de> for TomlActions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum _TomlActions {
            Unit(TomlAction),
            Array(Vec<TomlAction>),
        }

        let actions = _TomlActions::deserialize(deserializer)?;
        match actions {
            _TomlActions::Unit(s) => Ok(TomlActions(vec![s])),
            _TomlActions::Array(s) => Ok(TomlActions(s)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Event {
    extension: Option<Strings>,
    name: Option<Strings>,
    glob: Option<Strings>,
    action: Option<TomlActions>,
    group: Option<Strings>,
}

fn toml_action_to_actions(actions: TomlActions) -> Result<action::Actions, String> {
    let mut env_actions = Vec::new();
    for act in actions.0 {
        if let Some(build) = act.build {
            for b in build.0 {
                env_actions.push(action::Action::EnvBuild(b));
            }
        }
        if let Some(run) = act.run {
            for r in run.0 {
                env_actions.push(action::Action::EnvRun(r));
            }
        }
        if let Some(_) = act.skip {
            env_actions.push(action::Action::Skip);
        }
    }

    if env_actions.is_empty() {
        Err("An action must be specified".to_string())
    } else {
        Ok(env_actions)
    }
}

fn config_to_env(config: Config) -> Result<env::EnvBuilder, String> {
    let group_aliases: HashMap<String, action::Actions> = config
        .groups
        .unwrap_or(HashMap::new())
        .into_iter()
        .map(|(k, v)| match toml_action_to_actions(v) {
            Ok(a) => Ok((k, a)),
            Err(e) => Err(e),
        })
        .collect::<Result<HashMap<_, _>, _>>()?;

    let mut env = HashMap::new();
    for event in config.events {
        let mut actions = Vec::new();
        if let Some(action) = event.action {
            actions.extend(toml_action_to_actions(action)?);
        }
        if let Some(group) = event.group {
            for g in group.0 {
                if let Some(group_action) = group_aliases.get(&g) {
                    actions.extend(group_action.clone());
                } else {
                    return Err("Group Alias is not defined".to_string());
                }
            }
        }
        if actions.is_empty() {
            return Err("Must specify an event action".to_string());
        }

        let mut targets = Vec::new();
        if let Some(extension) = event.extension {
            for ext in extension.0 {
                targets.push(path_match::PathMatch::Extension(OsString::from(ext)));
            }
        }
        if let Some(name) = event.name {
            for n in name.0 {
                targets.push(path_match::PathMatch::Name(OsString::from(n)));
            }
        }
        if let Some(glob) = event.glob {
            for glb in glob.0 {
                targets.push(path_match::PathMatch::Glob(Pattern::new(&glb).unwrap()));
            }
        }
        // TODO: raise this error during deserialize
        if actions.is_empty() {
            return Err("Must specify an event target".to_string());
        };

        for t in targets {
            env.insert(t, actions.clone());
        }
    }

    Ok(env::EnvBuilder::raw_new(env))
}

pub fn toml_to_env(toml: &str) -> Result<env::EnvBuilder, String> {
    let structured_toml = toml::from_str(toml).unwrap();
    config_to_env(structured_toml)
}

pub fn default_env() -> env::EnvBuilder {
    let structured_toml = toml::from_str(DEFAULT_DEFINE).unwrap();
    config_to_env(structured_toml).unwrap()
}

#[cfg(test)]
mod test {
    use crate::define;

    #[test]
    fn define_minimal() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [[events]]
        name = 'code'
        action = {skip = true}
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_multi_names() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [[events]]
        name = ['code', 'program']
        action = {skip = true}
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_extension() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [[events]]
        extension = 'c'
        action = {skip = true}
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_glob() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [[events]]
        glob = '*'
        action = {skip = true}
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_group() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [groups]
        here = {run = 'over here'}
        [[events]]
        name = 'code'
        group = 'here'
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_full_event() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [groups]
        here = {run = 'over here'}
        [[events]]
        name = 'code'
        glob = '*'
        extension = 'c'
        group = 'here'
        action = {skip = true}
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_full_multi_event() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [groups]
        here = {run = 'over here'}
        there = {build ='this'}
        [[events]]
        name = ['code', 'program']
        glob = ['*', '.*']
        extension = ['c', 'h']
        group = ['here', 'there']
        action = [{skip = true}, {run = 'something else'}]
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_multi_group() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [groups]
        here = {skip = true, build = 'this', run = 'over here'}
        [[events]]
        name = 'code'
        group = 'here'
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }

    #[test]
    fn define_many_multi_group() {
        let config: define::Config = toml::from_str(
            r#"
        [school]
        build = 'build'
        run = 'run'
        [groups]
        here = [{skip = true, build = 'this', run = 'over here'}, {run = 'other'}]
        [[events]]
        name = 'code'
        group = 'here'
        "#,
        )
        .unwrap();

        assert!(define::config_to_env(config).is_ok());
    }
}
