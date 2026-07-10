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
const RATIO_PANEL_TITLE: &str = "Performance and memory geomean versus QuickJS";
const JETSTREAM_PANEL_TITLE: &str = "JetStream shell latency geomean versus QuickJS";
const TEST262_PANEL_TITLE: &str = "Full Test262 outcomes";

#[derive(Debug, Clone, Copy)]
pub(super) enum ChartTheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
struct ChartPalette {
    background: RGBColor,
    foreground: RGBColor,
    bold_grid: RGBColor,
    light_grid: RGBColor,
    performance: RGBColor,
    memory: RGBColor,
    jetstream: RGBColor,
    passed: RGBColor,
    failed: RGBColor,
    budget: RGBColor,
}

const LIGHT_PALETTE: ChartPalette = ChartPalette {
    background: WHITE,
    foreground: BLACK,
    bold_grid: RGBColor(170, 170, 170),
    light_grid: RGBColor(225, 225, 225),
    performance: BLUE,
    memory: MAGENTA,
    jetstream: CYAN,
    passed: GREEN,
    failed: RED,
    budget: RED,
};

const DARK_PALETTE: ChartPalette = ChartPalette {
    background: RGBColor(18, 18, 18),
    foreground: RGBColor(232, 232, 232),
    bold_grid: RGBColor(86, 86, 86),
    light_grid: RGBColor(45, 45, 45),
    performance: RGBColor(92, 166, 255),
    memory: RGBColor(255, 105, 255),
    jetstream: RGBColor(74, 222, 232),
    passed: RGBColor(90, 230, 110),
    failed: RGBColor(255, 100, 100),
    budget: RGBColor(255, 100, 100),
};

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
        render_chart(&root, records, timeline, &LIGHT_PALETTE)?;
        root.present()
            .map_err(|error| anyhow!("failed to render chart: {error:?}"))?;
    }
    let image = RgbImage::from_raw(CHART_WIDTH, CHART_HEIGHT, buffer)
        .context("failed to create chart image buffer")?;
    image
        .save_with_format(path, ImageFormat::Jpeg)
        .with_context(|| format!("failed to write chart '{}'", path.display()))
}

pub(super) fn write_svg_chart(
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    path: &Path,
    theme: ChartTheme,
) -> anyhow::Result<()> {
    let palette = match theme {
        ChartTheme::Light => LIGHT_PALETTE,
        ChartTheme::Dark => DARK_PALETTE,
    };
    let root = SVGBackend::new(path, (CHART_WIDTH, CHART_HEIGHT)).into_drawing_area();
    render_chart(&root, records, timeline, &palette)?;
    root.present()
        .map_err(|error| anyhow!("failed to render SVG chart: {error:?}"))
}

fn render_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    root.fill(&palette.background)
        .map_err(|error| anyhow!("failed to fill chart background: {error:?}"))?;
    let mut areas = root.split_evenly((3, 1)).into_iter();
    let ratio_area = areas.next().context("missing ratio chart area")?;
    let jetstream_area = areas.next().context("missing JetStream chart area")?;
    let test_area = areas.next().context("missing test coverage chart area")?;
    draw_ratio_panel(&ratio_area, records, timeline, palette)?;
    draw_jetstream_panel(&jetstream_area, records, timeline, palette)?;
    draw_test_panel(&test_area, records, timeline, palette)
}

fn draw_jetstream_panel<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    let points = jetstream_points(records, timeline)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            JETSTREAM_PANEL_TITLE,
            "No JetStream latency data available",
            palette,
        );
    }
    let values = points.iter().map(|point| point.latency);
    let bounds = chart_bounds(values, BUDGET_RATIO)?;
    let x_end = timeline.axis_end()?;
    let mut chart = ChartBuilder::on(area)
        .caption(
            jetstream_panel_title(&points),
            ("sans-serif", 30).into_font().color(&palette.foreground),
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
        .axis_style(palette.foreground)
        .bold_line_style(palette.bold_grid)
        .light_line_style(palette.light_grid)
        .label_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .axis_desc_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw JetStream chart mesh: {error:?}"))?;
    draw_budget_line(&mut chart, x_end, palette)?;
    let series_color = palette.jetstream;
    chart
        .draw_series(LineSeries::new(
            points.iter().map(|point| (point.x, point.latency)),
            series_color.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw JetStream chart series: {error:?}"))?
        .label("JetStream latency geomean")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], series_color.stroke_width(3))
        });
    chart
        .configure_series_labels()
        .background_style(palette.background.mix(0.85))
        .border_style(palette.foreground)
        .label_font(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw JetStream chart legend: {error:?}"))
}

