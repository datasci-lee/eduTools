//! Shape recognition, lasso selection, selection transforms, and screen covers.

use crate::ink::{
    InkGeometry, InkPoint, InkShapeKind, InkStroke, InkStrokeContent, InkStrokeTool,
    Point as InkLocation, Size,
};

// Swift: CGPoint
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    // Swift: CGPoint.init(x:y:)
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// Swift: CGRect
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    // Swift: CGRect.init(x:y:width:height:)
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    // Swift: CGRect union reduction in ShapeRecognizer.recognize(points:)
    pub fn from_points(points: &[Point]) -> Option<Self> {
        let first = *points.first()?;
        let mut minimum_x = first.x;
        let mut maximum_x = first.x;
        let mut minimum_y = first.y;
        let mut maximum_y = first.y;
        for point in &points[1..] {
            minimum_x = minimum_x.min(point.x);
            maximum_x = maximum_x.max(point.x);
            minimum_y = minimum_y.min(point.y);
            maximum_y = maximum_y.max(point.y);
        }
        Some(Self::new(
            minimum_x,
            minimum_y,
            maximum_x - minimum_x,
            maximum_y - minimum_y,
        ))
    }

    // Swift: CGRect.standardized
    pub fn standardized(self) -> Self {
        Self::new(
            self.x.min(self.x + self.width),
            self.y.min(self.y + self.height),
            self.width.abs(),
            self.height.abs(),
        )
    }

    // Swift: CGRect.contains(_:)
    pub fn contains(self, point: Point) -> bool {
        let rect = self.standardized();
        point.x >= rect.x
            && point.x <= rect.x + rect.width
            && point.y >= rect.y
            && point.y <= rect.y + rect.height
    }
}

// Swift: InkShapeKind
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeKind {
    Line,
    Rectangle,
    Ellipse,
}

// Swift: ShapeRecognizer recognition constants
struct RecognitionThresholds;

impl RecognitionThresholds {
    const LINE_ENDPOINT_DIAGONAL_RATIO: f64 = 0.8;
    const LINE_DEVIATION_DIAGONAL_RATIO: f64 = 0.08;
    const LINE_PATH_ENDPOINT_RATIO: f64 = 1.18;
    const CLOSED_ENDPOINT_DIAGONAL_RATIO: f64 = 0.24;
    const CLOSED_SIDE_DIAGONAL_RATIO: f64 = 0.18;
    const RECTANGLE_EDGE_SHORT_SIDE_RATIO: f64 = 0.14;
    const RECTANGLE_CORNER_DIAGONAL_RATIO: f64 = 0.22;
    const RECTANGLE_MINIMUM_PERIMETER_RATIO: f64 = 0.72;
    const RECTANGLE_MAXIMUM_PERIMETER_RATIO: f64 = 1.35;
    const ELLIPSE_MINIMUM_ASPECT: f64 = 0.65;
    const ELLIPSE_MAXIMUM_ASPECT: f64 = 1.55;
    const ELLIPSE_MAXIMUM_AVERAGE_ERROR: f64 = 0.16;
    const ELLIPSE_MAXIMUM_POINT_ERROR: f64 = 0.38;
    const ELLIPSE_MINIMUM_CIRCUMFERENCE_RATIO: f64 = 0.68;
    const ELLIPSE_MAXIMUM_CIRCUMFERENCE_RATIO: f64 = 1.35;
}

// Swift: ShapeRecognizer
pub struct ShapeRecognizer;

