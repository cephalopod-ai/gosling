use serde_json::Value;
use std::collections::HashMap;
use std::sync::Once;

static EXPORT_ONCE: Once = Once::new();

/// Export AWS_* keys from goose config/secrets into the process environment so
/// `aws_config`'s default chain can resolve them.
///
/// `std::env::set_var` is a data race against any concurrent `getenv` on the
/// multi-threaded runtime (libc setenv/getenv are unsynchronized; this can
/// segfault). Providers are constructed per session and per subagent, so this
/// runs at most once per process instead of on every construction, and it
/// never overwrites variables already present in the real environment — which
/// also matches goose's usual env-over-config precedence.
pub(crate) fn export_aws_env_once<I>(sources: I)
where
    I: IntoIterator<Item = HashMap<String, Value>>,
{
    EXPORT_ONCE.call_once(|| {
        for map in sources {
            map.into_iter()
                .filter(|(key, _)| key.starts_with("AWS_"))
                .filter(|(key, _)| std::env::var_os(key).is_none())
                .filter_map(|(key, value)| value.as_str().map(|s| (key, s.to_string())))
                .for_each(|(key, s)| std::env::set_var(key, s));
        }
    });
}
