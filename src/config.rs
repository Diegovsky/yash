use std::{path::PathBuf, io::BufRead};

use crate::utils::read_file;

pub fn get_config_folder() -> PathBuf {
    directories::BaseDirs::new().unwrap().config_dir().join("yash")
}

pub fn get_history_file() -> PathBuf {
    get_config_folder().join("yhist.txt")
}

pub fn get_history() -> std::io::Result<Vec<String>> {
    read_file(get_history_file())
}

pub fn get_yashfile() -> std::io::Result<Vec<String>> {
    read_file(get_config_folder().join("yashrc"))
}
