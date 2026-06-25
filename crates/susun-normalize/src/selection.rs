//! Active service selection for Compose profiles.

use indexmap::IndexSet;
use susun_model::{Project, ServiceName};

/// Services selected for operations after applying active profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSelection {
    /// Active service names in project order.
    pub active_services: IndexSet<ServiceName>,
}

/// Compute active services from the canonical project and requested profiles.
pub fn select_services(project: &Project, profiles: &[String]) -> ProjectSelection {
    let active_profiles: IndexSet<&str> = profiles.iter().map(String::as_str).collect();
    let active_services = project
        .services
        .iter()
        .filter_map(|(name, service)| {
            if service.profiles.is_empty()
                || service
                    .profiles
                    .iter()
                    .any(|profile| active_profiles.contains(profile.as_str()))
            {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    ProjectSelection { active_services }
}