impl ShapeRecognizer {
    // Swift: ShapeRecognizer.recognize(points:)
    pub fn recognize(points: &[Point]) -> Option<ShapeKind> {
        if points.len() < 2 {
            return None;
        }
        let bounds = Rect::from_points(points)?;
        let diagonal = bounds.width.hypot(bounds.height);
        if diagonal <= 0.0 {
            return None;
        }

        let path_length = points
            .windows(2)
            .map(|pair| distance(pair[0], pair[1]))
            .sum::<f64>();
        let endpoint_distance = distance(points[0], points[points.len() - 1]);
        if endpoint_distance >= diagonal * RecognitionThresholds::LINE_ENDPOINT_DIAGONAL_RATIO
            && maximum_line_deviation(points)
                <= diagonal * RecognitionThresholds::LINE_DEVIATION_DIAGONAL_RATIO
            && path_length <= endpoint_distance * RecognitionThresholds::LINE_PATH_ENDPOINT_RATIO
        {
            return Some(ShapeKind::Line);
        }

        if points.len() < 5
            || endpoint_distance > diagonal * RecognitionThresholds::CLOSED_ENDPOINT_DIAGONAL_RATIO
            || bounds.width < diagonal * RecognitionThresholds::CLOSED_SIDE_DIAGONAL_RATIO
            || bounds.height < diagonal * RecognitionThresholds::CLOSED_SIDE_DIAGONAL_RATIO
        {
            return None;
        }

        if resembles_rectangle(points, bounds, path_length) {
            return Some(ShapeKind::Rectangle);
        }
        if resembles_ellipse(points, bounds, path_length) {
            return Some(ShapeKind::Ellipse);
        }
        None
    }

    // Swift: ShapeRecognizer.correctedStroke(from:canvasSize:)
    pub fn corrected_stroke(stroke: &InkStroke, canvas_size: Size) -> Option<InkStroke> {
        if !matches!(&stroke.content, InkStrokeContent::Freehand)
            || canvas_size.width <= 0.0
            || canvas_size.height <= 0.0
        {
            return None;
        }
        let screen_points = stroke
            .points
            .iter()
            .map(|point| {
                let location = InkGeometry::screen_point(point.location, canvas_size);
                Point::new(location.x, location.y)
            })
            .collect::<Vec<_>>();
        let shape = Self::recognize(&screen_points)?;
        let first = stroke.points.first()?.clone();
        let last = stroke.points.last()?.clone();
        let kind = match shape {
            ShapeKind::Line => InkShapeKind::Line,
            ShapeKind::Rectangle => InkShapeKind::Rectangle,
            ShapeKind::Ellipse => InkShapeKind::Ellipse,
        };
        let points = match shape {
            ShapeKind::Line => vec![first, last],
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                let bounds = Rect::from_points(&screen_points)?;
                vec![
                    InkPoint::new(
                        InkGeometry::normalized_point(
                            InkLocation {
                                x: bounds.x,
                                y: bounds.y,
                            },
                            canvas_size,
                        ),
                        1.0,
                        first.timestamp,
                    ),
                    InkPoint::new(
                        InkGeometry::normalized_point(
                            InkLocation {
                                x: bounds.x + bounds.width,
                                y: bounds.y + bounds.height,
                            },
                            canvas_size,
                        ),
                        1.0,
                        last.timestamp,
                    ),
                ]
            }
        };
        Some(InkStroke::new(
            stroke.identifier.clone(),
            InkStrokeTool::Shape,
            stroke.color,
            stroke.width,
            false,
            points,
            InkStrokeContent::Shape(kind),
        ))
    }
}

