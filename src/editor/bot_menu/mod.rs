use std::any::TypeId;

use bevy::{prelude::*, ecs::{entity::EntityMap, reflect::ReflectMapEntities}, utils::HashMap};
use bevy_egui::*;

use crate::{prefab::{save::{SaveState, SaveConfig}, PrefabPlugin}, PrefabMarker, prelude::show_hierarchy, EditorState, EditorSet};

#[derive(Resource, Default, Clone)]
pub struct EditorLoader {
    pub scene : Option<Handle<DynamicScene>>
}

pub struct BotMenuPlugin;

impl Plugin for BotMenuPlugin {
    fn build(&self, app: &mut App) {

        if !app.is_plugin_added::<PrefabPlugin>() {
            app.add_plugins(PrefabPlugin);
        }
        app.init_resource::<EditorLoader>();

        app.add_systems(Update, bot_menu
            .after(super::inspector::inspect)
            .after(show_hierarchy)
            .in_set(EditorSet::Editor));
        app.add_systems(Update, (apply_deferred, load_listener).chain().after(bot_menu).in_set(EditorSet::Editor));
        app.add_systems(Update, bot_menu_game.in_set(EditorSet::Game));
    }
}

fn bot_menu_game(
    mut ctxs : EguiContexts,
    mut state : ResMut<NextState<EditorState>>
) {
    egui::TopBottomPanel::bottom("bot_panel").show(ctxs.ctx_mut(), |ui| {
        ui.vertical_centered(|ui| {
            if ui.button("⏸").clicked() {
                state.set(EditorState::Editor);
            }
        });
    });
}

fn bot_menu(
    mut commands : Commands,
    mut ctxs : EguiContexts,
    mut save_confg : ResMut<SaveConfig>,
    mut save_state : ResMut<NextState<SaveState>>,
    mut assets : ResMut<AssetServer>,
    mut load_server : ResMut<EditorLoader>,
    mut state : ResMut<NextState<EditorState>>
) {
    let ctx = ctxs.ctx_mut();
    egui::TopBottomPanel::bottom("bot_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {

            ui.label("Save path:");
            ui.add(egui::TextEdit::singleline(&mut save_confg.path));

            if ui.button("Save").clicked() {
                save_state.set(SaveState::Save);
            }

            if ui.button("Load").clicked() {
                if !save_confg.path.is_empty() {
                    load_server.scene = Some(
                        assets.load(format!("{}.scn.ron",save_confg.path))
                    );
                }
                // TODO: else show notification with information
            }

            if ui.button("▶").clicked() {
                state.set(EditorState::GamePrepare);
            }
        });
    });
}

fn load_listener(
    world : &mut World
) {
    let app_registry = world.resource::<AppTypeRegistry>().clone();
    let load_server = world.resource::<EditorLoader>().clone();
    let prefab;
    {
        let assets = world.resource::<Assets<DynamicScene>>();
        if let Some(scene) = &load_server.scene {
            if let Some(scene) = assets.get(scene) {
                let mut scene = Scene::from_dynamic_scene(scene, &app_registry).unwrap();
                scene.world.insert_resource(app_registry.clone());
                prefab = DynamicScene::from_scene(&scene); //kill me, is it clone() analog for DynamicScene
            } else {
                return;
            }
        } else {
            return;
        }
    }
    world.resource_mut::<EditorLoader>().scene = None;
    let type_registry = app_registry.read();
   
    let mut map = EntityMap::default();
    let mut scene_mappings: HashMap<TypeId, Vec<Entity>> = HashMap::default();

    let  mut query = world.query_filtered::<Entity, With<PrefabMarker>>();
    let mark_to_delete : Vec<_> = query.iter(&world).collect();
    for entity in mark_to_delete {
        world.entity_mut(entity).despawn_recursive();
    }

    for scene_entity in prefab.entities.iter() {
        let mut entity = map.get(scene_entity.entity).unwrap_or_else(
                || world.spawn_empty().id()
        );

        let mut entity_mut = world.entity_mut(entity);

        // Apply/ add each component to the given entity.
        for component in &scene_entity.components {
            let Some(registration) = type_registry
                .get_with_name(component.type_name()) else {
                    error!("Cannot find component registration in editor prefab loader");
                    return;
                };
            let Some(reflect_component) =
                registration.data::<ReflectComponent>() else {
                error!("Cannot reglect serialized component in editor prefab loader");
                return;
            };

            // If this component references entities in the scene, track it
            // so we can update it to the entity in the world.
            if registration.data::<ReflectMapEntities>().is_some() {
                scene_mappings
                    .entry(registration.type_id())
                    .or_insert(Vec::new())
                    .push(entity);
            }

            // If the entity already has the given component attached,
            // just apply the (possibly) new value, otherwise add the
            // component to the entit
            reflect_component.apply_or_insert(&mut entity_mut, &**component);

            entity_mut.insert(PrefabMarker);
        }
    }

}
