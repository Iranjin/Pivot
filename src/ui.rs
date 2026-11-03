use nih_plug_egui::egui::{
    self, Align2, Color32, FontId, Response, Sense, Shape, Stroke, Ui, Vec2,
};

const ACCENT: Color32 = Color32::from_rgb(96, 206, 245);
const DIM: Color32 = Color32::from_rgb(46, 50, 62);
const TRACK: Color32 = Color32::from_rgb(34, 37, 46);
const BACKGROUND: Color32 = Color32::from_rgb(20, 22, 28);
const HUB: Color32 = Color32::from_rgb(13, 15, 20);
const TEXT: Color32 = Color32::from_rgb(235, 238, 245);
const MUTED: Color32 = Color32::from_rgb(140, 146, 160);

pub fn pitch_knob(ui: &mut Ui, value: &mut f32) -> Response {
    let size = ui.available_width().clamp(160.0, 300.0);
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = size * 0.5;

    {
        let painter = ui.painter();
        painter.circle_filled(center, radius, BACKGROUND);
        painter.circle_stroke(center, radius, Stroke::new(1.0, DIM));
        painter.circle_filled(center, radius * 0.58, HUB);
        painter.circle_stroke(center, radius * 0.58, Stroke::new(1.5, TRACK));

        let angle_for = |value: f32| (value / 24.0).clamp(-1.0, 1.0) * 135.0_f32.to_radians();
        let point =
            |angle: f32, distance: f32| center + Vec2::new(angle.sin(), -angle.cos()) * distance;

        let arc_radius = radius * 0.86;
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
            let inner = radius * if major { 0.72 } else { 0.75 };
            let outer = radius * 0.79;
            painter.line_segment(
                [point(angle, inner), point(angle, outer)],
                Stroke::new(
                    if major { 1.5 } else { 1.0 },
                    if major { ACCENT } else { DIM },
                ),
            );
        }

        for semitone in [-24, -12, 0, 12, 24] {
            let label = if semitone == 0 {
                String::from("0")
            } else {
                format!("{semitone:+}")
            };
            painter.text(
                point(angle_for(semitone as f32), radius * 0.93),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional((radius * 0.075).clamp(9.0, 13.0)),
                MUTED,
            );
        }

        let pointer_inner = point(angle, radius * 0.64);
        let pointer_outer = point(angle, radius * 0.80);
        painter.line_segment([pointer_inner, pointer_outer], Stroke::new(4.0, ACCENT));
        painter.circle_filled(pointer_outer, 5.0, ACCENT);

        let display = if value.abs() < 0.005 { 0.0 } else { *value };
        painter.text(
            center + Vec2::new(0.0, -radius * 0.03),
            Align2::CENTER_CENTER,
            format!("{display:+.2}"),
            FontId::proportional(radius * 0.26),
            TEXT,
        );
        painter.text(
            center + Vec2::new(0.0, radius * 0.17),
            Align2::CENTER_CENTER,
            "semitones",
            FontId::proportional((radius * 0.075).clamp(9.0, 13.0)),
            MUTED,
        );
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
