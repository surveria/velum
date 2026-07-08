use std::{ops::Range, path::Path};

use anyhow::{Context as _, anyhow, bail};
use image::{ImageFormat, RgbImage};
use plotters::{
    coord::{
        Shift,
        types::{RangedCoordf64, RangedCoordi32},
    },
    prelude::*,
};

use super::ReportRecord;

const BUDGET_RATIO: f64 = 1.00;
const CHART_WIDTH: u32 = 1400;
const CHART_HEIGHT: u32 = 900;
const RGB_CHANNELS: usize = 3;

pub(super) fn write_chart(records: &[ReportRecord], path: &Path) -> anyhow::Result<()> {
    let width = usize::try_from(CHART_WIDTH).context("chart width does not fit usize")?;
    let height = usize::try_from(CHART_HEIGHT).context("chart height does not fit usize")?;
    let pixel_count = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(RGB_CHANNELS))
        .context("chart buffer size overflow")?;
    let mut buffer = vec![255u8; pixel_count];
    {
        let root = BitMapBackend::with_buffer(&mut buffer, (CHART_WIDTH, CHART_HEIGHT))
            .into_drawing_area();
        root.fill(&WHITE)
            .map_err(|error| anyhow!("failed to fill chart background: {error:?}"))?;
        let mut areas = root.split_evenly((2, 1)).into_iter();
        let ratio_area = areas.next().context("missing ratio chart area")?;
        let test_area = areas.next().context("missing test coverage chart area")?;
        draw_ratio_panel(&ratio_area, records)?;
        draw_test_panel(&test_area, records)?;
        root.present()
            .map_err(|error| anyhow!("failed to render chart: {error:?}"))?;
    }
    let image = RgbImage::from_raw(CHART_WIDTH, CHART_HEIGHT, buffer)
        .context("failed to create chart image buffer")?;
    image
        .save_with_format(path, ImageFormat::Jpeg)
        .with_context(|| format!("failed to write chart '{}'", path.display()))
}

fn draw_ratio_panel(
    area: &DrawingArea<BitMapBackend<'_>, Shift>,
    records: &[ReportRecord],
) -> anyhow::Result<()> {
    let points = ratio_points(records)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            "Performance and memory geomean versus QuickJS",
            "No QuickJS ratio data available",
        );
    }
    let values = points
        .iter()
        .flat_map(|point| [point.performance, point.memory])
        .flatten();
    let bounds = chart_bounds(values, BUDGET_RATIO)?;
    let x_end = x_axis_end(points.len())?;
    let mut chart = ChartBuilder::on(area)
        .caption(
            "Performance and memory geomean versus QuickJS",
            ("sans-serif", 30).into_font(),
        )
        .margin(18)
        .x_label_area_size(34)
        .y_label_area_size(60)
        .build_cartesian_2d(0..x_end, bounds)
        .map_err(|error| anyhow!("failed to build ratio chart: {error:?}"))?;
    chart
        .configure_mesh()
        .x_desc("measured report order")
        .y_desc("ratio")
        .draw()
        .map_err(|error| anyhow!("failed to draw ratio chart mesh: {error:?}"))?;
    draw_budget_line(&mut chart, x_end)?;
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .filter_map(|point| point.performance.map(|value| (point.x, value))),
            BLUE.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw performance chart series: {error:?}"))?
        .label("performance geomean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], BLUE.stroke_width(3)));
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .filter_map(|point| point.memory.map(|value| (point.x, value))),
            MAGENTA.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw memory chart series: {error:?}"))?
        .label("memory geomean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], MAGENTA.stroke_width(3)));
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.85))
        .border_style(BLACK)
        .draw()
        .map_err(|error| anyhow!("failed to draw ratio chart legend: {error:?}"))
}

fn draw_budget_line(
    chart: &mut ChartContext<'_, BitMapBackend<'_>, Cartesian2d<RangedCoordi32, RangedCoordf64>>,
    x_end: i32,
) -> anyhow::Result<()> {
    chart
        .draw_series(LineSeries::new(
            [(0, BUDGET_RATIO), (x_end.saturating_sub(1), BUDGET_RATIO)],
            RED.stroke_width(2),
        ))
        .map_err(|error| anyhow!("failed to draw budget line: {error:?}"))?
        .label("1.00x budget")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], RED.stroke_width(2)));
    Ok(())
}

