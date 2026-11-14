//! Turning geometry into pixels.

mod arc;
mod fill;
mod ink;
mod line;
mod painter;
mod spline;
mod trig;

pub(crate) use arc::arc_points;
pub(crate) use fill::fill_polygon;
pub(crate) use ink::Ink;
pub(crate) use painter::Painter;
pub(crate) use spline::span_points;
