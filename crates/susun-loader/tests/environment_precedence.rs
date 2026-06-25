#![allow(missing_docs)]

use std::{collections::BTreeMap, error::Error, path::PathBuf};

use susun_diagnostics::DiagnosticReport;
use susun_loader::{DotenvEntry, EnvResolver, MapEnvironment, environment::dotenv::parse_dotenv};
use susun_source::{MemorySourceProvider, SourceMap, SourceProvider, SourceRequest};

type TestResult = Result<(), Box<dyn Error>>;

fn parse_env_str(contents: &str) -> Result<Vec<DotenvEntry>, Box<dyn Error>> {
    let path = PathBuf::from(".env");
    let provider = MemorySourceProvider::with_files([(path.clone(), contents)]);
    let loaded = provider.read(&SourceRequest::new(&path))?;
    let mut sm = SourceMap::new();
    let source_id = sm.register(loaded);
    let mut report = DiagnosticReport::new();
    Ok(parse_dotenv(source_id, contents, &mut report))
}

fn make_process(pairs: &[(&str, &str)]) -> MapEnvironment {
    MapEnvironment::new(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
}

struct Case {
    desc: &'static str,
    process: &'static [(&'static str, &'static str)],
    env_file: &'static str,
    dotenv: &'static str,
    key: &'static str,
    expected: Option<&'static str>,
}

/// Table-driven test covering all seven distinct precedence scenarios.
#[test]
fn precedence_table() -> TestResult {
    let cases: &[Case] = &[
        Case {
            desc: "process env wins over env-file and dotenv",
            process: &[("A", "proc")],
            env_file: "A=file\n",
            dotenv: "A=dot\n",
            key: "A",
            expected: Some("proc"),
        },
        Case {
            desc: "env-file wins over dotenv when process is unset",
            process: &[],
            env_file: "A=file\n",
            dotenv: "A=dot\n",
            key: "A",
            expected: Some("file"),
        },
        Case {
            desc: "dotenv is last resort when process and env-file are unset",
            process: &[],
            env_file: "",
            dotenv: "A=dot\n",
            key: "A",
            expected: Some("dot"),
        },
        Case {
            desc: "returns None when key is absent in all three layers",
            process: &[],
            env_file: "",
            dotenv: "",
            key: "A",
            expected: None,
        },
        Case {
            desc: "key found only in process env",
            process: &[("ONLY_PROC", "yes")],
            env_file: "",
            dotenv: "",
            key: "ONLY_PROC",
            expected: Some("yes"),
        },
        Case {
            desc: "key found only in env-file",
            process: &[],
            env_file: "ONLY_FILE=yes\n",
            dotenv: "",
            key: "ONLY_FILE",
            expected: Some("yes"),
        },
        Case {
            desc: "key found only in dotenv",
            process: &[],
            env_file: "",
            dotenv: "ONLY_DOT=yes\n",
            key: "ONLY_DOT",
            expected: Some("yes"),
        },
        Case {
            desc: "unrelated keys in lower layers do not bleed up",
            process: &[("PROC", "p")],
            env_file: "FILE=f\n",
            dotenv: "DOT=d\n",
            key: "FILE",
            expected: Some("f"),
        },
    ];

    for case in cases {
        let process = make_process(case.process);
        let env_file = parse_env_str(case.env_file)?;
        let dotenv = parse_env_str(case.dotenv)?;
        let resolver = EnvResolver::new(process, env_file, dotenv);
        assert_eq!(
            resolver.get(case.key).as_deref(),
            case.expected,
            "case: {}",
            case.desc,
        );
    }
    Ok(())
}

#[test]
fn empty_resolver_returns_none_for_any_key() {
    let resolver = EnvResolver::new(MapEnvironment::default(), vec![], vec![]);
    assert_eq!(resolver.get("ANYTHING"), None);
}

#[test]
fn process_env_shadows_env_file_value() -> TestResult {
    let process = make_process(&[("PORT", "9000")]);
    let env_file = parse_env_str("PORT=3000\n")?;
    let resolver = EnvResolver::new(process, env_file, vec![]);
    assert_eq!(resolver.get("PORT").as_deref(), Some("9000"));
    Ok(())
}

#[test]
fn env_file_shadows_dotenv_value() -> TestResult {
    let env_file = parse_env_str("DB=main\n")?;
    let dotenv = parse_env_str("DB=test\n")?;
    let resolver = EnvResolver::new(MapEnvironment::default(), env_file, dotenv);
    assert_eq!(resolver.get("DB").as_deref(), Some("main"));
    Ok(())
}

#[test]
fn resolver_returns_correct_values_for_multiple_distinct_keys() -> TestResult {
    let process = make_process(&[("HOST", "localhost")]);
    let env_file = parse_env_str("PORT=8080\n")?;
    let dotenv = parse_env_str("MODE=production\n")?;
    let resolver = EnvResolver::new(process, env_file, dotenv);
    assert_eq!(resolver.get("HOST").as_deref(), Some("localhost"));
    assert_eq!(resolver.get("PORT").as_deref(), Some("8080"));
    assert_eq!(resolver.get("MODE").as_deref(), Some("production"));
    assert_eq!(resolver.get("MISSING"), None);
    Ok(())
}
