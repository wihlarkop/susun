//! Tests for feature-gated serde serialization of canonical model types.

#![allow(clippy::unwrap_used)]

#[cfg(feature = "serde")]
mod with_serde {
    use indexmap::IndexMap;
    use susun_model::{ImageRef, Project, ProjectName, Service, ServiceName};

    fn one_service_project() -> Project {
        let mut services = IndexMap::new();
        services.insert(
            ServiceName::new("web"),
            Service { image: Some(ImageRef::new("nginx:1.25")) },
        );
        Project { name: ProjectName::new("myapp"), services }
    }

    #[test]
    fn project_serializes_to_json() {
        let project = one_service_project();
        let json = serde_json::to_string_pretty(&project).unwrap();
        assert!(json.contains("\"name\": \"myapp\""));
        assert!(json.contains("\"web\""));
        assert!(json.contains("\"nginx:1.25\""));
    }

    #[test]
    fn project_roundtrips_through_json() {
        let original = one_service_project();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn service_with_no_image_serializes_null() {
        let service = Service { image: None };
        let json = serde_json::to_string(&service).unwrap();
        assert!(json.contains("null"));
    }

    #[test]
    fn newtype_serializes_as_plain_string() {
        let name = ProjectName::new("demo");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"demo\"");
    }
}
