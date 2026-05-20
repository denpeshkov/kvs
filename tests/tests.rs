use std::{fs, process::Command};

use assert_cmd::prelude::*;
use kvs::{KvStore, KvsError, Result};
use predicates::{
    ord::eq,
    str::{PredicateStrExt, contains, is_empty},
};
use tempfile::TempDir;
use walkdir::WalkDir;

/// 4 * 1024 * 1024 — mirrors `kv::MAX_FILE_SIZE`.
const MAX_FILE_SIZE: usize = 4 * 1024 * 1024;

// `kvs` with no args should exit with a non-zero code.
#[test]
fn cli_no_args() {
    Command::cargo_bin("kvs").unwrap().assert().failure();
}

// `kvs -V` should print the version
#[test]
fn cli_version() {
    Command::cargo_bin("kvs")
        .unwrap()
        .args(["-V"])
        .assert()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

// `kvs get <KEY>` should print "Key not found" for a non-existent key and exit
// with zero.
#[test]
fn cli_get_non_existent_key() {
    let temp_dir = TempDir::new().unwrap();
    Command::cargo_bin("kvs")
        .unwrap()
        .args(["get", "key1"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(eq("Key not found").trim());
}

// `kvs rm <KEY>` should print "Key not found" for an empty database and exit
// with non-zero code.
#[test]
fn cli_rm_non_existent_key() {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    Command::cargo_bin("kvs")
        .unwrap()
        .args(["rm", "key1"])
        .current_dir(&temp_dir)
        .assert()
        .failure()
        .stdout(eq("Key not found").trim());
}

// `kvs set <KEY> <VALUE>` should print nothing and exit with zero.
#[test]
fn cli_set() {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    Command::cargo_bin("kvs")
        .unwrap()
        .args(["set", "key1", "value1"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(is_empty());
}

#[test]
fn cli_get_stored() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");

    let mut store = KvStore::open(temp_dir.path())?;
    store.set("key1".to_owned(), "value1".to_owned())?;
    store.set("key2".to_owned(), "value2".to_owned())?;
    drop(store);

    Command::cargo_bin("kvs")
        .unwrap()
        .args(["get", "key1"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(eq("value1").trim());

    Command::cargo_bin("kvs")
        .unwrap()
        .args(["get", "key2"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(eq("value2").trim());

    Ok(())
}

// `kvs rm <KEY>` should print nothing and exit with zero.
#[test]
fn cli_rm_stored() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");

    let mut store = KvStore::open(temp_dir.path())?;
    store.set("key1".to_owned(), "value1".to_owned())?;
    drop(store);

    Command::cargo_bin("kvs")
        .unwrap()
        .args(["rm", "key1"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(is_empty());

    Command::cargo_bin("kvs")
        .unwrap()
        .args(["get", "key1"])
        .current_dir(&temp_dir)
        .assert()
        .success()
        .stdout(eq("Key not found").trim());

    Ok(())
}

#[test]
fn cli_invalid_get() {
    Command::cargo_bin("kvs").unwrap().args(["get"]).assert().failure();

    Command::cargo_bin("kvs").unwrap().args(["get", "extra", "field"]).assert().failure();
}

#[test]
fn cli_invalid_set() {
    Command::cargo_bin("kvs").unwrap().args(["set"]).assert().failure();

    Command::cargo_bin("kvs").unwrap().args(["set", "missing_field"]).assert().failure();

    Command::cargo_bin("kvs").unwrap().args(["set", "extra", "extra", "field"]).assert().failure();
}

#[test]
fn cli_invalid_rm() {
    Command::cargo_bin("kvs").unwrap().args(["rm"]).assert().failure();

    Command::cargo_bin("kvs").unwrap().args(["rm", "extra", "field"]).assert().failure();
}

#[test]
fn cli_invalid_subcommand() {
    Command::cargo_bin("kvs").unwrap().args(["unknown", "subcommand"]).assert().failure();
}

// Should get previously stored value.
#[test]
fn get_stored_value() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    let mut store = KvStore::open(temp_dir.path())?;

    store.set("key1".to_owned(), "value1".to_owned())?;
    store.set("key2".to_owned(), "value2".to_owned())?;

    assert_eq!(store.get("key1".to_owned())?, Some("value1".to_owned()));
    assert_eq!(store.get("key2".to_owned())?, Some("value2".to_owned()));

    // Open from disk again and check persistent data.
    drop(store);
    let mut store = KvStore::open(temp_dir.path())?;
    assert_eq!(store.get("key1".to_owned())?, Some("value1".to_owned()));
    assert_eq!(store.get("key2".to_owned())?, Some("value2".to_owned()));

    Ok(())
}

// Should overwrite existent value.
#[test]
fn overwrite_value() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    println!("KvStore::open()");
    let mut store = KvStore::open(temp_dir.path())?;

    store.set("key1".to_owned(), "value1".to_owned())?;
    assert_eq!(store.get("key1".to_owned())?, Some("value1".to_owned()));
    store.set("key1".to_owned(), "value2".to_owned())?;
    assert_eq!(store.get("key1".to_owned())?, Some("value2".to_owned()));

    // Open from disk again and check persistent data.
    drop(store);
    println!("KvStore::open()");
    let mut store = KvStore::open(temp_dir.path())?;
    assert_eq!(store.get("key1".to_owned())?, Some("value2".to_owned()));
    store.set("key1".to_owned(), "value3".to_owned())?;
    assert_eq!(store.get("key1".to_owned())?, Some("value3".to_owned()));

    Ok(())
}

// Should get `None` when getting a non-existent key.
#[test]
fn get_non_existent_value() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    let mut store = KvStore::open(temp_dir.path())?;

    store.set("key1".to_owned(), "value1".to_owned())?;
    assert_eq!(store.get("key2".to_owned())?, None);

    // Open from disk again and check persistent data.
    drop(store);
    let mut store = KvStore::open(temp_dir.path())?;
    assert_eq!(store.get("key2".to_owned())?, None);

    Ok(())
}

#[test]
fn remove_non_existent_key() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    let mut store = KvStore::open(temp_dir.path())?;
    assert!(store.remove("key1".to_owned()).is_err());
    Ok(())
}

#[test]
fn remove_key() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    let mut store = KvStore::open(temp_dir.path())?;
    store.set("key1".to_owned(), "value1".to_owned())?;
    store.remove("key1".to_owned()).unwrap();
    assert_eq!(store.get("key1".to_owned())?, None);
    Ok(())
}

// Insert data until total size of the directory decreases.
// Test data correctness after compaction.
#[test]
fn compaction() -> Result<()> {
    let temp_dir = TempDir::new().expect("unable to create temporary working directory");
    let mut store = KvStore::open(temp_dir.path())?;

    let dir_size = || {
        let entries = WalkDir::new(temp_dir.path()).into_iter();
        let len: walkdir::Result<u64> = entries
            .map(|res| res.and_then(|entry| entry.metadata()).map(|metadata| metadata.len()))
            .sum();
        len.expect("fail to get directory size")
    };

    let mut current_size = dir_size();
    for iter in 0..1000 {
        for key_id in 0..1000 {
            let key = format!("key{key_id}");
            let value = format!("{iter}");
            store.set(key, value)?;
        }

        let new_size = dir_size();
        if new_size > current_size {
            current_size = new_size;
            continue;
        }
        // Compaction triggered.

        drop(store);
        // reopen and check content.
        let mut store = KvStore::open(temp_dir.path())?;
        for key_id in 0..1000 {
            let key = format!("key{key_id}");
            assert_eq!(store.get(key)?, Some(format!("{iter}")));
        }
        return Ok(());
    }

    panic!("No compaction detected");
}

fn open_temp_store() -> (KvStore, TempDir) {
    let dir = TempDir::new().expect("failed to create temp dir");
    let store = KvStore::open(dir.path()).expect("failed to open store");
    (store, dir)
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

fn log_file_count(path: &std::path::Path) -> usize {
    fs::read_dir(path)
        .unwrap()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|ext| ext == "log")
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_some()
        })
        .count()
}

#[test]
fn set_and_get_single_key() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("key".into(), "value".into())?;
    assert_eq!(store.get("key".into())?, Some("value".into()));
    Ok(())
}

#[test]
fn set_and_get_multiple_keys() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("a".into(), "1".into())?;
    store.set("b".into(), "2".into())?;
    store.set("c".into(), "3".into())?;
    assert_eq!(store.get("a".into())?, Some("1".into()));
    assert_eq!(store.get("b".into())?, Some("2".into()));
    assert_eq!(store.get("c".into())?, Some("3".into()));
    Ok(())
}

