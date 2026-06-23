//! Tests for feature-gated serde serialization of canonical model types.
#![allow(missing_docs)]

#[cfg(feature = "serde")]
mod with_serde {
    use std::error::Error;

    use indexmap::IndexMap;
    use susun_model::{Command, ImageRef, Project, ProjectName, Service, ServiceName};

    type TestResult = Result<(), Box<dyn Error>>;

    fn one_service_project() -> Project {
        let mut services = IndexMap::new();
        services.insert(
            ServiceName::new("web"),
            Service { image: Some(ImageRef::new("nginx:1.25")), ..Service::default() },
        );
        Project { name: ProjectName::new("myapp"), services }
    }

    #[test]
    fn project_serializes_to_json() -> TestResult {
        let project = one_service_project();
        let json = serde_json::to_string_pretty(&project)?;
        assert!(json.contains("\"name\": \"myapp\""));
        assert!(json.contains("\"web\""));
        assert!(json.contains("\"nginx:1.25\""));
        Ok(())
    }

    #[test]
    fn project_roundtrips_through_json() -> TestResult {
        let original = one_service_project();
        let json = serde_json::to_string(&original)?;
        let restored: Project = serde_json::from_str(&json)?;
        assert_eq!(original, restored);
        Ok(())
    }

    #[test]
    fn service_with_no_image_omits_image_field() -> TestResult {
        let service = Service::default();
        let json = serde_json::to_string(&service)?;
        // Absence is preserved: no fields are emitted for a default service.
        assert_eq!(json, "{}");
        Ok(())
    }

    #[test]
    fn service_with_command_roundtrips() -> TestResult {
        let service = Service {
            command: Some(Command::Exec(vec!["nginx".into(), "-g".into(), "daemon off;".into()])),
            ..Service::default()
        };
        let json = serde_json::to_string(&service)?;
        let restored: Service = serde_json::from_str(&json)?;
        assert_eq!(service, restored);
        Ok(())
    }

    #[test]
    fn newtype_serializes_as_plain_string() -> TestResult {
        let name = ProjectName::new("demo");
        let json = serde_json::to_string(&name)?;
        assert_eq!(json, "\"demo\"");
        Ok(())
    }
}