fn draw_ratio_panel<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    let points = ratio_points(records, timeline)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            RATIO_PANEL_TITLE,
            "No QuickJS ratio data available",
            palette,
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
            ratio_panel_title(&points),
            ("sans-serif", 30).into_font().color(&palette.foreground),
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
        .axis_style(palette.foreground)
        .bold_line_style(palette.bold_grid)
        .light_line_style(palette.light_grid)
        .label_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .axis_desc_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw ratio chart mesh: {error:?}"))?;
    draw_budget_line(&mut chart, x_end, palette)?;
    let performance_color = palette.performance;
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .filter_map(|point| point.performance.map(|value| (point.x, value))),
            performance_color.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw performance chart series: {error:?}"))?
        .label("performance geomean")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], performance_color.stroke_width(3))
        });
    let memory_color = palette.memory;
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .filter_map(|point| point.memory.map(|value| (point.x, value))),
            memory_color.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw memory chart series: {error:?}"))?
        .label("memory geomean")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], memory_color.stroke_width(3))
        });
    chart
        .configure_series_labels()
        .background_style(palette.background.mix(0.85))
        .border_style(palette.foreground)
        .label_font(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw ratio chart legend: {error:?}"))
}

fn draw_budget_line<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordi32, RangedCoordf64>>,
    x_end: i32,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    let budget_color = palette.budget;
    chart
        .draw_series(LineSeries::new(
            [(0, BUDGET_RATIO), (x_end.saturating_sub(1), BUDGET_RATIO)],
            budget_color.stroke_width(2),
        ))
        .map_err(|error| anyhow!("failed to draw budget line: {error:?}"))?
        .label("1.00x budget")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], budget_color.stroke_width(2))
        });
    Ok(())
}

fn draw_test_panel<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    records: &[ReportRecord],
    timeline: &CommitTimeline,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    let points = test_points(records, timeline)?;
    if points.is_empty() {
        return draw_empty_panel(
            area,
            TEST262_PANEL_TITLE,
            "No Test262 outcome data available",
            palette,
        );
    }
    let values = points
        .iter()
        .flat_map(|point| [Some(f64::from(point.passed)), Some(f64::from(point.failed))])
        .flatten();
    let bounds = chart_bounds(values, 0.0)?;
    let x_end = timeline.axis_end()?;
    let mut chart = ChartBuilder::on(area)
        .caption(
            test_panel_title(&points),
            ("sans-serif", 30).into_font().color(&palette.foreground),
        )
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
        .axis_style(palette.foreground)
        .bold_line_style(palette.bold_grid)
        .light_line_style(palette.light_grid)
        .label_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .axis_desc_style(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw Test262 chart mesh: {error:?}"))?;
    let passed_color = palette.passed;
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .map(|point| (point.x, f64::from(point.passed))),
            passed_color.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw Test262 passed series: {error:?}"))?
        .label("passed")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], passed_color.stroke_width(3))
        });
    let failed_color = palette.failed;
    chart
        .draw_series(LineSeries::new(
            points
                .iter()
                .map(|point| (point.x, f64::from(point.failed))),
            failed_color.stroke_width(3),
        ))
        .map_err(|error| anyhow!("failed to draw Test262 failed series: {error:?}"))?
        .label("failed")
        .legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 24, y)], failed_color.stroke_width(3))
        });
    chart
        .configure_series_labels()
        .background_style(palette.background.mix(0.85))
        .border_style(palette.foreground)
        .label_font(("sans-serif", 15).into_font().color(&palette.foreground))
        .draw()
        .map_err(|error| anyhow!("failed to draw Test262 chart legend: {error:?}"))
}

