#![allow(clippy::disallowed_methods)]

//! Privacy by design: the core library keeps every secret on the machine. It must not pull an
//! HTTP client, a telemetry/analytics SDK or a crash reporter, even transitively, and its code
//! must not open a socket. The dependency check walks the resolved graph in `Cargo.lock`, so a
//! new transitive dependency is caught even if no `Cargo.toml` line mentions it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Crates that only exist to send data over the network, or to report on the user.
const FORBIDDEN: &[&str] = &[
    // HTTP / WebSocket clients
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "surf",
    "attohttpc",
    "curl",
    "awc",
    "tungstenite",
    "tokio-tungstenite",
    "h2",
    // Telemetry, analytics, crash reporting
    "sentry",
    "opentelemetry",
    "opentelemetry-otlp",
    "tracing-opentelemetry",
    "posthog-rs",
    "segment",
    "datadog-apm",
    "mixpanel",
    "rudderanalytics",
    "crash-handler",
    "minidumper",
];

fn workspace_lock() -> String {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    std::fs::read_to_string(&lock).expect("workspace Cargo.lock")
}

/// `name -> dependency names` for every package of the lock file.
fn dependency_graph(lock: &str) -> BTreeMap<String, Vec<String>> {
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for block in lock.split("[[package]]").skip(1) {
        let mut name = None;
        let mut dependencies = Vec::new();
        let mut in_dependencies = false;
        for line in block.lines().map(str::trim) {
            if let Some(value) = line.strip_prefix("name = ") {
                name = Some(value.trim_matches('"').to_string());
            } else if line.starts_with("dependencies = [") {
                in_dependencies = !line.ends_with(']');
            } else if in_dependencies {
                if line == "]" {
                    in_dependencies = false;
                } else if let Some(entry) = line.trim_end_matches(',').strip_prefix('"') {
                    // "name", "name version" or "name version (source)"
                    let dependency = entry.trim_end_matches('"');
                    if let Some(dep_name) = dependency.split_whitespace().next() {
                        dependencies.push(dep_name.to_string());
                    }
                }
            }
        }
        if let Some(name) = name {
            graph.entry(name).or_default().extend(dependencies);
        }
    }
    graph
}

fn closure(graph: &BTreeMap<String, Vec<String>>, root: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(package) = stack.pop() {
        if seen.insert(package.clone())
            && let Some(dependencies) = graph.get(&package)
        {
            stack.extend(dependencies.iter().cloned());
        }
    }
    seen
}

#[test]
fn the_lock_file_parser_sees_the_real_graph() {
    let graph = dependency_graph(&workspace_lock());
    let core = closure(&graph, "heelonvault-core");
    for expected in ["aes-gcm", "argon2", "zeroize", "secrecy", "sqlx-sqlite"] {
        assert!(
            core.contains(expected),
            "{expected} missing: the parser no longer reads Cargo.lock correctly"
        );
    }
}

#[test]
fn heelonvault_core_pulls_no_network_client_nor_telemetry_even_transitively() {
    let graph = dependency_graph(&workspace_lock());
    let core = closure(&graph, "heelonvault-core");

    let offending: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|name| core.contains(*name))
        .collect();

    assert!(
        offending.is_empty(),
        "heelonvault-core must stay local-only, but depends on {offending:?}"
    );
}

fn rust_sources(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read source dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "rs" || extension == "inc")
        {
            found.push(path);
        }
    }
}

#[test]
fn the_code_opens_no_socket_and_calls_no_remote_endpoint() {
    let roots = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../heelonvault-app/src"),
    ];
    let forbidden_apis = [
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "UnixStream",
        "reqwest::",
        "hyper::",
        "ureq::",
        "sentry::",
        "opentelemetry",
    ];

    let mut sources = Vec::new();
    for root in &roots {
        rust_sources(root, &mut sources);
    }
    assert!(sources.len() > 20, "source scan found too few files");

    let mut violations = Vec::new();
    for path in &sources {
        let content = std::fs::read_to_string(path).expect("read source");
        for (number, line) in content.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for api in forbidden_apis {
                if code.contains(api) {
                    violations.push(format!("{}:{}: {api}", path.display(), number + 1));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "network access outside the reviewed premium modules:\n{}",
        violations.join("\n")
    );
}
