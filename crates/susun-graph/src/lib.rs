//! Deterministic dependency graph construction.

use indexmap::{IndexMap, IndexSet};
use susun_diagnostics::{Diagnostic, DiagnosticReport, Severity};
use susun_model::{Project, ServiceName};
use susun_normalize::selection::ProjectSelection;

const CYCLE: &str = "SUS-GRAPH-001";

/// Dependency graph for selected services.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    /// Services in deterministic topological order.
    pub order: Vec<ServiceName>,
    /// Dependency edges keyed by service name.
    pub edges: IndexMap<ServiceName, IndexSet<ServiceName>>,
}

/// Graph build result.
pub struct GraphOutcome {
    /// Dependency graph, absent when a cycle is detected.
    pub graph: Option<DependencyGraph>,
    /// Diagnostics emitted during graph construction.
    pub report: DiagnosticReport,
}

/// Build a dependency graph for active services.
pub fn build_graph(project: &Project, selection: &ProjectSelection) -> GraphOutcome {
    let mut report = DiagnosticReport::new();
    let active = &selection.active_services;
    let mut edges: IndexMap<ServiceName, IndexSet<ServiceName>> = IndexMap::new();
    let mut indegree: IndexMap<ServiceName, usize> = IndexMap::new();

    for service in active {
        edges.insert(service.clone(), IndexSet::new());
        indegree.insert(service.clone(), 0);
    }

    for service_name in active {
        let Some(service) = project.services.get(service_name) else {
            continue;
        };
        for dependency_name in service.depends_on.keys() {
            if !active.contains(dependency_name) {
                continue;
            }
            if let Some(dependents) = edges.get_mut(dependency_name) {
                if dependents.insert(service_name.clone()) {
                    *indegree.entry(service_name.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut ready: Vec<ServiceName> = active
        .iter()
        .filter(|service| indegree.get(*service).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    let mut order = Vec::new();

    while let Some(service) = ready.first().cloned() {
        ready.remove(0);
        order.push(service.clone());

        let Some(dependents) = edges.get(&service) else {
            continue;
        };
        for dependent in dependents {
            let Some(count) = indegree.get_mut(dependent) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.push(dependent.clone());
            }
        }
    }

    if order.len() != active.len() {
        report.push(Diagnostic::new(
            CYCLE,
            Severity::Error,
            "dependency cycle detected among active services",
        ));
        return GraphOutcome {
            graph: None,
            report,
        };
    }

    GraphOutcome {
        graph: Some(DependencyGraph { order, edges }),
        report,
    }
}
