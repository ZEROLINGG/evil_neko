//lib/src/lib.rs
mod shell;
mod sandbox;

#[macro_export]
macro_rules! s_fmt {
    ($fmt:expr $(,)?) => {
        $crate::obfstr::obfstr!($fmt).to_string()
    };
    ($fmt:expr, $($arg:tt)+) => {
        format!($crate::obfstr::obfstr!($fmt), $($arg)+)
    };
}
#[macro_export]
macro_rules! s {
    ($lit:expr) => {
        obfstr::obfstr!($lit).to_string()
    };
}
#[macro_export]
macro_rules! ss {
    ($lit:expr) => {
        obfstr::obfstr!($lit)
    };
}



#[cfg(test)]
mod tests {


}
