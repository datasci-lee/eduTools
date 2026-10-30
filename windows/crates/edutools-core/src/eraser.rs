//! 부분·획·영역 지우개의 OS 독립 판정.

use crate::ink::{
    InkGeometry, InkPoint, InkShapeKind, InkStroke, InkStrokeContent, InkStrokeTool, Point, Rect,
    Size,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

const INTERSECTION_EPSILON: f64 = 0.000_001;

/// Swift: EraserMode
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EraserMode {
    Partial,
    Stroke,
    Area,
}

/// Swift: EraserHitRule
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EraserHitRule {
    Touching,
    Contained,
}

/// Swift: EraserTargetFilter
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EraserTargetFilter {
    All,
    PenOnly,
    HighlighterOnly,
}

impl EraserTargetFilter {
    /// Swift: EraserTargetFilter.includes(_:)
    pub fn includes(self, tool: InkStrokeTool) -> bool {
        match self {
            Self::All => true,
            Self::PenOnly => tool == InkStrokeTool::Pen,
            Self::HighlighterOnly => tool == InkStrokeTool::Highlighter,
        }
    }
}

/// Swift: EraserAreaShape
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EraserAreaShape {
    Rectangle,
    Lasso,
}

impl EraserAreaShape {
    /// Swift: EraserAreaShape.next
    pub fn next(self) -> Self {
        match self {
            Self::Rectangle => Self::Lasso,
            Self::Lasso => Self::Rectangle,
        }
    }
}

/// Swift: AppState.toggleEraserAreaShape()
pub fn toggle_eraser_area_shape(shape: EraserAreaShape) -> EraserAreaShape {
    shape.next()
}

/// Swift: EraserSettings
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EraserSettings {
    pub diameter: f64,
    pub mode: EraserMode,
    pub hit_rule: EraserHitRule,
    pub target_filter: EraserTargetFilter,
    pub area_shape: EraserAreaShape,
}

/// Swift: EraserGesture
#[derive(Clone, Debug, PartialEq)]
pub enum EraserGesture {
    Sweep(Vec<Point>),
    Area(Rect),
    Lasso(Vec<Point>),
}

/// Swift: EraserEngine
pub struct EraserEngine;

impl EraserEngine {
    /// Swift: EraserEngine.erase(strokes:gesture:settings:canvasSize:pressureMinimumWidthRatio:)
    pub fn erase(
        strokes: &[InkStroke],
        gesture: &EraserGesture,
        settings: &EraserSettings,
        canvas_size: Size,
        pressure_minimum_width_ratio: f64,
    ) -> Vec<InkStroke> {
        if canvas_size.width <= 0.0 || canvas_size.height <= 0.0 {
            return strokes.to_vec();
        }

        match (settings.mode, gesture) {
            (EraserMode::Partial, EraserGesture::Sweep(points)) => erase_partial_strokes(
                strokes,
                points,
                settings,
                canvas_size,
                pressure_minimum_width_ratio,
            ),
            (EraserMode::Stroke, EraserGesture::Sweep(points)) => {
                erase_whole_strokes(strokes, points, settings, canvas_size)
            }
            (EraserMode::Area, EraserGesture::Area(rect)) => {
                erase_strokes_in_rect(strokes, standardized(*rect), settings, canvas_size)
            }
            (EraserMode::Area, EraserGesture::Lasso(points)) => {
                erase_strokes_in_lasso(strokes, points, settings, canvas_size)
            }
            _ => strokes.to_vec(),
        }
    }
}

/// Swift: EraserEngine.erasePartialStrokes(_:along:settings:canvasSize:pressureMinimumWidthRatio:)
fn erase_partial_strokes(
    strokes: &[InkStroke],
    eraser_points: &[Point],
    settings: &EraserSettings,
    canvas_size: Size,
    pressure_minimum_width_ratio: f64,
) -> Vec<InkStroke> {
    if eraser_points.is_empty() {
        return strokes.to_vec();
    }
    let radius = settings.diameter / 2.0;
    let Some(eraser_bounds) = bounds_of_points(eraser_points).map(|bounds| bounds.inset(-radius))
    else {
        return strokes.to_vec();
    };
    let mut result = Vec::new();

    for stroke in strokes {
        let fragments = settings
            .target_filter
            .includes(stroke.tool)
            .then(|| stroke_bounds(stroke, canvas_size))
            .flatten()
            .filter(|bounds| bounds.overlaps_or_touches(eraser_bounds))
            .and_then(|_| {
                partial_fragments(
                    stroke,
                    eraser_points,
                    radius,
                    canvas_size,
                    pressure_minimum_width_ratio,
                )
            });
        if let Some(fragments) = fragments {
            result.extend(fragments);
        } else {
            result.push(stroke.clone());
        }
    }
    result
}

/// Swift: EraserEngine.partialFragments(of:along:radius:canvasSize:pressureMinimumWidthRatio:)
fn partial_fragments(
    stroke: &InkStroke,
    eraser_points: &[Point],
    radius: f64,
    canvas_size: Size,
    pressure_minimum_width_ratio: f64,
) -> Option<Vec<InkStroke>> {
    if stroke.content != InkStrokeContent::Freehand {
        let item_points = item_screen_points(stroke, canvas_size);
        return (distance_between(&item_points, eraser_points) <= radius + stroke.width / 2.0)
            .then(Vec::new);
    }

    let samples = resampled_points(&stroke.points, canvas_size, (radius / 2.0).max(1.0));
    if samples.is_empty() {
        return None;
    }

    let mut groups: Vec<Vec<InkPoint>> = Vec::new();
    let mut current_group = Vec::new();
    let mut removed_point = false;
    for point in samples {
        let screen_point = InkGeometry::screen_point(point.location, canvas_size);
        let stroke_radius =
            stroke.effective_width(point.pressure, pressure_minimum_width_ratio) / 2.0;
        if distance_from_point_to_polyline(screen_point, eraser_points) <= radius + stroke_radius {
            removed_point = true;
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
        } else {
            current_group.push(point);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }
    if !removed_point {
        return None;
    }

    Some(
        groups
            .into_iter()
            .enumerate()
            .map(|(index, points)| {
                // Swift creates a UUID here. The core has no random source, so the removed
                // identifier and fragment order form a stable identifier without hiding I/O.
                InkStroke::new(
                    format!("{}:fragment:{index}", stroke.identifier),
                    stroke.tool,
                    stroke.color,
                    stroke.width,
                    stroke.uses_pressure,
                    points,
                    InkStrokeContent::Freehand,
                )
            })
            .collect(),
    )
}

/// Swift: EraserEngine.eraseWholeStrokes(_:along:settings:canvasSize:)
fn erase_whole_strokes(
    strokes: &[InkStroke],
    eraser_points: &[Point],
    settings: &EraserSettings,
    canvas_size: Size,
) -> Vec<InkStroke> {
    if eraser_points.is_empty() {
        return strokes.to_vec();
    }
    let radius = settings.diameter / 2.0;
    let Some(eraser_bounds) = bounds_of_points(eraser_points).map(|bounds| bounds.inset(-radius))
    else {
        return strokes.to_vec();
    };

    strokes
        .iter()
        .filter(|stroke| {
            if !settings.target_filter.includes(stroke.tool) {
                return true;
            }
            let Some(screen_bounds) = stroke_bounds(stroke, canvas_size) else {
                return true;
            };
            if !screen_bounds.overlaps_or_touches(eraser_bounds) {
                return true;
            }

            let stroke_points: Vec<_> = stroke
                .points
                .iter()
                .map(|point| InkGeometry::screen_point(point.location, canvas_size))
                .collect();
            let should_erase = match settings.hit_rule {
                EraserHitRule::Touching => {
                    distance_between(&stroke_points, eraser_points) <= radius + stroke.width / 2.0
                }
                EraserHitRule::Contained => {
                    let samples =
                        resampled_points(&stroke.points, canvas_size, (radius / 2.0).max(1.0));
                    let available_radius = radius - stroke.width / 2.0;
                    available_radius >= 0.0
                        && !samples.is_empty()
                        && samples.iter().all(|point| {
                            distance_from_point_to_polyline(
                                InkGeometry::screen_point(point.location, canvas_size),
                                eraser_points,
                            ) <= available_radius
                        })
                }
            };
            !should_erase
        })
        .cloned()
        .collect()
}

/// Swift: EraserEngine.eraseStrokes(_:in:settings:canvasSize:) with CGRect
fn erase_strokes_in_rect(
    strokes: &[InkStroke],
    area: Bounds,
    settings: &EraserSettings,
    canvas_size: Size,
) -> Vec<InkStroke> {
    strokes
        .iter()
        .filter(|stroke| {
            if !settings.target_filter.includes(stroke.tool) {
                return true;
            }
            let Some(screen_bounds) = stroke_bounds(stroke, canvas_size) else {
                return true;
            };
            if !screen_bounds.overlaps_or_touches(area) {
                return true;
            }
            let should_erase = match settings.hit_rule {
                EraserHitRule::Touching => {
                    let hit_area = area.inset(-stroke.width / 2.0);
                    polyline_intersects_rect(&item_screen_points(stroke, canvas_size), hit_area)
                }
                EraserHitRule::Contained => area.contains_bounds(screen_bounds),
            };
            !should_erase
        })
        .cloned()
        .collect()
}

/// Swift: EraserEngine.eraseStrokes(_:in:settings:canvasSize:) with lasso
fn erase_strokes_in_lasso(
    strokes: &[InkStroke],
    lasso: &[Point],
    settings: &EraserSettings,
    canvas_size: Size,
) -> Vec<InkStroke> {
    strokes
        .iter()
        .filter(|stroke| {
            !settings.target_filter.includes(stroke.tool)
                || !lasso_selects(stroke, lasso, settings.hit_rule, canvas_size)
        })
        .cloned()
        .collect()
}

/// Swift: InkItemGeometry.screenPoints(for:canvasSize:)
fn item_screen_points(stroke: &InkStroke, canvas_size: Size) -> Vec<Point> {
    let points: Vec<_> = stroke
        .points
        .iter()
        .map(|point| InkGeometry::screen_point(point.location, canvas_size))
        .collect();
    let (Some(first), Some(last)) = (points.first().copied(), points.last().copied()) else {
        return Vec::new();
    };

    match &stroke.content {
        InkStrokeContent::Freehand => points,
        InkStrokeContent::Shape(InkShapeKind::Line | InkShapeKind::Arrow(_)) => {
            vec![first, last]
        }
        InkStrokeContent::Shape(InkShapeKind::Rectangle)
        | InkStrokeContent::Text(_)
        | InkStrokeContent::Cover => rectangle_polyline(first, last),
        InkStrokeContent::Shape(InkShapeKind::Ellipse) => {
            let center = Point {
                x: (first.x + last.x) / 2.0,
                y: (first.y + last.y) / 2.0,
            };
            let radius_x = (last.x - first.x).abs() / 2.0;
            let radius_y = (last.y - first.y).abs() / 2.0;
            (0..=32)
                .map(|index| {
                    let angle = index as f64 / 32.0 * 2.0 * PI;
                    Point {
                        x: center.x + angle.cos() * radius_x,
                        y: center.y + angle.sin() * radius_y,
                    }
                })
                .collect()
        }
    }
}

/// Swift: InkItemGeometry.screenPoints(for:canvasSize:) rectangle/text/cover branch
fn rectangle_polyline(first: Point, last: Point) -> Vec<Point> {
    let bounds = Bounds::from_points(first, last);
    vec![
        Point {
            x: bounds.min_x,
            y: bounds.min_y,
        },
        Point {
            x: bounds.max_x,
            y: bounds.min_y,
        },
        Point {
            x: bounds.max_x,
            y: bounds.max_y,
        },
        Point {
            x: bounds.min_x,
            y: bounds.max_y,
        },
        Point {
            x: bounds.min_x,
            y: bounds.min_y,
        },
    ]
}

/// Swift: InkItemGeometry.bounds(for:canvasSize:)
fn stroke_bounds(stroke: &InkStroke, canvas_size: Size) -> Option<Bounds> {
    bounds_of_points(&item_screen_points(stroke, canvas_size))
        .map(|bounds| bounds.inset(-stroke.width / 2.0))
}

/// Swift: EraserEngine.resampledPoints(_:canvasSize:maximumSpacing:)
fn resampled_points(points: &[InkPoint], canvas_size: Size, maximum_spacing: f64) -> Vec<InkPoint> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut result = vec![first.clone()];
    for pair in points.windows(2) {
        let start = &pair[0];
        let end = &pair[1];
        let start_point = InkGeometry::screen_point(start.location, canvas_size);
        let end_point = InkGeometry::screen_point(end.location, canvas_size);
        let segment_length = (end_point.x - start_point.x).hypot(end_point.y - start_point.y);
        let steps = (segment_length / maximum_spacing).ceil().max(1.0) as usize;
        for step in 1..=steps {
            let fraction = step as f64 / steps as f64;
            result.push(InkPoint::new(
                Point {
                    x: start.location.x + (end.location.x - start.location.x) * fraction,
                    y: start.location.y + (end.location.y - start.location.y) * fraction,
                },
                start.pressure + (end.pressure - start.pressure) * fraction,
                start.timestamp + (end.timestamp - start.timestamp) * fraction,
            ));
        }
    }
    result
}

/// Swift: CGRect helper values used by EraserEngine
#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl Bounds {
    /// Swift: CGRect.standardized
    fn from_points(first: Point, second: Point) -> Self {
        Self {
            min_x: first.x.min(second.x),
            max_x: first.x.max(second.x),
            min_y: first.y.min(second.y),
            max_y: first.y.max(second.y),
        }
    }

    /// Swift: CGRect.insetBy(dx:dy:)
    fn inset(self, amount: f64) -> Self {
        Self {
            min_x: self.min_x + amount,
            max_x: self.max_x - amount,
            min_y: self.min_y + amount,
            max_y: self.max_y - amount,
        }
    }

    /// Swift: CGRect.contains(_:)
    fn contains_point(self, point: Point) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
    }

    /// Swift: EraserEngine.contains(_:_:)
    fn contains_bounds(self, inner: Self) -> bool {
        inner.min_x >= self.min_x
            && inner.max_x <= self.max_x
            && inner.min_y >= self.min_y
            && inner.max_y <= self.max_y
    }

    /// Swift: EraserEngine.overlapsOrTouches(_:_:)
    fn overlaps_or_touches(self, other: Self) -> bool {
        self.max_x >= other.min_x
            && self.min_x <= other.max_x
            && self.max_y >= other.min_y
            && self.min_y <= other.max_y
    }
}