fn draw_empty_panel<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    title: &str,
    message: &str,
    palette: &ChartPalette,
) -> anyhow::Result<()> {
    let (width, height) = area.dim_in_pixel();
    let x = i32::try_from(width / 2).context("empty chart x coordinate does not fit i32")?;
    let y = i32::try_from(height / 2).context("empty chart y coordinate does not fit i32")?;
    area.draw(&Text::new(
        title.to_owned(),
        (18, 36),
        ("sans-serif", 30).into_font().color(&palette.foreground),
    ))
    .map_err(|error| anyhow!("failed to draw empty chart title: {error:?}"))?;
    area.draw(&Text::new(
        message.to_owned(),
        (x.saturating_sub(190), y),
        ("sans-serif", 24).into_font().color(&palette.foreground),
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
    total: u32,
    passed: u32,
    failed: u32,
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
                total: value.total,
                passed: value.passed,
                failed: value.failed,
            },
        );
    }
    Ok(points.into_values().collect())
}

fn ratio_panel_title(points: &[RatioPoint]) -> String {
    let performance = points.iter().rev().find_map(|point| point.performance);
    let memory = points.iter().rev().find_map(|point| point.memory);
    match (performance, memory) {
        (Some(performance), Some(memory)) => format!(
            "{RATIO_PANEL_TITLE} (latest: performance {performance:.2}x, memory {memory:.2}x)"
        ),
        (Some(performance), None) => {
            format!("{RATIO_PANEL_TITLE} (latest: performance {performance:.2}x)")
        }
        (None, Some(memory)) => format!("{RATIO_PANEL_TITLE} (latest: memory {memory:.2}x)"),
        (None, None) => RATIO_PANEL_TITLE.to_owned(),
    }
}

fn jetstream_panel_title(points: &[JetStreamPoint]) -> String {
    let Some(latest) = points.last() else {
        return JETSTREAM_PANEL_TITLE.to_owned();
    };
    format!("{JETSTREAM_PANEL_TITLE} (latest: {:.2}x)", latest.latency)
}

fn test_panel_title(points: &[TestPoint]) -> String {
    let Some(latest) = points.last() else {
        return TEST262_PANEL_TITLE.to_owned();
    };
    if latest.total == 0 {
        return format!("{TEST262_PANEL_TITLE} (0 / 0 passed)");
    }
    let pass_rate = f64::from(latest.passed) * 100.0 / f64::from(latest.total);
    format!(
        "{TEST262_PANEL_TITLE} ({} / {} passed, {pass_rate:.2}%)",
        grouped_count(latest.passed),
        grouped_count(latest.total)
    )
}

fn grouped_count(value: u32) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len().saturating_add(digits.len() / 3));
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && digits.len().saturating_sub(position).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
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
    use super::{
        JetStreamPoint, RatioPoint, TestPoint, grouped_count, jetstream_panel_title,
        jetstream_points, ratio_panel_title, ratio_points, test_panel_title, test_points,
    };
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
                .is_some_and(|point| point.x == 5 && point.passed == 40 && point.total == 100)
            && timeline.label(2) == "c2"
            && timeline.description() == "main first-parent commit";
        if valid {
            return Ok(());
        }
        Err("chart series did not preserve the shared sparse commit domain".into())
    }

    #[test]
    fn panel_titles_show_latest_values_without_failed_test_count() -> TestResult {
        let ratios = [
            RatioPoint {
                x: 2,
                performance: Some(1.25),
                memory: Some(0.98),
            },
            RatioPoint {
                x: 5,
                performance: Some(1.10),
                memory: None,
            },
        ];
        let jetstream = [JetStreamPoint {
            x: 7,
            latency: 22.125,
        }];
        let tests = [TestPoint {
            x: 9,
            total: 102_578,
            passed: 36_553,
            failed: 66_025,
        }];
        let valid = ratio_panel_title(&ratios)
            .ends_with("(latest: performance 1.10x, memory 0.98x)")
            && jetstream_panel_title(&jetstream).ends_with("(latest: 22.12x)")
            && test_panel_title(&tests)
                == "Full Test262 outcomes (36,553 / 102,578 passed, 35.63%)"
            && !test_panel_title(&tests).contains("66,025")
            && grouped_count(1_234_567) == "1,234,567";
        if valid {
            return Ok(());
        }
        Err("chart panel titles did not expose the intended latest values".into())
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