// Swift: ShapeRecognizer.resemblesRectangle(_:bounds:pathLength:)
fn resembles_rectangle(points: &[Point], bounds: Rect, path_length: f64) -> bool {
    let shorter_side = bounds.width.min(bounds.height);
    let edge_tolerance = shorter_side * RecognitionThresholds::RECTANGLE_EDGE_SHORT_SIDE_RATIO;
    let corner_tolerance =
        bounds.width.hypot(bounds.height) * RecognitionThresholds::RECTANGLE_CORNER_DIAGONAL_RATIO;
    let perimeter = 2.0 * (bounds.width + bounds.height);
    if path_length < perimeter * RecognitionThresholds::RECTANGLE_MINIMUM_PERIMETER_RATIO
        || path_length > perimeter * RecognitionThresholds::RECTANGLE_MAXIMUM_PERIMETER_RATIO
        || !points.iter().all(|point| {
            (point.x - bounds.x)
                .abs()
                .min((point.x - (bounds.x + bounds.width)).abs())
                .min((point.y - bounds.y).abs())
                .min((point.y - (bounds.y + bounds.height)).abs())
                <= edge_tolerance
        })
    {
        return false;
    }

    let corners = [
        Point::new(bounds.x, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
        Point::new(bounds.x, bounds.y + bounds.height),
    ];
    corners.iter().all(|corner| {
        points
            .iter()
            .any(|point| distance(*point, *corner) <= corner_tolerance)
    })
}

// Swift: ShapeRecognizer.resemblesEllipse(_:bounds:pathLength:)
fn resembles_ellipse(points: &[Point], bounds: Rect, path_length: f64) -> bool {
    let aspect = bounds.width / bounds.height;
    if !(RecognitionThresholds::ELLIPSE_MINIMUM_ASPECT
        ..=RecognitionThresholds::ELLIPSE_MAXIMUM_ASPECT)
        .contains(&aspect)
    {
        return false;
    }
    let radius_x = bounds.width / 2.0;
    let radius_y = bounds.height / 2.0;
    let center = Point::new(bounds.x + radius_x, bounds.y + radius_y);
    let errors = points
        .iter()
        .map(|point| {
            let normalized_x = (point.x - center.x) / radius_x;
            let normalized_y = (point.y - center.y) / radius_y;
            (normalized_x.hypot(normalized_y) - 1.0).abs()
        })
        .collect::<Vec<_>>();
    let average_error = errors.iter().sum::<f64>() / errors.len() as f64;
    let maximum_error = errors.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let circumference = std::f64::consts::PI
        * (3.0 * (radius_x + radius_y)
            - ((3.0 * radius_x + radius_y) * (radius_x + 3.0 * radius_y)).sqrt());
    average_error <= RecognitionThresholds::ELLIPSE_MAXIMUM_AVERAGE_ERROR
        && maximum_error <= RecognitionThresholds::ELLIPSE_MAXIMUM_POINT_ERROR
        && path_length >= circumference * RecognitionThresholds::ELLIPSE_MINIMUM_CIRCUMFERENCE_RATIO
        && path_length <= circumference * RecognitionThresholds::ELLIPSE_MAXIMUM_CIRCUMFERENCE_RATIO
}

// Swift: ShapeRecognizer.maximumLineDeviation(_:)
fn maximum_line_deviation(points: &[Point]) -> f64 {
    let Some(first) = points.first().copied() else {
        return f64::INFINITY;
    };
    let Some(last) = points.last().copied() else {
        return f64::INFINITY;
    };
    points
        .iter()
        .map(|point| point_to_segment_distance(*point, first, last))
        .fold(f64::NEG_INFINITY, f64::max)
}

// Swift: ShapeRecognizer.pointToSegmentDistance(_:_:_:)
fn point_to_segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let delta = Point::new(end.x - start.x, end.y - start.y);
    let length_squared = delta.x * delta.x + delta.y * delta.y;
    if length_squared <= 0.0 {
        return distance(point, start);
    }
    let projection = (((point.x - start.x) * delta.x + (point.y - start.y) * delta.y)
        / length_squared)
        .clamp(0.0, 1.0);
    distance(
        point,
        Point::new(
            start.x + delta.x * projection,
            start.y + delta.y * projection,
        ),
    )
}

// Swift: ShapeRecognizer.distance(_:_:)
fn distance(first: Point, second: Point) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectionHit {
    Resize { anchor: Point },
    Move,
    Lasso,
}