#[test]
fn overwrite_returns_latest_value() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("k".into(), "v1".into())?;
    assert_eq!(store.get("k".into())?, Some("v1".into()));
    store.set("k".into(), "v2".into())?;
    assert_eq!(store.get("k".into())?, Some("v2".into()));
    store.set("k".into(), "v3".into())?;
    assert_eq!(store.get("k".into())?, Some("v3".into()));
    Ok(())
}

#[test]
fn remove_then_get_returns_none() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("k".into(), "v".into())?;
    assert_eq!(store.get("k".into())?, Some("v".into()));
    store.remove("k".into())?;
    assert_eq!(store.get("k".into())?, None);
    Ok(())
}

#[test]
fn set_after_remove_same_key() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("k".into(), "v1".into())?;
    store.remove("k".into())?;
    assert_eq!(store.get("k".into())?, None);
    store.set("k".into(), "v2".into())?;
    assert_eq!(store.get("k".into())?, Some("v2".into()));
    Ok(())
}

#[test]
fn interleaved_set_get_remove() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("a".into(), "1".into())?;
    store.set("b".into(), "2".into())?;
    assert_eq!(store.get("a".into())?, Some("1".into()));

    store.remove("a".into())?;
    assert_eq!(store.get("a".into())?, None);
    assert_eq!(store.get("b".into())?, Some("2".into()));

    store.set("a".into(), "3".into())?;
    store.set("c".into(), "4".into())?;
    store.remove("b".into())?;

    assert_eq!(store.get("a".into())?, Some("3".into()));
    assert_eq!(store.get("b".into())?, None);
    assert_eq!(store.get("c".into())?, Some("4".into()));
    Ok(())
}