/// Swift: EraserEngine.standardized(_:)
fn standardized(rect: Rect) -> Bounds {
    Bounds::from_points(
        Point {
            x: rect.x,
            y: rect.y,
        },
        Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
    )
}

/// Swift: EraserEngine.bounds(of:)
fn bounds_of_points(points: &[Point]) -> Option<Bounds> {
    let first = *points.first()?;
    Some(
        points
            .iter()
            .skip(1)
            .fold(Bounds::from_points(first, first), |bounds, point| Bounds {
                min_x: bounds.min_x.min(point.x),
                max_x: bounds.max_x.max(point.x),
                min_y: bounds.min_y.min(point.y),
                max_y: bounds.max_y.max(point.y),
            }),
    )
}

/// Swift: EraserEngine.polyline(_:intersects:)
fn polyline_intersects_rect(points: &[Point], rect: Bounds) -> bool {
    let Some(first) = points.first().copied() else {
        return false;
    };
    if rect.contains_point(first) {
        return true;
    }
    points
        .windows(2)
        .any(|pair| rect.contains_point(pair[1]) || segment_intersects_rect(pair[0], pair[1], rect))
}

/// Swift: EraserEngine.segment(_:_:intersects:)
fn segment_intersects_rect(start: Point, end: Point, rect: Bounds) -> bool {
    let corners = [
        Point {
            x: rect.min_x,
            y: rect.min_y,
        },
        Point {
            x: rect.max_x,
            y: rect.min_y,
        },
        Point {
            x: rect.max_x,
            y: rect.max_y,
        },
        Point {
            x: rect.min_x,
            y: rect.max_y,
        },
    ];
    (0..corners.len()).any(|index| {
        segments_intersect(
            start,
            end,
            corners[index],
            corners[(index + 1) % corners.len()],
        )
    })
}

