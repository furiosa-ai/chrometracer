#![feature(thread_id_value)]

mod tracer;

pub use chrometracer_attributes::instrument;
pub use tracer::{ChromeTracerGuard, SlimEvent, Span, builder, current};
