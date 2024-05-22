use bevy::prelude::*;

#[derive(Component)]
pub struct DespawnOnFinish;

pub fn despawn_expired_animations(
    mut commands: Commands,
    mut anim_q: Query<(Entity, &AnimationState), With<DespawnOnFinish>>
) {
    for (entity, state) in anim_q.iter_mut() {
        if state.is_ended() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

#[derive(Clone, Component, Deref)]
pub struct Animation(pub benimator::Animation);

#[derive(Default, Component, Deref, DerefMut)]
pub struct AnimationState(pub benimator::State);

fn animate(
    time: Res<Time>,
    mut query: Query<(&mut AnimationState, &mut TextureAtlas, &Animation)>
) {
    for (mut state, mut texture, animation) in query.iter_mut() {
        state.update(animation, time.delta());

        texture.index = state.frame_index();
    }
}

pub struct AnimationPlugin;
impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPostUpdate, (animate, despawn_expired_animations).chain());
    }
}