fn draw_test_panel(
    area: &DrawingArea<BitMapBackend<'_>, Shift>,
    records: &[ReportRecord],
) -> anyhow::Result<()> {
    let points = test_points(records)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            "Full Test262 outcomes",
            "No Test262 outcome data available",
        );
    }
    let values = points
        .iter()
        .flat_map(|point| [Some(point.passed), Some(point.failed)])
        .flatten();
    let bounds = chart_bounds(values, 0.0)?;
    let x_end = x_axis_end(points.len())?;
    let mut chart = ChartBuilder::on(area)
        .caption("Full Test262 outcomes", ("sans-serif", 30).into_font())
        .margin(18)
        .x_label_area_size(34)
        .y_label_area_size(60)
        .build_cartesian_2d(0..x_end, bounds)
        .map_err(|error| anyhow!("failed to build Test262 chart: {error:?}"))?;
    chart
        .configure_mesh()
        .x_desc("measured report order")
        .y_desc("cases")
        .y_label_formatter(&|value| format!("{value:.0}"))
        .draw()
        .map_err(|error| anyhow!("failed to draw Test262 chart mesh: {error:?}"))?;
    chart
        .draw_series(LineSeries::new(
            points.iter().map(|point| (point.x, point.passed)),
            GREEN.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw Test262 passed series: {error:?}"))?
        .label("passed")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], GREEN.stroke_width(3)));
    chart
        .draw_series(LineSeries::new(
            points.iter().map(|point| (point.x, point.failed)),
            RED.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw Test262 failed series: {error:?}"))?
        .label("failed")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], RED.stroke_width(3)));
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.85))
        .border_style(BLACK)
        .draw()
        .map_err(|error| anyhow!("failed to draw Test262 chart legend: {error:?}"))
}

fn draw_empty_panel(
    area: &DrawingArea<BitMapBackend<'_>, Shift>,
    title: &str,
    message: &str,
) -> anyhow::Result<()> {
    let (width, height) = area.dim_in_pixel();
    let x = i32::try_from(width / 2).context("empty chart x coordinate does not fit i32")?;
    let y = i32::try_from(height / 2).context("empty chart y coordinate does not fit i32")?;
    area.draw(&Text::new(
        title.to_owned(),
        (18, 36),
        ("sans-serif", 30).into_font(),
    ))
    .map_err(|error| anyhow!("failed to draw empty chart title: {error:?}"))?;
    area.draw(&Text::new(
        message.to_owned(),
        (x.saturating_sub(190), y),
        ("sans-serif", 24).into_font(),
    ))
    .map_err(|error| anyhow!("failed to draw empty chart message: {error:?}"))
}

#[derive(Debug, Clone, Copy)]
struct RatioPoint {
    x: i32,
    performance: Option<f64>,
    memory: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct TestPoint {
    x: i32,
    passed: f64,
    failed: f64,
}

fn ratio_points(records: &[ReportRecord]) -> anyhow::Result<Vec<RatioPoint>> {
    let mut points = Vec::new();
    for record in records {
        if record.latency_geomean.is_none() && record.memory_geomean.is_none() {
            continue;
        }
        points.push(RatioPoint {
            x: i32::try_from(points.len()).context("too many reports to plot")?,
            performance: record.latency_geomean,
            memory: record.memory_geomean,
        });
    }
    Ok(points)
}

fn test_points(records: &[ReportRecord]) -> anyhow::Result<Vec<TestPoint>> {
    let mut points = Vec::new();
    for record in records {
        let Some(value) = record.full_test262 else {
            continue;
        };
        points.push(TestPoint {
            x: i32::try_from(points.len()).context("too many reports to plot")?,
            passed: f64::from(value.passed),
            failed: f64::from(value.failed),
        });
    }
    Ok(points)
}

fn chart_bounds(values: impl Iterator<Item = f64>, anchor: f64) -> anyhow::Result<Range<f64>> {
    let mut min_value = anchor;
    let mut max_value = anchor;
    let mut seen = false;
    for value in values {
        min_value = min_value.min(value);
        max_value = max_value.max(value);
        seen = true;
    }
    if !seen {
        bail!("cannot build chart bounds without data");
    }
    let span = (max_value - min_value).max(0.01);
    let padding = (span * 0.08).max(0.05);
    Ok((min_value - padding)..(max_value + padding))
}

fn x_axis_end(point_count: usize) -> anyhow::Result<i32> {
    let count = i32::try_from(point_count).context("too many chart points")?;
    Ok(count.max(1))
}
