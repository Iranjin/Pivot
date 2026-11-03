use nih_plug_egui::egui::{self, Color32, Response, Sense, Shape, Stroke, Ui, Vec2};

const ACCENT: Color32 = Color32::from_rgb(96, 206, 245);
const DIM: Color32 = Color32::from_rgb(46, 50, 62);
const TRACK: Color32 = Color32::from_rgb(34, 37, 46);
const BACKGROUND: Color32 = Color32::from_rgb(20, 22, 28);
const HUB: Color32 = Color32::from_rgb(28, 31, 40);

pub fn pitch_knob(ui: &mut Ui, value: &mut f32) -> Response {
    let size = ui.available_width().clamp(120.0, 220.0);
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;

    {
        let painter = ui.painter();
        painter.circle_filled(center, radius, BACKGROUND);
        painter.circle_stroke(center, radius, Stroke::new(1.0, DIM));

        let angle_for = |value: f32| (value / 24.0).clamp(-1.0, 1.0) * 135.0_f32.to_radians();
        let point =
            |angle: f32, distance: f32| center + Vec2::new(angle.sin(), -angle.cos()) * distance;

        let arc_radius = radius * 0.84;
        let start = -135.0_f32.to_radians();
        let end = 135.0_f32.to_radians();
        let mut track_points = Vec::with_capacity(65);
        for step in 0..=64 {
            let t = step as f32 / 64.0;
            track_points.push(point(start + (end - start) * t, arc_radius));
        }
        painter.add(Shape::line(track_points, Stroke::new(3.0, TRACK)));

        let angle = angle_for(*value);
        let mut value_points = Vec::with_capacity(33);
        for step in 0..=32 {
            let t = step as f32 / 32.0;
            value_points.push(point(angle * t, arc_radius));
        }
        painter.add(Shape::line(value_points, Stroke::new(5.0, ACCENT)));

        for semitone in -24..=24 {
            let angle = angle_for(semitone as f32);
            let major = semitone % 12 == 0;
            let inner = radius * if major { 0.66 } else { 0.70 };
            let outer = radius * 0.74;
            painter.line_segment(
                [point(angle, inner), point(angle, outer)],
                Stroke::new(
                    if major { 1.5 } else { 1.0 },
                    if major { ACCENT } else { DIM },
                ),
            );
        }

        let indicator = point(angle, radius * 0.62);
        painter.line_segment([center, indicator], Stroke::new(4.0, ACCENT));
        painter.circle_filled(indicator, 6.0, ACCENT);
        painter.circle_filled(center, 10.0, HUB);
        painter.circle_stroke(center, 10.0, Stroke::new(2.0, ACCENT));
    }

    if response.dragged() {
        let sensitivity = if ui.input(|input| input.modifiers.shift) {
            0.005
        } else {
            0.05
        };
        let delta = -response.drag_delta().y * sensitivity;
        if delta != 0.0 {
            *value = (*value + delta).clamp(-24.0, 24.0);
            response.mark_changed();
        }
    }

    if response.hovered() {
        let scroll = ui.input(|input| input.raw_scroll_delta.y);
        if scroll != 0.0 {
            let sensitivity = if ui.input(|input| input.modifiers.shift) {
                0.0005
            } else {
                0.002
            };
            *value = (*value + scroll * sensitivity).clamp(-24.0, 24.0);
            response.mark_changed();
        }
    }

    if response.double_clicked() {
        *value = 0.0;
        response.mark_changed();
    }

    response.on_hover_cursor(egui::CursorIcon::ResizeVertical)
}
