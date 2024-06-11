use bevy::{
    core_pipeline::core_2d::graph::{ Core2d, Node2d },
    prelude::*,
    render::{
        extract_component::ExtractComponentPlugin, render_graph::{ RenderGraphApp, ViewNodeRunner }, RenderApp
    },
};

use self::{
    light_apply::{LightApply, ApplyLightingLabel, ApplyLightingNode, ApplyLightingPipeline}, light_propagate::{ LightPropagationLabel, LightPropagationNode, LightPropagationPipeline, LightPropagationSettings }, light_calculate::{ CalculateLightingLabel, CalculateLightingNode, CalculateLightingPipeline, LightMask }
};

pub mod light_propagate;
pub mod light_calculate;
pub mod light_apply;

pub struct PostProcessPlugin;

impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<LightPropagationSettings>::default(),
            ExtractComponentPlugin::<LightMask>::default(),
            ExtractComponentPlugin::<LightApply>::default(),
        ));

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<LightPropagationNode>>(
                Core2d,
                LightPropagationLabel
            )
            .add_render_graph_node::<ViewNodeRunner<CalculateLightingNode>>(
                Core2d,
                CalculateLightingLabel
            )
            .add_render_graph_node::<ViewNodeRunner<ApplyLightingNode>>(
                Core2d,
                ApplyLightingLabel
            )
            .add_render_graph_edges(
                Core2d,
                (Node2d::Tonemapping, CalculateLightingLabel, LightPropagationLabel, ApplyLightingLabel, Node2d::EndMainPassPostProcessing)
            );
    }

    fn finish(&self, app: &mut App) {
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<CalculateLightingPipeline>()
            .init_resource::<LightPropagationPipeline>()
            .init_resource::<ApplyLightingPipeline>();
    }
}