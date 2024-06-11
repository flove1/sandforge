use bevy::prelude::*;
use bevy_tween::{
    component_tween_system,
    interpolate::Interpolator,
    BevyTweenRegisterSystems,
    DefaultTweenPlugins,
};

fn color_lerp(start: Color, end: Color, v: f32) -> Color {
    let Color::Rgba {
        red: start_red,
        green: start_green,
        blue: start_blue,
        alpha: start_alpha,
    } = start.as_rgba() else {
        unreachable!()
    };

    let Color::Rgba {
        red: end_red,
        green: end_green,
        blue: end_blue,
        alpha: end_alpha,
    } = end.as_rgba() else {
        unreachable!()
    };

    Color::Rgba {
        red: start_red.lerp(end_red, v),
        green: start_green.lerp(end_green, v),
        blue: start_blue.lerp(end_blue, v),
        alpha: start_alpha.lerp(end_alpha, v),
    }
}

pub struct InterpolateTopOffset {
    pub start: Val,
    pub end: Val,
}

impl Interpolator for InterpolateTopOffset {
    type Item = Style;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        match &mut item.top {
            Val::Px(offset) => {
                let Val::Px(start_offset) = self.start else { unreachable!() };
                let Val::Px(end_offset) = self.end else { unreachable!() };

                *offset = start_offset + (end_offset - start_offset) * value;
            }
            _ => {
                let Val::Px(start_offset) = self.start else { unreachable!() };
                let Val::Px(end_offset) = self.end else { unreachable!() };

                item.top = Val::Px(start_offset + (end_offset - start_offset) * value);
            }
        }
    }
}

pub struct InterpolatePadding {
    pub start: [f32; 4],
    pub end: [f32; 4],
}

impl Interpolator for InterpolatePadding {
    type Item = Style;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        item.padding = UiRect {
            left: Val::Px(self.start[0] + (self.end[0] - self.start[0]) * value),
            right: Val::Px(self.start[1] + (self.end[1] - self.start[1]) * value),
            top: Val::Px(self.start[2] + (self.end[2] - self.start[2]) * value),
            bottom: Val::Px(self.start[3] + (self.end[3] - self.start[3]) * value),
        };
    }
}

pub struct InterpolateSize {
    pub start: Vec2,
    pub end: Vec2,
}

impl Interpolator for InterpolateSize {
    type Item = Style;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        item.width = Val::Px(self.start.x + (self.end.x - self.start.x) * value);
        item.height = Val::Px(self.start.y + (self.end.y - self.start.y) * value);
    }
}

pub struct InterpolateBackgroundColor {
    pub start: Color,
    pub end: Color,
}

impl Interpolator for InterpolateBackgroundColor {
    type Item = BackgroundColor;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        item.0 = color_lerp(self.start, self.end, value);
    }
}

pub struct InterpolateTextColor {
    pub start: Color,
    pub end: Color,
}

impl Interpolator for InterpolateTextColor {
    type Item = Text;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        for section in item.sections.iter_mut() {
            section.style.color = color_lerp(self.start, self.end, value);
        }
    }
}

pub struct InterpolateVolume {
    pub start: f32,
    pub end: f32,
}

impl Interpolator for InterpolateVolume {
    type Item = AudioSink;

    fn interpolate(&self, item: &mut Self::Item, value: f32) {
        item.set_volume(self.start + (self.end - self.start) * value);
    }
}

pub struct InterpolatorPlugin;

impl Plugin for InterpolatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultTweenPlugins)
            .add_tween_systems(component_tween_system::<InterpolateSize>())
            .add_tween_systems(component_tween_system::<InterpolateBackgroundColor>())
            .add_tween_systems(component_tween_system::<InterpolatePadding>())
            .add_tween_systems(component_tween_system::<InterpolateTopOffset>())
            .add_tween_systems(component_tween_system::<InterpolateTextColor>())
            .add_tween_systems(component_tween_system::<InterpolateVolume>());
    }
}
