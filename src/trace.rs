// To filter aws sdk logs, see: https://docs.aws.amazon.com/sdk-for-rust/latest/dg/logging.html#logging-filtering

use clap_verbosity_flag::{LogLevel, Verbosity};
use tracing::{level_filters::LevelFilter, Level};
use tracing_log::AsTrace as _;
use tracing_subscriber::{
    fmt::{
        format::{DefaultFields, Format, Full},
        time::ChronoLocal,
        SubscriberBuilder,
    },
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_fmt_with_json<L: LogLevel>(v: &Verbosity<L>) {
    init_subscriber(v, |b| b.json());
}

pub fn init_fmt_with_pretty<L: LogLevel>(v: &Verbosity<L>) {
    init_subscriber(v, |b| b.pretty());
}

pub fn init_fmt_with_full<L: LogLevel>(v: &Verbosity<L>) {
    init_subscriber(v, |b| b.with_ansi(false));
}

type DefaultSubscriberBuilder =
    SubscriberBuilder<DefaultFields, Format<Full, ChronoLocal>, EnvFilter>;

fn init_subscriber<L, F, B>(v: &Verbosity<L>, f: F)
where
    L: LogLevel,
    F: FnOnce(DefaultSubscriberBuilder) -> B,
    B: SubscriberInitExt,
{
    // Don't set subscriber if user wants to silence output.
    match v.log_level_filter().as_trace() {
        LevelFilter::OFF => (),
        filter => {
            let env_filter = into_env_filter(filter);
            let builder = SubscriberBuilder::default()
                .with_timer(ChronoLocal::rfc_3339())
                .with_env_filter(env_filter);
            f(builder).init();
        }
    }
}

fn into_env_filter(filter: LevelFilter) -> EnvFilter {
    // If log level is lower than debug, only apply it to orgu targets.
    let default = if filter >= Level::DEBUG {
        format!("info,orgu={filter}")
    } else {
        filter.to_string()
    };
    EnvFilter::try_from_default_env().unwrap_or_else(|_| default.into())
}
