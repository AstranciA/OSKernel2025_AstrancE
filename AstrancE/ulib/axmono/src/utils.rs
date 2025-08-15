use alloc::{string::String, vec::Vec};

pub fn get_pwd_from_envs(env: &[String]) -> (Option<String>, Option<String>) {
    let mut old_pwd = None;
    let mut pwd = None;
    // 遍历环境变量数组
    for var in env {
        // 分割等号，获取键值对
        let parts: Vec<&str> = var.splitn(2, '=').collect();

        // 确保分割后有两个部分（键和值）
        debug_assert_eq!(parts.len(), 2);
        if parts[0] == "PWD" {
            pwd = Some(parts[1].into());
        } else if parts[0] == "OLDPWD" {
            old_pwd = Some(parts[1].into());
        }
    }

    (old_pwd, pwd)
}