/// Swift: EraserEngine.distance(from:to:)
fn distance_from_point_to_polyline(point: Point, polyline: &[Point]) -> f64 {
    let Some(first) = polyline.first().copied() else {
        return f64::INFINITY;
    };
    if polyline.len() == 1 {
        return (point.x - first.x).hypot(point.y - first.y);
    }
    polyline
        .windows(2)
        .map(|pair| point_to_segment_distance(point, pair[0], pair[1]))
        .fold(f64::INFINITY, f64::min)
}

/// Swift: EraserEngine.distance(between:and:)
fn distance_between(first: &[Point], second: &[Point]) -> f64 {
    let (Some(first_point), Some(second_point)) = (first.first().copied(), second.first().copied())
    else {
        return f64::INFINITY;
    };
    if first.len() == 1 {
        return distance_from_point_to_polyline(first_point, second);
    }
    if second.len() == 1 {
        return distance_from_point_to_polyline(second_point, first);
    }
    first
        .windows(2)
        .flat_map(|first_pair| {
            second.windows(2).map(move |second_pair| {
                segment_distance(first_pair[0], first_pair[1], second_pair[0], second_pair[1])
            })
        })
        .fold(f64::INFINITY, f64::min)
}

/// Swift: EraserEngine.segmentDistance(_:_:_:_:)
fn segment_distance(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> f64 {
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return 0.0;
    }
    point_to_segment_distance(first_start, second_start, second_end)
        .min(point_to_segment_distance(
            first_end,
            second_start,
            second_end,
        ))
        .min(point_to_segment_distance(
            second_start,
            first_start,
            first_end,
        ))
        .min(point_to_segment_distance(
            second_end,
            first_start,
            first_end,
        ))
}

