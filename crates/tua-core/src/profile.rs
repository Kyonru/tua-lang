use std::borrow::Cow;
use std::env;
use std::time::Instant;

pub struct ProfileSpan {
    enabled: bool,
    name: Cow<'static, str>,
    start: Instant,
}

impl Drop for ProfileSpan {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        eprintln!(
            "[tua-profile] {} {:.3}ms",
            self.name,
            self.start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

pub fn enabled() -> bool {
    env::var_os("TUA_PROFILE").is_some_and(|value| value != "0")
}

pub fn span(name: impl Into<Cow<'static, str>>) -> ProfileSpan {
    ProfileSpan {
        enabled: enabled(),
        name: name.into(),
        start: Instant::now(),
    }
}

pub fn span_for_path(name: &'static str, path: &std::path::Path) -> ProfileSpan {
    span(format!("{name} {}", path.display()))
}
