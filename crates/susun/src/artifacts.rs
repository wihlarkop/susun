//! Versioned display-safe artifact and build summaries.

use serde::{Deserialize, Serialize, de::Error as _};

use crate::{ArtifactMutationSchemaVersion, ImagePushResult, ImageRemoveResult, ImageTagResult};

/// Display-safe build result for local APIs and persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildResultSummary {
    /// Serialized contract version.
    pub schema_version: ArtifactMutationSchemaVersion,
    /// Resulting image reference.
    pub reference: String,
    /// Immutable digest when reported by the builder.
    pub digest: Option<String>,
}

impl From<&susun_build::BuildResult> for BuildResultSummary {
    fn from(result: &susun_build::BuildResult) -> Self {
        Self {
            schema_version: ArtifactMutationSchemaVersion::CURRENT,
            reference: result.image.reference.clone(),
            digest: result.image.digest.clone(),
        }
    }
}

macro_rules! json_helpers {
    ($render:ident, $parse:ident, $type:ty) => {
        #[doc = "Renders this artifact result as pretty JSON."]
        pub fn $render(value: &$type) -> Result<String, serde_json::Error> {
            serde_json::to_string_pretty(value)
        }

        #[doc = "Parses and validates this artifact result JSON."]
        pub fn $parse(input: &str) -> Result<$type, serde_json::Error> {
            let value: $type = serde_json::from_str(input)?;
            validate_schema(value.schema_version)?;
            Ok(value)
        }
    };
}

json_helpers!(
    render_image_remove_result_json,
    parse_image_remove_result_json,
    ImageRemoveResult
);
json_helpers!(
    render_image_tag_result_json,
    parse_image_tag_result_json,
    ImageTagResult
);
json_helpers!(
    render_image_push_result_json,
    parse_image_push_result_json,
    ImagePushResult
);
json_helpers!(
    render_build_result_summary_json,
    parse_build_result_summary_json,
    BuildResultSummary
);

fn validate_schema(version: ArtifactMutationSchemaVersion) -> Result<(), serde_json::Error> {
    if version != ArtifactMutationSchemaVersion::CURRENT {
        return Err(serde_json::Error::custom(format!(
            "unsupported artifact mutation schema version {}.{}",
            version.major, version.minor
        )));
    }
    Ok(())
}
