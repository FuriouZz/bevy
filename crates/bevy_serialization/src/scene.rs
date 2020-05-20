use crate::ComponentRegistration;
use legion::{
    prelude::{Entity, World},
    storage::{ComponentMeta, ComponentStorage, ComponentTypeId, ComponentResourceSet},
};
use serde::{
    ser::{Serialize, SerializeSeq, SerializeStruct},
    Deserialize,
};
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub struct Scene {
    pub world: World,
}

#[derive(Default)]
pub struct ComponentRegistry {
    pub registrations: HashMap<ComponentTypeId, ComponentRegistration>,
}

impl ComponentRegistry {
    pub fn register<T>(&mut self)
    where
        T: Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
    {
        let registration = ComponentRegistration::of::<T>();
        self.registrations.insert(registration.ty, registration);
    }

    pub fn get(&self, type_id: ComponentTypeId) -> Option<&ComponentRegistration> {
        self.registrations.get(&type_id)
    }
}

pub struct SerializableScene<'a> {
    pub scene: &'a Scene,
    pub component_registry: &'a ComponentRegistry,
}

impl<'a> SerializableScene<'a> {
    pub fn new(scene: &'a Scene, component_registry: &'a ComponentRegistry) -> Self {
        SerializableScene {
            scene,
            component_registry,
        }
    }
}

impl<'a> Serialize for SerializableScene<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.scene.world.iter_entities().count()))?;
        for archetype in self.scene.world.storage().archetypes() {
            for chunkset in archetype.chunksets() {
                for component_storage in chunkset.occupied() {
                    for (index, entity) in component_storage.entities().iter().enumerate() {
                        seq.serialize_element(&WorldEntity {
                            index,
                            archetype_components: archetype.description().components(),
                            component_registry: &self.component_registry,
                            component_storage,
                            entity: *entity,
                        })?;
                    }
                }
            }
        }
        // for entity in self.scene.world.iter_entities() {
        //     seq.serialize_element(&WorldEntity {
        //         world: &self.scene.world,
        //         component_registry: &self.component_registry,
        //         entity,
        //     })?;
        // }

        seq.end()
    }
}

struct WorldEntity<'a> {
    archetype_components: &'a [(ComponentTypeId, ComponentMeta)],
    component_registry: &'a ComponentRegistry,
    component_storage: &'a ComponentStorage,
    entity: Entity,
    index: usize,
}

impl<'a> Serialize for WorldEntity<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Entity", 2)?;
        state.serialize_field("id", &self.entity.index())?;
        state.serialize_field(
            "components",
            &EntityComponents {
                archetype_components: self.archetype_components,
                component_registry: self.component_registry,
                component_storage: self.component_storage,
                index: self.index,
            },
        )?;
        state.end()
    }
}

struct EntityComponents<'a> {
    index: usize,
    archetype_components: &'a [(ComponentTypeId, ComponentMeta)],
    component_storage: &'a ComponentStorage,
    component_registry: &'a ComponentRegistry,
}

impl<'a> Serialize for EntityComponents<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.archetype_components.len()))?;
        for (component_type, _) in self.archetype_components.iter() {
            seq.serialize_element(&EntityComponent {
                index: self.index,
                component_resource_set: self.component_storage.components(*component_type).unwrap(),
                component_registration: self.component_registry.get(*component_type).unwrap(),
            })?;
        }
        seq.end()
    }
}

struct EntityComponent<'a> {
    index: usize,
    component_resource_set: &'a ComponentResourceSet,
    component_registration: &'a ComponentRegistration,
}

impl<'a> Serialize for EntityComponent<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut result = None;
        let serializer = RefCell::new(Some(serializer));
        (self.component_registration.individual_comp_serialize_fn)(
            self.component_resource_set,
            self.index,
            &mut |serialize| {
                result = Some(erased_serde::serialize(
                    serialize,
                    serializer.borrow_mut().take().unwrap(),
                ));
            },
        );

        result.unwrap()
    }
}
