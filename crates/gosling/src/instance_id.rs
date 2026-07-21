use crate::config::paths::Paths;
use once_cell::sync::Lazy;
use std::fs;
use uuid::Uuid;

static INSTANCE_IDS_BY_STATE_PATH: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, String>>,
> = std::sync::LazyLock::new(|| {
    std::sync::Mutex::new(std::collections::HashMap::new())
});

pub fn get_instance_id() -> String {
    let state_dir = Paths::state_dir();
    let mut instance_ids = INSTANCE_IDS_BY_STATE_PATH
        .lock()
        .expect("instance ID path map lock poisoned");

    if let Some(instance_id) = instance_ids.get(&state_dir) {
        return instance_id.clone();
    }

    let instance_id_file = state_dir.join("instance_id");
    let instance_id = match std::fs::read_to_string(&instance_id_file) {
        Ok(instance_id) => instance_id,
        Err(_) => {
            let instance_id = Uuid::new_v4().to_string();
            if let Some(parent) = instance_id_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&instance_id_file, &instance_id);
            instance_id
        }
    };

    instance_ids.insert(state_dir, instance_id.clone());
    instance_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_id_is_stable() {
        let id1 = get_instance_id();
        let id2 = get_instance_id();
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
    }
}
