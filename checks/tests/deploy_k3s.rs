//! Checks over `deploy/k3s/`, the scripts that put the review site on the home
//! cluster. The helpers in `lib.sh` are run for real under bash; the ordering
//! rules in `apply.sh` and `check-moderation.sh` are read from the scripts,
//! because running them needs a cluster.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn k3s_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("deploy")
        .join("k3s")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(k3s_dir().join(name)).unwrap()
}

/// Source `lib.sh` with the given environment and run `script` after it.
fn with_lib(env: &[(&str, &str)], script: &str) -> std::process::Output {
    let lib = k3s_dir().join("lib.sh");
    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(format!(". '{}'\n{script}", lib.display()))
        .env_remove("NS")
        .env_remove("K3S_NS");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Byte offset of `needle` in `haystack`, failing with the needle's text.
fn at(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("not found: {needle}"))
}

#[test]
fn an_inherited_ns_cannot_move_the_scripts_to_another_namespace() {
    let out = with_lib(&[("NS", "kube-system")], "echo \"$NS\"");
    assert!(out.status.success());
    assert_eq!(stdout(&out), "trovato-site");

    let out = with_lib(&[("K3S_NS", "kube-system")], "echo reached");
    assert!(!out.status.success(), "a foreign K3S_NS was accepted");
    assert_ne!(stdout(&out), "reached");
}

#[test]
fn the_docker_shim_refuses_any_other_namespace() {
    let out = Command::new("bash")
        .arg(k3s_dir().join("shim").join("docker"))
        .args(["compose", "exec", "-T", "site", "true"])
        .env("K3S_NS", "kube-system")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("refusing namespace"));
}

#[test]
fn only_a_redirect_counts_as_an_installed_site() {
    for (code, want) in [
        ("200", "fresh"),
        ("302", "installed"),
        ("303", "installed"),
        ("429", "error"),
        ("500", "error"),
        ("000", "error"),
        ("", "error"),
    ] {
        let out = with_lib(&[], &format!("install_state '{code}'"));
        assert_eq!(stdout(&out), want, "install_state {code:?}");
    }
}

#[test]
fn a_plugin_left_disabled_fails_the_deployment() {
    let ok = with_lib(
        &[],
        "printf 'trovato_blog enabled\\ntrovato_site enabled\\n' | require_enabled trovato_blog trovato_site",
    );
    assert!(ok.status.success());

    let bad = with_lib(
        &[],
        "printf 'trovato_blog enabled\\ntrovato_site disabled\\n' | require_enabled trovato_blog trovato_site",
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("trovato_site"));

    let missing = with_lib(
        &[],
        "printf 'trovato_blog enabled\\n' | require_enabled trovato_blog trovato_spam",
    );
    assert!(!missing.status.success());
}

#[test]
fn populate_does_not_shrug_off_a_failed_step() {
    let lib = read("lib.sh");
    assert!(!lib.contains("WARNING: could not enable"));
    assert!(!lib.contains("WARNING: plugin migrate"));
    assert!(!lib.contains("WARNING: the cron tick failed"));
    assert!(lib.contains("require_enabled \"${IMAGE_PLUGINS[@]}\" \"${SITE_PLUGINS[@]}\""));
    assert!(lib.contains("/it/why is not served in Italian"));
    // The search page's in-browser search needs the index, so a deployment
    // that does not serve one has failed.
    assert!(lib.contains("/static/pagefind/pagefind-entry.json"));
    assert!(lib.contains("the search index was not built"));
}

