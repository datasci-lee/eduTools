//! Shape recognition, lasso selection, selection transforms, and screen covers.

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
