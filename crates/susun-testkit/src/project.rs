//! Project fixture builders.

use indexmap::IndexMap;
use susun_model::{
    Configs, ImageRef, Networks, Project, ProjectName, Secrets, Service, ServiceName, Volumes,
};

/// Builder for canonical Susun projects.
#[derive(Debug, Clone)]
pub struct ProjectBuilder {
    name: ProjectName,
    services: IndexMap<ServiceName, Service>,
    networks: Networks,
    volumes: Volumes,
    configs: Configs,
    secrets: Secrets,
}

impl ProjectBuilder {
    /// Creates a minimal project builder.
    pub fn new(name: impl Into<ProjectName>) -> Self {
        Self {
            name: name.into(),
            services: IndexMap::new(),
            networks: IndexMap::new(),
            volumes: IndexMap::new(),
            configs: IndexMap::new(),
            secrets: IndexMap::new(),
        }
    }

    /// Adds a service with an image.
    pub fn service_with_image(
        mut self,
        name: impl Into<ServiceName>,
        image: impl Into<ImageRef>,
    ) -> Self {
        let service = Service {
            image: Some(image.into()),
            ..Service::default()
        };
        self.services.insert(name.into(), service);
        self
    }

    /// Adds a service definition.
    pub fn service(mut self, name: impl Into<ServiceName>, service: Service) -> Self {
        self.services.insert(name.into(), service);
        self
    }

    /// Builds the project.
    pub fn build(self) -> Project {
        Project {
            name: self.name,
            services: self.services,
            networks: self.networks,
            volumes: self.volumes,
            configs: self.configs,
            secrets: self.secrets,
        }
    }
}

impl Default for ProjectBuilder {
    fn default() -> Self {
        Self::new("susun")
    }
}
