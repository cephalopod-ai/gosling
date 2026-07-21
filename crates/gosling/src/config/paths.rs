use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use std::path::PathBuf;

pub struct Paths;

impl Paths {
    fn get_dir(dir_type: DirType) -> PathBuf {
        if let Ok(test_root) = std::env::var("GOSLING_PATH_ROOT") {
            let base = PathBuf::from(test_root);
            match dir_type {
                DirType::Config => base.join("config"),
                DirType::Data => base.join("data"),
                DirType::State => base.join("state"),
                DirType::Plugins => base.join(".agents").join("plugins"),
                DirType::Agents => base.join(".agents").join("agents"),
                DirType::AgentsHome => base.join(".agents"),
            }
        } else {
            // NOTE: gosling deliberately uses its own app_name so its config/data/state
            // directories never collide with an upstream goose install on the same machine
            // (e.g. ~/.config/goose vs ~/.config/gosling).
            let strategy = choose_app_strategy(AppStrategyArgs {
                top_level_domain: "Block".to_string(),
                author: "Block".to_string(),
                app_name: "gosling".to_string(),
            })
            .expect("gosling requires a home dir");

            match dir_type {
                DirType::Config => strategy.config_dir(),
                DirType::Data => strategy.data_dir(),
                DirType::State => strategy.state_dir().unwrap_or(strategy.data_dir()),
                DirType::Plugins => strategy.home_dir().join(".agents").join("plugins"),
                DirType::Agents => strategy.home_dir().join(".agents").join("agents"),
                DirType::AgentsHome => strategy.home_dir().join(".agents"),
            }
        }
    }

    pub fn config_dir() -> PathBuf {
        RUNTIME_PATHS
            .try_with(|paths| paths.config_dir.clone())
            .unwrap_or_else(|_| { Self::get_dir(DirType::Config) })
    }

    pub fn data_dir() -> PathBuf {
        RUNTIME_PATHS
            .try_with(|paths| paths.data_dir.clone())
            .unwrap_or_else(|_| { Self::get_dir(DirType::Data) })
    }

    pub fn state_dir() -> PathBuf {
        RUNTIME_PATHS
            .try_with(|paths| paths.state_dir.clone())
            .unwrap_or_else(|_| { Self::get_dir(DirType::State) })
    }

    pub fn plugins_dir() -> PathBuf {
        Self::get_dir(DirType::Plugins)
    }

    pub fn agents_dir() -> PathBuf {
        Self::get_dir(DirType::Agents)
    }

    pub fn agents_home_dir() -> PathBuf {
        Self::get_dir(DirType::AgentsHome)
    }

    pub fn in_agents_home_dir(subpath: &str) -> PathBuf {
        Self::agents_home_dir().join(subpath)
    }

    pub fn in_state_dir(subpath: &str) -> PathBuf {
        Self::state_dir().join(subpath)
    }

    pub fn in_config_dir(subpath: &str) -> PathBuf {
        Self::config_dir().join(subpath)
    }

    pub fn in_data_dir(subpath: &str) -> PathBuf {
        Self::data_dir().join(subpath)
    }
}

enum DirType {
    Config,
    Data,
    State,
    Plugins,
    Agents,
    AgentsHome,
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl RuntimePaths {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf, state_dir: PathBuf) -> Self {
        Self {
            config_dir,
            data_dir,
            state_dir,
        }
    }
}

tokio::task_local! {
    static RUNTIME_PATHS: RuntimePaths;
}

impl Paths {
    pub async fn scope<F>(runtime_paths: RuntimePaths, future: F) -> F::Output
    where
        F: std::future::Future,
    {
        RUNTIME_PATHS.scope(runtime_paths, future).await
    }
}

#[cfg(test)]
mod runtime_paths_tests {
    use super::{Paths, RuntimePaths};

    #[tokio::test]
    async fn runtime_paths_are_isolated_between_concurrent_tasks() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();

        let first_paths = RuntimePaths::new(
            first.path().join("config"),
            first.path().join("data"),
            first.path().join("state"),
        );
        let second_paths = RuntimePaths::new(
            second.path().join("config"),
            second.path().join("data"),
            second.path().join("state"),
        );

        let (first_observed, second_observed) = tokio::join!(
            Paths::scope(first_paths.clone(), async {
                (
                    Paths::config_dir(),
                    Paths::data_dir(),
                    Paths::state_dir(),
                )
            }),
            Paths::scope(second_paths.clone(), async {
                (
                    Paths::config_dir(),
                    Paths::data_dir(),
                    Paths::state_dir(),
                )
            }),
        );

        assert_eq!(
            first_observed,
            (
                first_paths.config_dir,
                first_paths.data_dir,
                first_paths.state_dir,
            )
        );
        assert_eq!(
            second_observed,
            (
                second_paths.config_dir,
                second_paths.data_dir,
                second_paths.state_dir,
            )
        );
    }
}
