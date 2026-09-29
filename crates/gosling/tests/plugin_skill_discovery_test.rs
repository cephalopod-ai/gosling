use gosling::skills::discover_skills;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_plugin_skill(root: &Path, plugin: &str, skill: &str) {
    let skill_dir = root
        .join(".agents")
        .join("plugins")
        .join(plugin)
        .join("skills")
        .join(skill);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {plugin}:{skill}\ndescription: {skill} description\n---\nbody\n"),
    )
    .unwrap();
}

fn write_settings(base: &Path, contents: &str) {
    let dir = base.join(".config").join("gosling");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("settings.json"), contents).unwrap();
}

fn skill_names(project: &Path, root: &Path) -> Vec<String> {
    let _env = env_lock::lock_env([
        ("HOME", Some(root.to_str().unwrap())),
        ("GOSLING_PATH_ROOT", Some(root.to_str().unwrap())),
        ("GOSLING_DISABLE_KEYRING", Some("1")),
    ]);
    discover_skills(Some(project))
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

fn root_with_two_plugins() -> TempDir {
    let root = TempDir::new().unwrap();
    write_plugin_skill(root.path(), "pt-plugin", "review");
    write_plugin_skill(root.path(), "pt-other", "lint");
    root
}

#[test]
fn user_disabled_plugins_setting_hides_plugin_skills() {
    let root = root_with_two_plugins();
    let project = TempDir::new().unwrap();
    write_settings(root.path(), r#"{"disabledPlugins": ["pt-plugin"]}"#);

    let skills = skill_names(project.path(), root.path());

    assert!(
        !skills.contains(&"pt-plugin:review".to_string()),
        "{skills:?}"
    );
    assert!(skills.contains(&"pt-other:lint".to_string()), "{skills:?}");
}

#[test]
fn project_disabled_plugins_setting_hides_plugin_skills() {
    let root = root_with_two_plugins();
    let project = TempDir::new().unwrap();
    write_settings(project.path(), r#"{"disabledPlugins": ["pt-plugin"]}"#);

    let skills = skill_names(project.path(), root.path());

    assert!(
        !skills.contains(&"pt-plugin:review".to_string()),
        "{skills:?}"
    );
    assert!(skills.contains(&"pt-other:lint".to_string()), "{skills:?}");
}

#[test]
fn enabled_plugin_skills_are_found_when_working_dir_is_home() {
    let root = root_with_two_plugins();

    let skills = skill_names(root.path(), root.path());

    assert!(
        skills.contains(&"pt-plugin:review".to_string()),
        "{skills:?}"
    );
    assert!(skills.contains(&"pt-other:lint".to_string()), "{skills:?}");
}