#[test]
fn persistence_basic_set_get() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k1".into(), "v1".into())?;
        store.set("k2".into(), "v2".into())?;
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get("k1".into())?, Some("v1".into()));
    assert_eq!(store.get("k2".into())?, Some("v2".into()));
    Ok(())
}

#[test]
fn persistence_overwrite() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k".into(), "v1".into())?;
        store.set("k".into(), "v2".into())?;
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get("k".into())?, Some("v2".into()));
    Ok(())
}

#[test]
fn persistence_remove() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k".into(), "v".into())?;
        store.remove("k".into())?;
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get("k".into())?, None);
    Ok(())
}

#[test]
fn persistence_multiple_reopen_cycles() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k".into(), "v1".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, Some("v1".into()));
        store.set("k".into(), "v2".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, Some("v2".into()));
        store.remove("k".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, None);
    }
    Ok(())
}

#[test]
fn persistence_set_remove_set_across_sessions() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k".into(), "v1".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        store.remove("k".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, None);
        store.set("k".into(), "v2".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, Some("v2".into()));
    }
    Ok(())
}

#[test]
fn persistence_many_keys() -> Result<()> {
    let dir = TempDir::new()?;
    let n = 500;
    {
        let mut store = KvStore::open(dir.path())?;
        for i in 0..n {
            store.set(format!("key{i:05}"), format!("value{i:05}"))?;
        }
    }
    let mut store = KvStore::open(dir.path())?;
    for i in 0..n {
        assert_eq!(store.get(format!("key{i:05}"))?, Some(format!("value{i:05}")));
    }
    Ok(())
}

#[test]
fn persistence_empty_store_reopen() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let _store = KvStore::open(dir.path())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("anything".into())?, None);
        store.set("k".into(), "v".into())?;
    }
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("k".into())?, Some("v".into()));
    }
    Ok(())
}

#[test]
fn empty_string_key() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set(String::new(), "value".into())?;
    assert_eq!(store.get(String::new())?, Some("value".into()));
    Ok(())
}

