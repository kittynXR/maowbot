pub mod action;
pub mod filter;
pub mod pipeline;
pub mod builder;
pub mod examples;
pub mod filters;
pub mod actions;

pub use action::{EventAction, ActionResult, ActionContext};
pub use filter::{EventFilter, FilterResult};
pub use pipeline::{EventPipeline, PipelineExecutor, PipelineEventService};
pub use builder::PipelineBuilder;