// Swift: InkView.resizeAnchor(at:selectionBounds:) and the inflated move hit area.
pub fn selection_hit(point: Point, bounds: Option<Rect>) -> SelectionHit {
    let Some(bounds) = bounds else {
        return SelectionHit::Lasso;
    };
    let corners_and_anchors = [
        (
            Point::new(bounds.x, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
        ),
        (
            Point::new(bounds.x + bounds.width, bounds.y),
            Point::new(bounds.x, bounds.y + bounds.height),
        ),
        (
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            Point::new(bounds.x, bounds.y),
        ),
        (
            Point::new(bounds.x, bounds.y + bounds.height),
            Point::new(bounds.x + bounds.width, bounds.y),
        ),
    ];
    if let Some((_, anchor)) = corners_and_anchors
        .iter()
        .find(|(corner, _)| distance(point, *corner) <= 10.0)
    {
        return SelectionHit::Resize { anchor: *anchor };
    }
    let inflated = Rect::new(
        bounds.x - 6.0,
        bounds.y - 6.0,
        bounds.width + 12.0,
        bounds.height + 12.0,
    );
    if inflated.contains(point) {
        SelectionHit::Move
    } else {
        SelectionHit::Lasso
    }
}

// Swift: InkItemGeometry
pub struct InkItemGeometry;

impl InkItemGeometry {
    // Swift: InkItemGeometry.screenPoints(for:canvasSize:)
    pub fn item_screen_points(stroke: &InkStroke, canvas_size: Size) -> Vec<Point> {
        let points = stroke
            .points
            .iter()
            .map(|point| InkGeometry::screen_point(point.location, canvas_size))
            .map(|point| Point::new(point.x, point.y))
            .collect::<Vec<_>>();
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
            | InkStrokeContent::Cover => closed_bounds_points(first, last),
            InkStrokeContent::Shape(InkShapeKind::Ellipse) => {
                let center = Point::new((first.x + last.x) / 2.0, (first.y + last.y) / 2.0);
                let radius_x = (last.x - first.x).abs() / 2.0;
                let radius_y = (last.y - first.y).abs() / 2.0;
                (0..=32)
                    .map(|index| {
                        let angle = f64::from(index) / 32.0 * std::f64::consts::TAU;
                        Point::new(
                            center.x + angle.cos() * radius_x,
                            center.y + angle.sin() * radius_y,
                        )
                    })
                    .collect()
            }
        }
    }

    // Swift: InkItemGeometry.bounds(for:canvasSize:)
    pub fn item_bounds(stroke: &InkStroke, canvas_size: Size) -> Option<Rect> {
        let mut bounds = Rect::from_points(&Self::item_screen_points(stroke, canvas_size))?;
        let half_width = stroke.width / 2.0;
        bounds.x -= half_width;
        bounds.y -= half_width;
        bounds.width += stroke.width;
        bounds.height += stroke.width;
        Some(bounds)
    }
}

fn closed_bounds_points(first: Point, last: Point) -> Vec<Point> {
    let minimum_x = first.x.min(last.x);
    let maximum_x = first.x.max(last.x);
    let minimum_y = first.y.min(last.y);
    let maximum_y = first.y.max(last.y);
    let corners = [
        Point::new(minimum_x, minimum_y),
        Point::new(maximum_x, minimum_y),
        Point::new(maximum_x, maximum_y),
        Point::new(minimum_x, maximum_y),
    ];
    corners.into_iter().chain([corners[0]]).collect()
}

// Swift: EraserHitRule
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LassoHitRule {
    Contained,
    Touching,
}

// Swift: LassoGeometry
pub struct LassoGeometry;