#[test]
fn empty_string_value() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("key".into(), String::new())?;
    assert_eq!(store.get("key".into())?, Some(String::new()));
    Ok(())
}

#[test]
fn empty_key_and_empty_value() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set(String::new(), String::new())?;
    assert_eq!(store.get(String::new())?, Some(String::new()));
    Ok(())
}

#[test]
fn key_with_special_characters() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    let keys = [
        "key with spaces",
        "key\twith\ttabs",
        "key\nwith\nnewlines",
        "key/with/slashes",
        "emoji\u{1F511}key",
        "\u{65E5}\u{672C}\u{8A9E}",
        "key=with=equals&and&ampersands",
    ];
    for (i, key) in keys.iter().enumerate() {
        store.set((*key).into(), format!("v{i}"))?;
    }
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(store.get((*key).into())?, Some(format!("v{i}")));
    }
    Ok(())
}

#[test]
fn value_with_special_characters() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    let value = "line1\nline2\ttab\r\nwindows\0null";
    store.set("k".into(), value.into())?;
    assert_eq!(store.get("k".into())?, Some(value.into()));
    Ok(())
}

#[test]
fn special_characters_persist_across_reopen() -> Result<()> {
    let dir = TempDir::new()?;
    let key = "key\nwith\nnewlines";
    let value = "value\0with\0nulls";
    {
        let mut store = KvStore::open(dir.path())?;
        store.set(key.into(), value.into())?;
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get(key.into())?, Some(value.into()));
    Ok(())
}

#[test]
fn large_value_within_buffer_limit() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    // ~500 bytes — well within the 1024-byte deserialize buffer
    let value = "x".repeat(500);
    store.set("k".into(), value.clone())?;
    assert_eq!(store.get("k".into())?, Some(value));
    Ok(())
}

#[test]
fn large_value_near_buffer_limit() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    // ~990 bytes + key + overhead approaches 1024
    let value = "x".repeat(990);
    store.set("k".into(), value.clone())?;
    assert_eq!(store.get("k".into())?, Some(value));
    Ok(())
}

#[test]
fn get_nonexistent_key_empty_store() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    assert_eq!(store.get("nonexistent".into())?, None);
    Ok(())
}

#[test]
fn get_nonexistent_key_nonempty_store() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("existing".into(), "value".into())?;
    assert_eq!(store.get("nonexistent".into())?, None);
    Ok(())
}

#[test]
fn remove_nonexistent_key_empty_store() {
    let (mut store, _dir) = open_temp_store();
    let err = store.remove("nonexistent".into()).unwrap_err();
    assert!(matches!(err, KvsError::KeyNotFound));
}

#[test]
fn remove_nonexistent_key_nonempty_store() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("a".into(), "1".into())?;
    let err = store.remove("b".into()).unwrap_err();
    assert!(matches!(err, KvsError::KeyNotFound));
    // Existing key unaffected
    assert_eq!(store.get("a".into())?, Some("1".into()));
    Ok(())
}

#[test]
fn double_remove_returns_key_not_found() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    store.set("k".into(), "v".into())?;
    store.remove("k".into())?;
    let err = store.remove("k".into()).unwrap_err();
    assert!(matches!(err, KvsError::KeyNotFound));
    Ok(())
}

#[test]
fn many_overwrites_same_key() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    for i in 0..100 {
        store.set("k".into(), format!("v{i}"))?;
    }
    assert_eq!(store.get("k".into())?, Some("v99".into()));
    Ok(())
}

#[test]
fn many_overwrites_persist() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        for i in 0..100 {
            store.set("k".into(), format!("v{i}"))?;
        }
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get("k".into())?, Some("v99".into()));
    Ok(())
}

#[test]
fn open_creates_nested_directory() -> Result<()> {
    let dir = TempDir::new()?;
    let nested = dir.path().join("a").join("b").join("c");
    assert!(!nested.exists());
    let mut store = KvStore::open(&nested)?;
    assert!(nested.exists());
    store.set("k".into(), "v".into())?;
    assert_eq!(store.get("k".into())?, Some("v".into()));
    Ok(())
}

