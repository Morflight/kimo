//! Crash recovery keeps Undo: the desktop app autosaves with `save_project with_history`, and after a crash restores with
//! `load_project with_history restore`. The restored session must undo exactly like the one that crashed: no extra
//! "load" step on top, every saved step reachable, and redo of what was undone.
use kimo_core::commands::{dispatch, Session};
use serde_json::{json, Value};

fn c(s: &mut Session, n: &str, a: Value) -> Value { dispatch(s, n, &a).unwrap_or_else(|e| panic!("{n}: {e}")) }
fn widths(s: &mut Session) -> (u32, usize) { (s.project.width, c(s, "history_list", json!({}))["undo"].as_array().unwrap().len()) }

#[test]
fn restore_brings_back_the_exact_undo_stack() {
    let p = std::env::temp_dir().join(format!("kimo_hr_{}.kimo.autosave", std::process::id()));
    let ps = p.to_str().unwrap();
    // the session that will crash: three edits
    let mut a = Session::new();
    for w in [320, 640, 1280] { c(&mut a, "project_set", json!({"width": w, "height": 180, "fps": 24})); }
    assert_eq!(widths(&mut a), (1280, 3));
    let r = c(&mut a, "save_project", json!({"path": ps, "with_history": true, "history_steps": 50}));
    assert_eq!(r["undo_steps"], 3, "{r}");
    drop(a); // the crash

    // the next launch: a fresh engine restores the autosave
    let mut b = Session::new();
    let r = c(&mut b, "load_project", json!({"path": ps, "with_history": true, "restore": true}));
    assert!(r["history_ignored"].is_null(), "{r}");
    assert_eq!(widths(&mut b), (1280, 3), "restore: the load is not a step of its own");
    c(&mut b, "undo", json!({}));
    assert_eq!(b.project.width, 640, "the first undo reverts the last edit made before the crash");
    c(&mut b, "undo", json!({"steps": 2}));
    assert_eq!(widths(&mut b).1, 0);
    c(&mut b, "redo", json!({}));
    assert_eq!(b.project.width, 320);

    // without `restore` the documented behaviour stays: the load is one more step on top
    let mut d = Session::new();
    c(&mut d, "load_project", json!({"path": ps, "with_history": true}));
    assert_eq!(widths(&mut d), (1280, 4));

    // `restore` without a matching history is an ordinary load (still undoable)
    let _ = std::fs::remove_file(format!("{ps}.history"));
    let mut e = Session::new();
    let r = c(&mut e, "load_project", json!({"path": ps, "with_history": true, "restore": true}));
    assert!(r["history_ignored"].is_string(), "{r}");
    assert_eq!(widths(&mut e), (1280, 1));
    let _ = std::fs::remove_file(&p);
}

fn tmp(tag: &str) -> std::path::PathBuf { std::env::temp_dir().join(format!("kimo_hr_{tag}_{}", std::process::id())) }
fn err(s: &mut Session, n: &str, a: Value) -> String { dispatch(s, n, &a).err().unwrap_or_else(|| panic!("{n} {a} should fail")) }

#[test]
fn checkpoints_overwrite_cap_list_restore_and_delete() {
    let mut s = Session::new();
    assert_eq!(err(&mut s, "checkpoint", json!({"name": "   "})), "name must be 1 to 80 characters");
    assert_eq!(err(&mut s, "checkpoint", json!({"name": "x".repeat(81)})), "name must be 1 to 80 characters");
    c(&mut s, "project_set", json!({"width": 320}));
    c(&mut s, "checkpoint", json!({"name": " a "}));
    c(&mut s, "project_set", json!({"width": 640}));
    let r = c(&mut s, "checkpoint", json!({"name": "a"}));
    assert_eq!(r["checkpoints"], json!(["a"]), "same name overwrites: {r}");
    let h = c(&mut s, "history_list", json!({}));
    assert_eq!(h["checkpoints"], json!([{"name": "a", "after_undo_steps": 2}]));
    c(&mut s, "project_set", json!({"width": 1280}));
    c(&mut s, "restore", json!({"name": "a"}));
    assert_eq!(s.project.width, 640, "the overwritten snapshot is the one restored");
    let e = err(&mut s, "restore", json!({"name": "zz"}));
    assert_eq!(e, "no checkpoint 'zz'. Existing: a");
    for i in 0..25 { c(&mut s, "checkpoint", json!({"name": format!("c{i}")})); }
    let names: Vec<String> = s.checkpoints.iter().map(|c| c.0.clone()).collect();
    assert_eq!(names.len(), 24, "keeps the newest 24");
    assert_eq!((names[0].as_str(), names[23].as_str()), ("c1", "c24"), "the oldest ('a', then c0) were dropped");
    assert_eq!(c(&mut s, "checkpoint_delete", json!({"name": "c5"})), json!({"ok": true}));
    assert_eq!(s.checkpoints.len(), 23);
    assert_eq!(err(&mut s, "checkpoint_delete", json!({"name": "c5"})), "no checkpoint 'c5'");
}

#[test]
fn checkpoints_survive_save_and_load_with_history() {
    let p = tmp("cp.kimo"); let ps = p.to_str().unwrap();
    let mut a = Session::new();
    c(&mut a, "project_set", json!({"width": 320})); c(&mut a, "checkpoint", json!({"name": "grade"})); c(&mut a, "project_set", json!({"width": 640}));
    c(&mut a, "save_project", json!({"path": ps, "with_history": true}));
    // a save without history removes the old history file beside the project
    let r = c(&mut a, "save_project", json!({"path": ps}));
    assert_eq!(r["stale_history_removed"], true);
    assert!(!std::path::Path::new(&format!("{ps}.history")).exists());
    let r = c(&mut a, "save_project", json!({"path": ps, "with_history": true}));
    assert_eq!((r["undo_steps"].as_u64(), r["checkpoints"].as_u64()), (Some(2), Some(1)), "{r}");
    let mut b = Session::new();
    let r = c(&mut b, "load_project", json!({"path": ps, "with_history": true, "restore": true}));
    assert_eq!((r["checkpoints"].as_u64(), r["no_history"].as_bool()), (Some(1), Some(true)), "{r}");
    c(&mut b, "restore", json!({"name": "grade"}));
    assert_eq!(b.project.width, 320);
    let _ = std::fs::remove_file(&p); let _ = std::fs::remove_file(format!("{ps}.history"));
}
