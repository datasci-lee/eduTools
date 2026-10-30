use edutools_core::ink::{
    InkShapeKind, InkStroke, InkStrokeContent, InkTextContent, Point as InkPoint, Size,
};
use edutools_core::shape::{
    selection_hit as core_selection_hit, InkItemGeometry, LassoHitRule, Point, Rect,
    SelectionContent, SelectionEngine, SelectionItem, ShapeKind, ShapeSelection,
};
use std::collections::{BTreeMap, BTreeSet};

pub use edutools_core::shape::SelectionHit;

pub fn selection_hit(point: InkPoint, bounds: Option<Rect>) -> SelectionHit {
    core_selection_hit(Point::new(point.x, point.y), bounds)
}

pub fn selected_stroke_ids(
    strokes: &[InkStroke],
    polygon: &[InkPoint],
    hit_rule: LassoHitRule,
    canvas_size: Size,
) -> BTreeSet<String> {
    let items = strokes
        .iter()
        .enumerate()
        .map(|(index, stroke)| SelectionItem {
            id: index as u64,
            points: InkItemGeometry::item_screen_points(stroke, canvas_size),
            content: selection_content(stroke),
        })
        .collect::<Vec<_>>();
    let polygon = polygon
        .iter()
        .map(|point| Point::new(point.x, point.y))
        .collect::<Vec<_>>();
    SelectionEngine::selected_ids(&items, &polygon, hit_rule)
        .into_iter()
        .filter_map(|index| strokes.get(index as usize))
        .map(|stroke| stroke.identifier.clone())
        .collect()
}

pub fn selection_bounds(
    strokes: &[InkStroke],
    selected_ids: &BTreeSet<String>,
    canvas_size: Size,
) -> Option<Rect> {
    strokes
        .iter()
        .filter(|stroke| selected_ids.contains(&stroke.identifier))
        .filter_map(|stroke| InkItemGeometry::item_bounds(stroke, canvas_size))
        .fold(None, |bounds, item| Some(union_rect(bounds, item)))
}

pub fn normalized_point_bounds(
    strokes: &[InkStroke],
    selected_ids: &BTreeSet<String>,
) -> Option<Rect> {
    let points = strokes
        .iter()
        .filter(|stroke| selected_ids.contains(&stroke.identifier))
        .flat_map(|stroke| stroke.points.iter())
        .map(|point| Point::new(point.location.x, point.location.y))
        .collect::<Vec<_>>();
    Rect::from_points(&points)
}

pub fn moved_strokes(
    strokes: &[InkStroke],
    selected_ids: &BTreeSet<String>,
    delta: InkPoint,
) -> Vec<InkStroke> {
    let selection = normalized_selection(strokes, selected_ids);
    apply_selection(strokes, selection.moved(Point::new(delta.x, delta.y)))
}

pub fn resized_strokes(
    strokes: &[InkStroke],
    selected_ids: &BTreeSet<String>,
    source_bounds: Rect,
    destination_bounds: Rect,
) -> Vec<InkStroke> {
    let selection = normalized_selection(strokes, selected_ids);
    apply_selection(
        strokes,
        selection.resized(source_bounds, destination_bounds),
    )
}

fn normalized_selection(strokes: &[InkStroke], selected_ids: &BTreeSet<String>) -> ShapeSelection {
    ShapeSelection {
        items: strokes
            .iter()
            .enumerate()
            .filter(|(_, stroke)| selected_ids.contains(&stroke.identifier))
            .map(|(index, stroke)| SelectionItem {
                id: index as u64,
                points: stroke
                    .points
                    .iter()
                    .map(|point| Point::new(point.location.x, point.location.y))
                    .collect(),
                content: selection_content(stroke),
            })
            .collect(),
    }
}

fn selection_content(stroke: &InkStroke) -> SelectionContent {
    match &stroke.content {
        InkStrokeContent::Freehand => SelectionContent::Freehand,
        InkStrokeContent::Shape(kind) => SelectionContent::Shape(match kind {
            InkShapeKind::Line | InkShapeKind::Arrow(_) => ShapeKind::Line,
            InkShapeKind::Rectangle => ShapeKind::Rectangle,
            InkShapeKind::Ellipse => ShapeKind::Ellipse,
        }),
        InkStrokeContent::Text(text) => SelectionContent::Text {
            font_size: text.font_size,
        },
        InkStrokeContent::Cover => SelectionContent::Cover,
    }
}

fn apply_selection(strokes: &[InkStroke], selection: ShapeSelection) -> Vec<InkStroke> {
    let transformed = selection
        .items
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<BTreeMap<_, _>>();
    strokes
        .iter()
        .enumerate()
        .map(|(index, stroke)| {
            let Some(item) = transformed.get(&(index as u64)) else {
                return stroke.clone();
            };
            let mut replacement = stroke.clone();
            for (point, transformed) in replacement.points.iter_mut().zip(&item.points) {
                point.location = InkPoint {
                    x: transformed.x,
                    y: transformed.y,
                };
            }
            if let (InkStrokeContent::Text(text), SelectionContent::Text { font_size }) =
                (&replacement.content, &item.content)
            {
                replacement.content = InkStrokeContent::Text(InkTextContent {
                    string: text.string.clone(),
                    font_size: *font_size,
                });
            }
            replacement
        })
        .collect()
}

fn union_rect(bounds: Option<Rect>, item: Rect) -> Rect {
    let Some(bounds) = bounds else {
        return item.standardized();
    };
    let bounds = bounds.standardized();
    let item = item.standardized();
    let minimum_x = bounds.x.min(item.x);
    let minimum_y = bounds.y.min(item.y);
    let maximum_x = (bounds.x + bounds.width).max(item.x + item.width);
    let maximum_y = (bounds.y + bounds.height).max(item.y + item.height);
    Rect::new(
        minimum_x,
        minimum_y,
        maximum_x - minimum_x,
        maximum_y - minimum_y,
    )
}