#[test]
fn the_staged_base_url_is_the_deployments_own_origin() {
    let dir = std::env::temp_dir().join(format!("k3s-base-url-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("variable.plugin.trovato_site.site_base_url.yml");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("config")
            .join("variable.plugin.trovato_site.site_base_url.yml"),
        &file,
    )
    .unwrap();

    let out = with_lib(
        &[],
        &format!(
            "set_plugin_base_url '{}' 'http://10.20.250.17.nip.io'",
            dir.display()
        ),
    );
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(out.status.success());
    assert!(text.contains("\nvalue: http://10.20.250.17.nip.io\n"));
    assert!(!text.contains("value: https://trovato.rs"));
    assert!(text.contains("key: plugin.trovato_site.site_base_url"));

    let apply = read("apply.sh");
    assert!(apply.contains("set_plugin_base_url \"$ctx/config\" \"http://${SITE_HOST}\""));
}

#[test]
fn the_diff_comes_before_anything_is_created() {
    let apply = read("apply.sh");
    let diff = at(&apply, "echo \"==> kubectl diff\"");
    for mutation in [
        "kubectl apply -f \"$here/namespace.yaml\"\n",
        "k create secret generic trovato-site-secrets",
        "kubectl apply -f \"$here/namespace.yaml\" -f \"$rendered\"",
    ] {
        assert!(
            diff < at(&apply, mutation),
            "{mutation:?} runs before the diff"
        );
    }
}

#[test]
fn the_installer_is_closed_to_the_lan() {
    let caddy = read("Caddyfile");
    let matcher = at(&caddy, "@installer path /install /install/*");
    let handle = at(&caddy, "handle @installer {\n\t\trespond 404\n\t}");
    let site = at(&caddy, "import site\n}");
    assert!(matcher < handle && handle < site);

    let apply = read("apply.sh");
    assert!(apply.contains("\"http://${LB_IP}/install/admin\""));
    assert!(apply.contains("expected 404"));
}

#[test]
fn the_push_is_never_relayed_through_a_node() {
    // ks1 to ks3 and kw1 carry the load tests; the image goes from this
    // machine to the registry or not at all.
    let apply = read("apply.sh");
    assert!(!apply.contains("ssh "), "apply.sh reaches a node over ssh");
    assert!(!apply.contains("podman"), "apply.sh relays through podman");
    assert!(!apply.contains("PUSH_HOST"));
    assert!(apply.contains("docker push \"$SITE_IMAGE\""));
    assert!(apply.contains("docker login registry.pozza -u jeremyandrews"));
}

#[test]
fn the_image_is_built_for_the_node_it_runs_on() {
    let apply = read("apply.sh");
    assert!(apply.contains("docker build --platform linux/amd64 "));
    assert!(!apply.contains("linux/arm64"));
}

#[test]
fn every_pod_is_pinned_to_kw3_and_none_to_the_load_test_nodes() {
    for name in ["site.yaml", "postgres.yaml", "redis.yaml", "modcheck.yaml"] {
        let m = read(name);
        assert!(
            m.contains("- key: kubernetes.io/hostname\n") && m.contains("values: [kw3.pozza]"),
            "{name} is not pinned to kw3"
        );
        assert!(
            !m.contains("kubernetes.io/arch"),
            "{name} still selects on arch"
        );
        for node in [
            "ks1.pozza",
            "ks2.pozza",
            "ks3.pozza",
            "kw1.pozza",
            "kw4.pozza",
        ] {
            assert!(!m.contains(node), "{name} names {node}");
        }
        assert!(
            m.contains("value: plex"),
            "{name} does not tolerate kw3's taint"
        );
    }
}

#[test]
fn the_lan_address_is_announced_for_a_pod_on_a_kw_node() {
    // No MetalLB speaker runs on the kw nodes, so Local would announce nothing.
    let site = read("site.yaml");
    assert!(site.contains("externalTrafficPolicy: Cluster"));
    assert!(site.contains("metallb.io/loadBalancerIPs: __LB_IP__"));
}

#[test]
fn a_second_moderation_check_cannot_clean_up_the_first() {
    let script = read("check-moderation.sh");
    let create = at(&script, "k create secret generic trovato-modcheck-secrets");
    let destructive = at(&script, "trap cleanup EXIT");
    assert!(
        create < destructive,
        "the label-selected cleanup is armed before this run owns the throwaway site"
    );
    let before = &script[..create];
    assert!(!before.contains("trap cleanup"));
}

/// The cron sidecar's shell command, as `site.yaml` writes it.
fn cron_command() -> String {
    read("site.yaml")
        .lines()
        .map(str::trim)
        .find_map(|l| {
            l.strip_prefix("- '")?
                .strip_suffix('\'')
                .filter(|c| c.contains("/cron/"))
        })
        .expect("site.yaml has the cron sidecar's command")
        .to_string()
}

/// A new pod has no search index until a cron tick builds one, and it takes
/// traffic once /health answers. So the sidecar's first tick has to come as
/// soon as the kernel is up, not one interval after a tick fired too early.
/// Run the real command against a `curl` that fails /health twice and logs
/// every call, with an interval far longer than the test waits.
#[test]
fn the_cron_sidecar_ticks_as_soon_as_the_kernel_answers() {
    let dir = std::env::temp_dir().join(format!("trovato-cron-sidecar-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("calls.log");
    let stub = dir.join("curl");
    std::fs::write(
        &stub,
        format!(
            "#!/bin/sh\n\
             log='{log}'\n\
             case \"$*\" in\n\
             *health*)\n\
               n=$(grep -c health \"$log\" 2>/dev/null || true)\n\
               echo health >> \"$log\"\n\
               [ \"${{n:-0}}\" -ge 2 ] || exit 7 ;;\n\
             *cron*) echo cron >> \"$log\" ;;\n\
             esac\n\
             exit 0\n",
            log = log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Its own process group, so the sleep it leaves running is killed with it.
    use std::os::unix::process::CommandExt;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cron_command())
        .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
        .env("CRON_KEY", "test-key")
        .env("CRON_INTERVAL", "3600")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    let mut calls = String::new();
    while start.elapsed() < std::time::Duration::from_secs(20) {
        calls = std::fs::read_to_string(&log).unwrap_or_default();
        if calls.contains("cron") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Command::new("kill")
        .args(["-KILL", &format!("-{}", child.id())])
        .status()
        .unwrap();
    child.wait().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let calls: Vec<&str> = calls.lines().collect();
    assert_eq!(
        calls,
        ["health", "health", "health", "cron"],
        "the sidecar polls /health until it answers, then ticks without waiting an interval"
    );
}
