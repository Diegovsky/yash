use std::path::PathBuf;

pub fn get_config_folder() -> PathBuf {
    directories::BaseDirs::new().unwrap().config_dir().join("yash")
}

pub fn get_history_file() -> PathBuf {
    get_config_folder().join("yhist.txt")
}

pub fn get_history() -> std::io::Result<Vec<String>> {
    Ok(std::fs::read_to_string(get_history_file())?
            .split('\n')
            .filter(|s| !s.is_empty())
            .map(|s| String::from(s))
            .collect())
}
