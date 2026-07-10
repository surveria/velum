use std::{collections::BTreeMap, ops::Range, path::Path};

use anyhow::{Context as _, anyhow, bail};
use image::{ImageFormat, RgbImage};
use plotters::{
    coord::{
        Shift,
        types::{RangedCoordf64, RangedCoordi32},
    },
    prelude::*,
};

use super::{ReportRecord, report_rollup_timeline::CommitTimeline};

const BUDGET_RATIO: f64 = 1.00;
const CHART_WIDTH: u32 = 1400;
const CHART_HEIGHT: u32 = 1200;
const RGB_CHANNELS: usize = 3;

pub(super) fn write_chart(
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    path: &Path,
) -> anyhow::Result<()> {
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
        let mut areas = root.split_evenly((3, 1)).into_iter();
        let ratio_area = areas.next().context("missing ratio chart area")?;
        let jetstream_area = areas.next().context("missing JetStream chart area")?;
        let test_area = areas.next().context("missing test coverage chart area")?;
        draw_ratio_panel(&ratio_area, records, timeline)?;
        draw_jetstream_panel(&jetstream_area, records, timeline)?;
        draw_test_panel(&test_area, records, timeline)?;
        root.present()
            .map_err(|error| anyhow!("failed to render chart: {error:?}"))?;
    }
    let image = RgbImage::from_raw(CHART_WIDTH, CHART_HEIGHT, buffer)
        .context("failed to create chart image buffer")?;
    image
        .save_with_format(path, ImageFormat::Jpeg)
        .with_context(|| format!("failed to write chart '{}'", path.display()))
}

fn draw_jetstream_panel(
    area: &DrawingArea<BitMapBackend<'_>, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
) -> anyhow::Result<()> {
    let points = jetstream_points(records, timeline)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            "JetStream shell latency geomean versus QuickJS",
            "No JetStream latency data available",
        );
    }
    let values = points.iter().map(|point| point.latency);
    let bounds = chart_bounds(values, BUDGET_RATIO)?;
    let x_end = timeline.axis_end()?;
    let mut chart = ChartBuilder::on(area)
        .caption(
            "JetStream shell latency geomean versus QuickJS",
            ("sans-serif", 30).into_font(),
        )
        .margin(18)
        .x_label_area_size(34)
        .y_label_area_size(60)
        .build_cartesian_2d(0..x_end, bounds)
        .map_err(|error| anyhow!("failed to build JetStream chart: {error:?}"))?;
    let x_label_formatter = |value: &i32| timeline.label(*value);
    chart
        .configure_mesh()
        .x_labels(9)
        .x_label_formatter(&x_label_formatter)
        .x_desc(timeline.description())
        .y_desc("latency ratio")
        .draw()
        .map_err(|error| anyhow!("failed to draw JetStream chart mesh: {error:?}"))?;
    draw_budget_line(&mut chart, x_end)?;
    chart
        .draw_series(LineSeries::new(
            points.iter().map(|point| (point.x, point.latency)),
            CYAN.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw JetStream chart series: {error:?}"))?
        .label("JetStream latency geomean")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], CYAN.stroke_width(3)));
    chart
        .draw_series(
            points
                .iter()
                .map(|point| Circle::new((point.x, point.latency), 4, CYAN.filled())),
        )
        .map_err(|error| anyhow!("failed to draw JetStream chart points: {error:?}"))?;
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.85))
        .border_style(BLACK)
        .draw()
        .map_err(|error| anyhow!("failed to draw JetStream chart legend: {error:?}"))
}