#[test]
fn reopen_after_removing_all_keys() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("a".into(), "1".into())?;
        store.set("b".into(), "2".into())?;
        store.remove("a".into())?;
        store.remove("b".into())?;
    }
    let mut store = KvStore::open(dir.path())?;
    assert_eq!(store.get("a".into())?, None);
    assert_eq!(store.get("b".into())?, None);
    Ok(())
}

/// Non-.log files and .log files with non-numeric names should be ignored.
#[test]
fn non_log_files_in_directory_are_ignored() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("readme.txt"), "hello")?;
    fs::write(dir.path().join("data.json"), "{}")?;
    fs::write(dir.path().join("notanumber.log"), "garbage")?;

    let mut store = KvStore::open(dir.path())?;
    store.set("k".into(), "v".into())?;
    assert_eq!(store.get("k".into())?, Some("v".into()));
    Ok(())
}

#[test]
fn log_files_have_numeric_names() -> Result<()> {
    let dir = TempDir::new()?;
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("k".into(), "v".into())?;
    }
    let count = log_file_count(dir.path());
    assert!(count >= 1, "Expected at least one log file, got {count}");
    Ok(())
}

/// Writing > `MAX_FILE_SIZE` within a single session should trigger log
/// rotation.
#[test]
fn rotation_within_single_session() -> Result<()> {
    let dir = TempDir::new()?;
    let mut store = KvStore::open(dir.path())?;

    // Write ~4.5MB in a single session
    let value = "x".repeat(900);
    let entries = (MAX_FILE_SIZE / 900) + 500;
    for i in 0..entries {
        store.set(format!("key{i:05}"), value.clone())?;
    }

    let count = log_file_count(dir.path());
    assert!(
        count > 1,
        "Expected multiple log files after exceeding MAX_FILE_SIZE in one session, got {count}"
    );
    Ok(())
}

/// Rotation can trigger on the first `set()` of a new session if the log is
/// already > `MAX_FILE_SIZE`.
#[test]
fn rotation_on_first_write_after_reopen() -> Result<()> {
    let dir = TempDir::new()?;

    // Session 1: fill the log past MAX_FILE_SIZE
    {
        let mut store = KvStore::open(dir.path())?;
        let value = "x".repeat(900);
        let entries = (MAX_FILE_SIZE / 900) + 500;
        for i in 0..entries {
            store.set(format!("key{i:05}"), value.clone())?;
        }
    }

    // Session 2: first write should trigger rotation (uncompacted_bytes reset to 0)
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("trigger".into(), "rotation".into())?;
        drop(store);
    }

    let count = log_file_count(dir.path());
    assert!(count > 1, "Expected rotation on reopen, got {count} log file(s)");
    Ok(())
}

#[test]
fn values_readable_after_rotation_and_reopen() -> Result<()> {
    let dir = TempDir::new()?;

    // Session 1: fill the log past MAX_FILE_SIZE
    {
        let mut store = KvStore::open(dir.path())?;
        let value = "x".repeat(900);
        let entries = (MAX_FILE_SIZE / 900) + 500;
        for i in 0..entries {
            store.set(format!("key{i:05}"), value.clone())?;
        }
    }

    // Session 2: trigger rotation, then write to new log
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("new_key".into(), "new_value".into())?;

        // Within this session, old keys should still be accessible
        assert_eq!(store.get("key00000".into())?, Some("x".repeat(900)));
    }

    // Session 3: reopen with multiple log files
    {
        let mut store = KvStore::open(dir.path())?;
        // Key from old log file (log 0)
        assert_eq!(
            store.get("key00000".into())?,
            Some("x".repeat(900)),
            "key from old log file not readable after reopen"
        );
        // Key from new log file
        assert_eq!(store.get("new_key".into())?, Some("new_value".into()));
    }
    Ok(())
}