impl LassoGeometry {
    // Swift: LassoGeometry.contains(_:polygon:)
    pub fn contains(point: Point, polygon: &[Point]) -> bool {
        if polygon.len() < 3 {
            return false;
        }
        if polygon_edges(polygon).any(|(start, end)| point_on_segment(point, start, end)) {
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

    // Swift: LassoGeometry.selects(_:polygon:hitRule:canvasSize:)
    pub fn selects(item_points: &[Point], polygon: &[Point], hit_rule: LassoHitRule) -> bool {
        if polygon.len() < 3 || item_points.is_empty() {
            return false;
        }
        match hit_rule {
            LassoHitRule::Contained => item_points
                .iter()
                .all(|point| Self::contains(*point, polygon)),
            LassoHitRule::Touching => {
                if item_points
                    .iter()
                    .any(|point| Self::contains(*point, polygon))
                {
                    return true;
                }
                if let Some(bounds) = Rect::from_points(item_points) {
                    if polygon.iter().any(|point| bounds.contains(*point)) {
                        return true;
                    }
                }
                item_points.windows(2).any(|item_edge| {
                    polygon_edges(polygon).any(|(polygon_start, polygon_end)| {
                        segments_intersect(item_edge[0], item_edge[1], polygon_start, polygon_end)
                    })
                })
            }
        }
    }
}

// Swift: LassoGeometry.polygonEdges(_:)
fn polygon_edges(polygon: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
}

// Swift: LassoGeometry.segmentsIntersect(_:_:_:_:)
fn segments_intersect(
    first_start: Point,
    first_end: Point,
    second_start: Point,
    second_end: Point,
) -> bool {
    const EPSILON: f64 = 0.000_001;
    let first_a = cross(first_start, first_end, second_start);
    let first_b = cross(first_start, first_end, second_end);
    let second_a = cross(second_start, second_end, first_start);
    let second_b = cross(second_start, second_end, first_end);
    if ((first_a > EPSILON && first_b < -EPSILON) || (first_a < -EPSILON && first_b > EPSILON))
        && ((second_a > EPSILON && second_b < -EPSILON)
            || (second_a < -EPSILON && second_b > EPSILON))
    {
        return true;
    }
    (first_a.abs() <= EPSILON && point_on_segment(second_start, first_start, first_end))
        || (first_b.abs() <= EPSILON && point_on_segment(second_end, first_start, first_end))
        || (second_a.abs() <= EPSILON && point_on_segment(first_start, second_start, second_end))
        || (second_b.abs() <= EPSILON && point_on_segment(first_end, second_start, second_end))
}

// Swift: LassoGeometry.cross(_:_:_:)
fn cross(start: Point, end: Point, point: Point) -> f64 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

// Swift: LassoGeometry.pointOnSegment(_:_:_:)
fn point_on_segment(point: Point, start: Point, end: Point) -> bool {
    const EPSILON: f64 = 0.000_001;
    cross(start, end, point).abs() <= EPSILON
        && point.x >= start.x.min(end.x) - EPSILON
        && point.x <= start.x.max(end.x) + EPSILON
        && point.y >= start.y.min(end.y) - EPSILON
        && point.y <= start.y.max(end.y) + EPSILON
}

// Swift: InkStrokeContent subset used by SelectionTransform and ScreenCoverEngine
#[derive(Clone, Debug, PartialEq)]
pub enum SelectionContent {
    Freehand,
    Shape(ShapeKind),
    Text { font_size: f64 },
    Cover,
}

// Swift: InkStroke subset used by SelectionTransform and ScreenCoverEngine
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionItem {
    pub id: u64,
    pub points: Vec<Point>,
    pub content: SelectionContent,
}

// Swift: SelectionTransform
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeSelection {
    pub items: Vec<SelectionItem>,
}

impl ShapeSelection {
    // Swift: SelectionTransform.moved(_:by:)
    pub fn moved(&self, delta: Point) -> Self {
        Self {
            items: self
                .items
                .iter()
                .cloned()
                .map(|mut item| {
                    for point in &mut item.points {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                    item
                })
                .collect(),
        }
    }

    // Swift: SelectionTransform.resized(_:from:to:)
    pub fn resized(&self, source_bounds: Rect, destination_bounds: Rect) -> Self {
        if source_bounds.width <= 0.0 || source_bounds.height <= 0.0 {
            return self.clone();
        }
        let scale_x = destination_bounds.width / source_bounds.width;
        let scale_y = destination_bounds.height / source_bounds.height;
        Self {
            items: self
                .items
                .iter()
                .cloned()
                .map(|mut item| {
                    if let SelectionContent::Text { font_size } = &mut item.content {
                        let font_scale = scale_x.abs().min(scale_y.abs()).max(0.1);
                        *font_size = (*font_size * font_scale).max(1.0);
                    }
                    for point in &mut item.points {
                        point.x = destination_bounds.x + (point.x - source_bounds.x) * scale_x;
                        point.y = destination_bounds.y + (point.y - source_bounds.y) * scale_y;
                    }
                    item
                })
                .collect(),
        }
    }
}

// Swift: ScreenCoverEngine
pub struct ScreenCoverEngine;

impl ScreenCoverEngine {
    // Swift: ScreenCoverEngine.removingTopmostCover(at:from:)
    pub fn removing_topmost_cover(at: Point, from: &[SelectionItem]) -> Vec<SelectionItem> {
        let Some(index) = from.iter().rposition(|item| {
            item.content == SelectionContent::Cover
                && item.points.len() >= 2
                && Rect::from_points(&item.points).is_some_and(|bounds| bounds.contains(at))
        }) else {
            return from.to_vec();
        };
        let mut result = from.to_vec();
        result.remove(index);
        result
    }
}

// Swift: SelectionEngine
pub struct SelectionEngine;

impl SelectionEngine {
    // Swift: SelectionEngine.selectedIdentifiers(from:polygon:hitRule:canvasSize:)
    pub fn selected_ids(
        from: &[SelectionItem],
        polygon: &[Point],
        hit_rule: LassoHitRule,
    ) -> Vec<u64> {
        from.iter()
            .filter(|item| LassoGeometry::selects(&item.points, polygon, hit_rule))
            .map(|item| item.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        selection_hit, InkItemGeometry, Point as ShapePoint, Rect, SelectionHit, ShapeRecognizer,
    };
    use crate::ink::{
        ArrowStyle, InkPoint, InkShapeKind, InkStroke, InkStrokeContent, InkStrokeTool,
        InkTextContent, Point, Size,
    };

    #[derive(Clone, Copy)]
    enum Expected {
        Shape(InkShapeKind, [Point; 2]),
        None,
    }

    #[test]
    fn corrected_stroke_table() {
        let cases = [
            (
                "closed-rectangle",
                freehand(
                    "rectangle-id",
                    &[(0.2, 0.3), (0.8, 0.3), (0.8, 0.7), (0.2, 0.7), (0.2, 0.3)],
                ),
                Expected::Shape(
                    InkShapeKind::Rectangle,
                    [Point { x: 0.2, y: 0.3 }, Point { x: 0.8, y: 0.7 }],
                ),
            ),
            (
                "straight-line",
                freehand("line-id", &[(0.1, 0.2), (0.5, 0.5), (0.9, 0.8)]),
                Expected::Shape(
                    InkShapeKind::Line,
                    [Point { x: 0.1, y: 0.2 }, Point { x: 0.9, y: 0.8 }],
                ),
            ),
            (
                "arrow-path-is-not-corrected",
                freehand(
                    "arrow-id",
                    &[(0.1, 0.5), (0.8, 0.5), (0.65, 0.4), (0.8, 0.5), (0.65, 0.6)],
                ),
                Expected::None,
            ),
            (
                "non-freehand",
                InkStroke::new(
                    "shape-id",
                    InkStrokeTool::Shape,
                    [0.1, 0.2, 0.3, 1.0],
                    6.0,
                    false,
                    vec![
                        InkPoint::new(Point { x: 0.1, y: 0.1 }, 1.0, 10.0),
                        InkPoint::new(Point { x: 0.9, y: 0.9 }, 1.0, 20.0),
                    ],
                    InkStrokeContent::Shape(InkShapeKind::Arrow(ArrowStyle::Single)),
                ),
                Expected::None,
            ),
        ];

        for (name, stroke, expected) in cases {
            let corrected = ShapeRecognizer::corrected_stroke(
                &stroke,
                Size {
                    width: 1_000.0,
                    height: 800.0,
                },
            );
            match expected {
                Expected::None => assert!(corrected.is_none(), "{name}"),
                Expected::Shape(kind, points) => {
                    let corrected = corrected.unwrap_or_else(|| panic!("{name}"));
                    assert_eq!(corrected.identifier, stroke.identifier, "{name}");
                    assert_eq!(corrected.tool, InkStrokeTool::Shape, "{name}");
                    assert_eq!(corrected.content, InkStrokeContent::Shape(kind), "{name}");
                    assert!(!corrected.uses_pressure, "{name}");
                    assert_eq!(corrected.points.len(), 2, "{name}");
                    assert_eq!(corrected.points[0].location, points[0], "{name}");
                    assert_eq!(corrected.points[1].location, points[1], "{name}");
                }
            }
        }
    }

    #[test]
    fn item_screen_points_and_bounds_table() {
        let size = Size {
            width: 200.0,
            height: 100.0,
        };
        let cases = [
            (
                "freehand",
                content_stroke(InkStrokeContent::Freehand, 3),
                3,
                164.0,
                84.0,
            ),
            (
                "line",
                content_stroke(InkStrokeContent::Shape(InkShapeKind::Line), 3),
                2,
                164.0,
                84.0,
            ),
            (
                "arrow",
                content_stroke(
                    InkStrokeContent::Shape(InkShapeKind::Arrow(ArrowStyle::Single)),
                    3,
                ),
                2,
                164.0,
                84.0,
            ),
            (
                "rectangle",
                content_stroke(InkStrokeContent::Shape(InkShapeKind::Rectangle), 2),
                5,
                84.0,
                44.0,
            ),
            (
                "ellipse",
                content_stroke(InkStrokeContent::Shape(InkShapeKind::Ellipse), 2),
                33,
                84.0,
                44.0,
            ),
            (
                "text",
                content_stroke(
                    InkStrokeContent::Text(InkTextContent {
                        string: "T".to_owned(),
                        font_size: 20.0,
                    }),
                    2,
                ),
                5,
                84.0,
                44.0,
            ),
            (
                "cover",
                content_stroke(InkStrokeContent::Cover, 2),
                5,
                84.0,
                44.0,
            ),
        ];

        for (name, stroke, expected_count, expected_width, expected_height) in cases {
            assert_eq!(
                InkItemGeometry::item_screen_points(&stroke, size).len(),
                expected_count,
                "{name}"
            );
            let bounds =
                InkItemGeometry::item_bounds(&stroke, size).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(bounds.x, 18.0, "{name}");
            assert_eq!(bounds.y, 8.0, "{name}");
            assert_eq!(bounds.width, expected_width, "{name}");
            assert_eq!(bounds.height, expected_height, "{name}");
        }
    }

    #[test]
    fn handle_and_move_region_boundary_table() {
        let bounds = Rect::new(100.0, 100.0, 80.0, 60.0);
        let cases = [
            (
                "handle radius inclusive",
                ShapePoint::new(110.0, 100.0),
                SelectionHit::Resize {
                    anchor: ShapePoint::new(180.0, 160.0),
                },
            ),
            (
                "outside handle but inside selection",
                ShapePoint::new(111.0, 100.0),
                SelectionHit::Move,
            ),
            (
                "inflated move edge inclusive",
                ShapePoint::new(94.0, 130.0),
                SelectionHit::Move,
            ),
            (
                "outside inflated move edge",
                ShapePoint::new(93.999, 130.0),
                SelectionHit::Lasso,
            ),
            (
                "no selection",
                ShapePoint::new(100.0, 100.0),
                SelectionHit::Lasso,
            ),
        ];

        for (name, point, expected) in cases {
            let input_bounds = (name != "no selection").then_some(bounds);
            assert_eq!(selection_hit(point, input_bounds), expected, "{name}");
        }
    }

    fn freehand(identifier: &str, points: &[(f64, f64)]) -> InkStroke {
        InkStroke::new(
            identifier,
            InkStrokeTool::Pen,
            [0.1, 0.2, 0.3, 1.0],
            6.0,
            true,
            points
                .iter()
                .enumerate()
                .map(|(index, &(x, y))| InkPoint::new(Point { x, y }, 0.5, 10.0 + index as f64))
                .collect(),
            InkStrokeContent::Freehand,
        )
    }

    fn content_stroke(content: InkStrokeContent, point_count: usize) -> InkStroke {
        let locations = [
            Point { x: 0.1, y: 0.1 },
            Point { x: 0.5, y: 0.5 },
            Point { x: 0.9, y: 0.9 },
        ];
        InkStroke::new(
            "geometry",
            InkStrokeTool::Shape,
            [0.1, 0.2, 0.3, 1.0],
            4.0,
            false,
            locations[..point_count]
                .iter()
                .enumerate()
                .map(|(index, location)| InkPoint::new(*location, 1.0, index as f64))
                .collect(),
            content,
        )
    }
}
