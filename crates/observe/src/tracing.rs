use {
    crate::{request_id::RequestIdLayer, tracing_reload_handler::spawn_reload_handler},
    std::{panic::PanicHookInfo, sync::Once},
    time::macros::format_description,
    tracing::level_filters::LevelFilter,
    tracing_subscriber::{
        EnvFilter,
        Layer,
        fmt::{time::UtcTime, writer::MakeWriterExt as _},
        prelude::*,
        util::SubscriberInitExt,
    },
};

/// Initializes tracing setup that is shared between the binaries.
/// `env_filter` has similar syntax to env_logger. It is documented at
/// https://docs.rs/tracing-subscriber/0.2.15/tracing_subscriber/filter/struct.EnvFilter.html
pub fn initialize(env_filter: &str, stderr_threshold: LevelFilter, use_json_format: bool) {
    set_tracing_subscriber(env_filter, stderr_threshold, use_json_format);
    std::panic::set_hook(Box::new(tracing_panic_hook));
}

/// Like [`initialize`], but can be called multiple times in a row. Later calls
/// are ignored.
///
/// Useful for tests.
pub fn initialize_reentrant(env_filter: &str, use_json_format: bool) {
    // The tracing subscriber below is global object so initializing it again in the
    // same process by a different thread would fail.
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_tracing_subscriber(env_filter, LevelFilter::ERROR, use_json_format);
        std::panic::set_hook(Box::new(tracing_panic_hook));
    });
}

fn set_tracing_subscriber(env_filter: &str, stderr_threshold: LevelFilter, use_json_format: bool) {
    let initial_filter = env_filter.to_string();

    // The `tracing` APIs are heavily generic to enable zero overhead. Unfortunately
    // this leads to very annoying type constraints which can only be satisfied
    // by literally copy and pasting the code so the compiler doesn't try to
    // infer types that satisfy both the tokio-console and the regular case.
    // It's tempting to resolve this mess by first configuring the `fmt_layer` and
    // only then the `console_subscriber`. However, this setup was the only way
    // I found that:
    // 1. actually makes `tokio-console` work
    // 2. prints logs if `tokio-console` is disabled
    // 3. does NOT skip the next log following a `tracing::event!()`. These calls
    //    happen for example under the hood in `sqlx`. I don't understand what's
    //    actually causing that but at this point I'm just happy if all the features
    //    work correctly.
    macro_rules! fmt_layer {
        ($env_filter:expr_2021, $stderr_threshold:expr_2021, $use_json_format:expr_2021) => {{
            let writer = std::io::stdout
                .with_min_level(
                    $stderr_threshold
                        .into_level()
                        .unwrap_or(tracing::Level::ERROR),
                )
                .or_else(std::io::stderr);
            let timer = UtcTime::new(format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
            ));

            if use_json_format {
                // structured logging
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(writer)
                    .with_timer(timer)
                    .with_filter($env_filter)
                    .boxed()
            } else {
                tracing_subscriber::fmt::layer()
                    .with_ansi(atty::is(atty::Stream::Stdout))
                    .with_writer(writer)
                    .with_timer(timer)
                    .with_filter($env_filter)
                    .boxed()
            }
        }};
    }

    let enable_tokio_console: bool = std::env::var("TOKIO_CONSOLE")
        .unwrap_or("false".to_string())
        .parse()
        .unwrap();

    if cfg!(tokio_unstable) && enable_tokio_console {
        let (env_filter, reload_handle) =
            tracing_subscriber::reload::Layer::new(EnvFilter::new(&initial_filter));

        tracing_subscriber::registry()
            .with(console_subscriber::spawn())
            .with(fmt_layer!(env_filter, stderr_threshold, use_json_format))
            .with(RequestIdLayer)
            .init();
        tracing::info!("started programm with support for tokio-console");

        if cfg!(unix) {
            spawn_reload_handler(initial_filter, reload_handle);
        }
    } else {
        let (env_filter, reload_handle) =
            tracing_subscriber::reload::Layer::new(EnvFilter::new(&initial_filter));

        tracing_subscriber::registry()
            // Without this the subscriber ignores the next log after an `tracing::event!()` which
            // `sqlx` uses under the hood.
            .with(tracing::level_filters::LevelFilter::TRACE)
            .with(fmt_layer!(env_filter, stderr_threshold, use_json_format))
            .with(RequestIdLayer)
            .init();
        tracing::info!("started programm without support for tokio-console");

        if cfg!(unix) {
            spawn_reload_handler(initial_filter, reload_handle);
        }
    }
}

/// Panic hook that prints roughly the same message as the default panic hook
/// but uses tracing:error instead of stderr.
///
/// Useful when we want panic messages to have the proper log format for Kibana.
fn tracing_panic_hook(panic: &PanicHookInfo) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let backtrace = std::backtrace::Backtrace::force_capture();
    tracing::error!("thread '{name}' {panic}\nstack backtrace:\n{backtrace}");
}