/// After rotation, overwritten keys should still reflect their latest value.
#[test]
fn overwrite_across_log_files() -> Result<()> {
    let dir = TempDir::new()?;

    // Session 1: write a key, then fill past MAX_FILE_SIZE
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("shared".into(), "old_value".into())?;
        let value = "x".repeat(900);
        let entries = (MAX_FILE_SIZE / 900) + 500;
        for i in 0..entries {
            store.set(format!("pad{i:05}"), value.clone())?;
        }
    }

    // Session 2: trigger rotation, overwrite the key in the new log
    {
        let mut store = KvStore::open(dir.path())?;
        store.set("shared".into(), "new_value".into())?;
    }

    // Session 3: should see the latest value
    {
        let mut store = KvStore::open(dir.path())?;
        assert_eq!(store.get("shared".into())?, Some("new_value".into()));
    }
    Ok(())
}

#[test]
fn compaction_reduces_size_and_preserves_data() -> Result<()> {
    let dir = TempDir::new()?;
    let mut store = KvStore::open(dir.path())?;

    let mut current_size = dir_size(dir.path());
    for iter in 0..1000 {
        for key_id in 0..1000 {
            store.set(format!("key{key_id}"), format!("{iter}"))?;
        }

        let new_size = dir_size(dir.path());
        if new_size < current_size {
            // Compaction triggered — verify all data
            drop(store);
            let mut store = KvStore::open(dir.path())?;
            for key_id in 0..1000 {
                assert_eq!(
                    store.get(format!("key{key_id}"))?,
                    Some(format!("{iter}")),
                    "key{key_id} wrong after compaction at iter {iter}"
                );
            }
            return Ok(());
        }
        current_size = new_size;
    }
    panic!("No compaction detected after 1M writes");
}

#[test]
fn bulk_set_then_bulk_get() -> Result<()> {
    let (mut store, _dir) = open_temp_store();
    let n = 1000;
    for i in 0..n {
        store.set(format!("key{i:05}"), format!("value{i:05}"))?;
    }
    for i in 0..n {
        assert_eq!(store.get(format!("key{i:05}"))?, Some(format!("value{i:05}")));
    }
    Ok(())
}

#[test]
fn bulk_set_remove_even_keys_persist() -> Result<()> {
    let dir = TempDir::new()?;
    let n = 500;
    {
        let mut store = KvStore::open(dir.path())?;
        for i in 0..n {
            store.set(format!("key{i:05}"), format!("value{i:05}"))?;
        }
        for i in (0..n).step_by(2) {
            store.remove(format!("key{i:05}"))?;
        }
    }
    let mut store = KvStore::open(dir.path())?;
    for i in 0..n {
        if i % 2 == 0 {
            assert_eq!(store.get(format!("key{i:05}"))?, None, "even key{i:05} should be removed");
        } else {
            assert_eq!(
                store.get(format!("key{i:05}"))?,
                Some(format!("value{i:05}")),
                "odd key{i:05} should exist"
            );
        }
    }
    Ok(())
}

#[test]
fn sequential_reopen_with_writes_each_cycle() -> Result<()> {
    let dir = TempDir::new()?;
    for cycle in 0..10 {
        let mut store = KvStore::open(dir.path())?;
        for i in 0..10 {
            store.set(format!("c{cycle}_k{i}"), format!("c{cycle}_v{i}"))?;
        }
        // Verify all keys from all cycles so far
        for c in 0..=cycle {
            for i in 0..10 {
                assert_eq!(
                    store.get(format!("c{c}_k{i}"))?,
                    Some(format!("c{c}_v{i}")),
                    "cycle {c} key {i} missing at cycle {cycle}"
                );
            }
        }
    }
    Ok(())
}

/// Overwrite all keys in bulk, verify only latest values survive.
#[test]
fn bulk_overwrite_all_keys() -> Result<()> {
    let dir = TempDir::new()?;
    let n = 200;
    {
        let mut store = KvStore::open(dir.path())?;
        for round in 0..5 {
            for i in 0..n {
                store.set(format!("key{i:03}"), format!("round{round}_val{i}"))?;
            }
        }
    }
    let mut store = KvStore::open(dir.path())?;
    for i in 0..n {
        assert_eq!(
            store.get(format!("key{i:03}"))?,
            Some(format!("round4_val{i}")),
            "key{i:03} should have round 4 value"
        );
    }
    Ok(())
}
