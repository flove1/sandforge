use bevy::prelude::*;
use bevy_rapier2d::dynamics::Velocity;

use crate::animation::AnimationState;

use super::actor::Actor;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct IdleAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct MoveAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct FallAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct HurtAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct AttackAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct LandAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct JumpAnimation;

pub fn create_run_trigger(
    threshold: f32
) -> impl (Fn(In<Entity>, Query<&Velocity, With<Actor>>) -> Result<(), ()>) + Copy{
    move |In(entity): In<Entity>, actor_q: Query<&Velocity, With<Actor>>| -> Result<(), ()> {
        match actor_q.get(entity).unwrap().linvel.x.abs() >= threshold {
            true => Ok(()),
            false => Err(()),
        }
    }
}

pub fn create_animation_end_trigger() -> impl (Fn(In<Entity>, Query<&AnimationState, With<Actor>>) -> Result<(), ()>) + Copy {
    move |In(entity): In<Entity>, animation_q: Query<&AnimationState, With<Actor>>| {
        match animation_q.get(entity).unwrap().is_ended() {
            true => Ok(()),
            false => Err(()),
        }
    }
}