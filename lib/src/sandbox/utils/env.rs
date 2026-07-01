#![allow(unused)]



use crate::sandbox::*;



pub(crate) fn __env_has_key(kv: &(String, String), match_key: impl Str, mut action: ScoreAction) -> Option<ScoreAction>{
    if kv.0 == match_key.as_str() {
        action.set_msg(s_add!("Env Key found: ", kv.0));
        return Some(action);
    }
    None
}

pub(crate) fn __env_has_key_prefix(kv: &(String, String), prefix: impl Str, mut action: ScoreAction) -> Option<ScoreAction> {
    if kv.0.starts_with(prefix.as_str()) {
        action.set_msg(s_add!("Env Key prefix found: ", kv.0));
        return Some(action);
    }
    None
}

fn __env_has_key_val(kv: &(String, String), contains: impl Str, mut action: ScoreAction) -> Option<ScoreAction> {
    if kv.1.contains(&contains.as_str()) {
        action.set_msg(s_fmt!("[{}] contains: {}", kv.0, contains));
        return Some(action);
    }
    None
}
pub(crate) fn __env_has_kv(kv: &(String, String), match_key: impl Str, contains: &[impl Str], mut action: ScoreAction) -> Option<ScoreAction> {
    if kv.0 != match_key.as_str() { return None ; }
    for contain in contains {
        if kv.1.contains(contain.as_str()) {
            action.set_msg(s_add!("[", kv.0, "] contains: ", contain));
            return Some(action);
        }
    }
    None
}