/// Swift: EraserEngine.pointToSegmentDistance(_:_:_:)
fn point_to_segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let delta = Point {
        x: end.x - start.x,
        y: end.y - start.y,
    };
    let length_squared = delta.x * delta.x + delta.y * delta.y;
    if length_squared <= 0.0 {
        return (point.x - start.x).hypot(point.y - start.y);
    }
    let projection = (((point.x - start.x) * delta.x + (point.y - start.y) * delta.y)
        / length_squared)
        .clamp(0.0, 1.0);
    let nearest = Point {
        x: start.x + delta.x * projection,
        y: start.y + delta.y * projection,
    };
    (point.x - nearest.x).hypot(point.y - nearest.y)
}

/// Swift: EraserEngine.segmentsIntersect(_:_:_:_:)
fn segments_intersect(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> bool {
    let d1 = cross(first_start, first_end, second_start);
    let d2 = cross(first_start, first_end, second_end);
    let d3 = cross(second_start, second_end, first_start);
    let d4 = cross(second_start, second_end, first_end);
    if ((d1 > INTERSECTION_EPSILON && d2 < -INTERSECTION_EPSILON)
        || (d1 < -INTERSECTION_EPSILON && d2 > INTERSECTION_EPSILON))
        && ((d3 > INTERSECTION_EPSILON && d4 < -INTERSECTION_EPSILON)
            || (d3 < -INTERSECTION_EPSILON && d4 > INTERSECTION_EPSILON))
    {
        return true;
    }
    (d1.abs() <= INTERSECTION_EPSILON && point_on_segment(second_start, first_start, first_end))
        || (d2.abs() <= INTERSECTION_EPSILON
            && point_on_segment(second_end, first_start, first_end))
        || (d3.abs() <= INTERSECTION_EPSILON
            && point_on_segment(first_start, second_start, second_end))
        || (d4.abs() <= INTERSECTION_EPSILON
            && point_on_segment(first_end, second_start, second_end))
}

/// Swift: EraserEngine.cross(_:_:_:)
fn cross(start: Point, end: Point, point: Point) -> f64 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

/// Swift: EraserEngine.point(_:liesOn:_:)
fn point_on_segment(point: Point, start: Point, end: Point) -> bool {
    cross(start, end, point).abs() <= INTERSECTION_EPSILON
        && point.x >= start.x.min(end.x) - INTERSECTION_EPSILON
        && point.x <= start.x.max(end.x) + INTERSECTION_EPSILON
        && point.y >= start.y.min(end.y) - INTERSECTION_EPSILON
        && point.y <= start.y.max(end.y) + INTERSECTION_EPSILON
}

/// Swift: LassoGeometry.selects(_:polygon:hitRule:canvasSize:)
fn lasso_selects(
    stroke: &InkStroke,
    polygon: &[Point],
    hit_rule: EraserHitRule,
    canvas_size: Size,
) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let item_points = item_screen_points(stroke, canvas_size);
    if item_points.is_empty() {
        return false;
    }
    match hit_rule {
        EraserHitRule::Contained => item_points
            .iter()
            .all(|point| lasso_contains(*point, polygon)),
        EraserHitRule::Touching => {
            if item_points
                .iter()
                .any(|point| lasso_contains(*point, polygon))
            {
                return true;
            }
            if let Some(item_bounds) = stroke_bounds(stroke, canvas_size) {
                if polygon
                    .iter()
                    .any(|point| item_bounds.contains_point(*point))
                {
                    return true;
                }
            }
            let polygon_edges = closed_edges(polygon);
            item_points.windows(2).any(|item_edge| {
                polygon_edges.iter().any(|polygon_edge| {
                    segments_intersect(item_edge[0], item_edge[1], polygon_edge.0, polygon_edge.1)
                })
            })
        }
    }
}

/// Swift: LassoGeometry.contains(_:polygon:)
fn lasso_contains(point: Point, polygon: &[Point]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    if closed_edges(polygon)
        .iter()
        .any(|edge| point_on_segment(point, edge.0, edge.1))
    {
        return true;
    }
    let mut is_inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        let crosses = (current.y > point.y) != (previous.y > point.y);
        if crosses {
            let crossing_x = (previous.x - current.x) * (point.y - current.y)
                / (previous.y - current.y)
                + current.x;
            if point.x < crossing_x {
                is_inside = !is_inside;
            }
        }
        previous = current;
    }
    is_inside
}

/// Swift: LassoGeometry.polygonEdges(_:)
fn closed_edges(points: &[Point]) -> Vec<(Point, Point)> {
    if points.is_empty() {
        return Vec::new();
    }
    (0..points.len())
        .map(|index| (points[index], points[(index + 1) % points.len()]))
        .collect()
}