fn draw_ratio_panel(
    area: &DrawingArea<BitMapBackend<'_>, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
) -> anyhow::Result<()> {
    let points = ratio_points(records, timeline)?;
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
    let x_end = timeline.axis_end()?;
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
    let x_label_formatter = |value: &i32| timeline.label(*value);
    chart
        .configure_mesh()
        .x_labels(9)
        .x_label_formatter(&x_label_formatter)
        .x_desc(timeline.description())
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
        .draw_series(points.iter().filter_map(|point| {
            point
                .performance
                .map(|value| Circle::new((point.x, value), 4, BLUE.filled()))
        }))
        .map_err(|error| anyhow!("failed to draw performance chart points: {error:?}"))?;
    chart
        .draw_series(points.iter().filter_map(|point| {
            point
                .memory
                .map(|value| Circle::new((point.x, value), 4, MAGENTA.filled()))
        }))
        .map_err(|error| anyhow!("failed to draw memory chart points: {error:?}"))?;
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
    timeline: &CommitTimeline,
) -> anyhow::Result<()> {
    let points = test_points(records, timeline)?;
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
    let x_end = timeline.axis_end()?;
    let mut chart = ChartBuilder::on(area)
        .caption("Full Test262 outcomes", ("sans-serif", 30).into_font())
        .margin(18)
        .x_label_area_size(34)
        .y_label_area_size(60)
        .build_cartesian_2d(0..x_end, bounds)
        .map_err(|error| anyhow!("failed to build Test262 chart: {error:?}"))?;
    let x_label_formatter = |value: &i32| timeline.label(*value);
    chart
        .configure_mesh()
        .x_labels(9)
        .x_label_formatter(&x_label_formatter)
        .x_desc(timeline.description())
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
        .draw_series(
            points
                .iter()
                .map(|point| Circle::new((point.x, point.passed), 4, GREEN.filled())),
        )
        .map_err(|error| anyhow!("failed to draw Test262 passed points: {error:?}"))?;
    chart
        .draw_series(
            points
                .iter()
                .map(|point| Circle::new((point.x, point.failed), 4, RED.filled())),
        )
        .map_err(|error| anyhow!("failed to draw Test262 failed points: {error:?}"))?;
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
struct JetStreamPoint {
    x: i32,
    latency: f64,
}

#[derive(Debug, Clone, Copy)]
struct TestPoint {
    x: i32,
    passed: f64,
    failed: f64,
}

fn ratio_points(
    records: &[ReportRecord],
    timeline: &CommitTimeline,
) -> anyhow::Result<Vec<RatioPoint>> {
    let mut points = BTreeMap::new();
    for record in records {
        if record.latency_geomean.is_none() && record.memory_geomean.is_none() {
            continue;
        }
        let x = timeline.position(record)?;
        let point = points.entry(x).or_insert(RatioPoint {
            x,
            performance: None,
            memory: None,
        });
        if record.latency_geomean.is_some() {
            point.performance = record.latency_geomean;
        }
        if record.memory_geomean.is_some() {
            point.memory = record.memory_geomean;
        }
    }
    Ok(points.into_values().collect())
}

fn jetstream_points(
    records: &[ReportRecord],
    timeline: &CommitTimeline,
) -> anyhow::Result<Vec<JetStreamPoint>> {
    let mut points = BTreeMap::new();
    for record in records {
        let Some(latency) = record.jetstream_latency_geomean else {
            continue;
        };
        let x = timeline.position(record)?;
        points.insert(x, JetStreamPoint { x, latency });
    }
    Ok(points.into_values().collect())
}

fn test_points(
    records: &[ReportRecord],
    timeline: &CommitTimeline,
) -> anyhow::Result<Vec<TestPoint>> {
    let mut points = BTreeMap::new();
    for record in records {
        let Some(value) = record.full_test262 else {
            continue;
        };
        let x = timeline.position(record)?;
        points.insert(
            x,
            TestPoint {
                x,
                passed: f64::from(value.passed),
                failed: f64::from(value.failed),
            },
        );
    }
    Ok(points.into_values().collect())
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

#[cfg(test)]
mod tests {
    use super::{jetstream_points, ratio_points, test_points};
    use crate::report_rollup::{
        ReportContext, ReportRecord, TestCounts, report_rollup_timeline::CommitTimeline,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn all_series_use_shared_commit_positions_and_collapse_duplicate_commits() -> TestResult {
        let timeline = CommitTimeline::for_test(
            12,
            &[
                ("performance-a.md", 2),
                ("test262.md", 5),
                ("performance-old.md", 9),
                ("performance-new.md", 9),
                ("jetstream.yaml", 9),
            ],
        );
        let records = vec![
            record("performance-a.md", Some(1.2), None, None),
            record(
                "test262.md",
                None,
                None,
                Some(TestCounts {
                    total: 100,
                    passed: 40,
                    failed: 60,
                }),
            ),
            record("performance-old.md", Some(1.4), None, None),
            record("performance-new.md", Some(1.1), None, None),
            record("jetstream.yaml", None, Some(22.0), None),
        ];

        let ratios = ratio_points(&records, &timeline)?;
        let jetstream = jetstream_points(&records, &timeline)?;
        let tests = test_points(&records, &timeline)?;
        let valid = timeline.axis_end()? == 12
            && ratios.len() == 2
            && ratios
                .first()
                .is_some_and(|point| point.x == 2 && point.performance == Some(1.2))
            && ratios
                .get(1)
                .is_some_and(|point| point.x == 9 && point.performance == Some(1.1))
            && jetstream
                .first()
                .is_some_and(|point| point.x == 9 && (point.latency - 22.0).abs() < f64::EPSILON)
            && tests
                .first()
                .is_some_and(|point| point.x == 5 && (point.passed - 40.0).abs() < f64::EPSILON)
            && timeline.label(2) == "c2"
            && timeline.description() == "main first-parent commit";
        if valid {
            return Ok(());
        }
        Err("chart series did not preserve the shared sparse commit domain".into())
    }

    fn record(
        file_name: &str,
        performance: Option<f64>,
        jetstream: Option<f64>,
        full_test262: Option<TestCounts>,
    ) -> ReportRecord {
        ReportRecord {
            file_name: file_name.to_owned(),
            timestamp: String::new(),
            benchmark_count: usize::from(performance.is_some()),
            latency_geomean: performance,
            memory_geomean: None,
            jetstream_count: usize::from(jetstream.is_some()),
            jetstream_latency_geomean: jetstream,
            latency_over: 0,
            memory_over: 0,
            jetstream_latency_over: 0,
            benchmark_report: performance.is_some(),
            jetstream_report: jetstream.is_some(),
            full_test262,
            context: ReportContext::default(),
        }
    }
}
